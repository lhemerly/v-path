use std::{error::Error, io, time::Duration};

use crate::{
    apply_musical_filters, find_paths_with_limit, Chord, ChordQuality, Fretboard, MusicalFilter,
    PitchClass, Riff,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

const CREATOR_RESULT_LIMIT: usize = 96;
const CREATOR_MIN_NOTES: usize = 3;
const CREATOR_MAX_NOTES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    MainMenu,
    Creator,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuChoice {
    CreatorMode,
    LiveMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreatorPrompt {
    Chord,
    TagFilter,
}

#[derive(Debug, Clone)]
pub struct App {
    mode: AppMode,
    should_quit: bool,
    menu_index: usize,
    creator: CreatorState,
}

#[derive(Debug, Clone)]
pub struct CreatorState {
    progression: Vec<Chord>,
    chord_input: String,
    tag_filter_input: String,
    tag_filter: Option<String>,
    prompt: CreatorPrompt,
    transition_index: usize,
    riffs: Vec<Riff>,
    selected_riff: usize,
    saved_riff: Option<Riff>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::MainMenu,
            should_quit: false,
            menu_index: 0,
            creator: CreatorState::new(),
        }
    }
}

impl CreatorState {
    fn new() -> Self {
        Self {
            progression: Vec::new(),
            chord_input: String::new(),
            tag_filter_input: String::new(),
            tag_filter: None,
            prompt: CreatorPrompt::Chord,
            transition_index: 0,
            riffs: Vec::new(),
            selected_riff: 0,
            saved_riff: None,
            status: "Type a chord such as D, Gm, A7, Cmaj7, or Bdim; Enter adds it.".to_owned(),
        }
    }

    pub fn progression(&self) -> &[Chord] {
        &self.progression
    }

    pub fn riffs(&self) -> &[Riff] {
        &self.riffs
    }

    pub fn selected_riff_index(&self) -> usize {
        self.selected_riff
    }

    pub fn tag_filter(&self) -> Option<&str> {
        self.tag_filter.as_deref()
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    fn current_transition(&self) -> Option<(Chord, Chord)> {
        self.progression
            .windows(2)
            .nth(self.transition_index)
            .map(|pair| (pair[0], pair[1]))
    }

    fn add_chord_from_input(&mut self) {
        let raw = self.chord_input.trim().to_owned();
        if raw.is_empty() {
            self.status = "Enter a chord name before adding the next progression step.".to_owned();
            return;
        }

        match parse_chord(&raw) {
            Ok(chord) => {
                self.progression.push(chord);
                self.chord_input.clear();
                self.transition_index = self.progression.len().saturating_sub(2);
                self.status = format!(
                    "Added {chord}. Add another chord to generate a transition, or use h/l to revisit transitions."
                );
                self.refresh_riffs();
            }
            Err(error) => {
                self.status = format!("Could not parse chord '{raw}': {error}");
            }
        }
    }

    fn refresh_riffs(&mut self) {
        self.riffs.clear();
        self.selected_riff = 0;

        let Some((from, to)) = self.current_transition() else {
            return;
        };

        let generated = match find_paths_with_limit(
            Fretboard::standard(),
            from,
            to,
            CREATOR_MIN_NOTES,
            CREATOR_MAX_NOTES,
            CREATOR_RESULT_LIMIT,
        ) {
            Ok(riffs) => riffs,
            Err(error) => {
                self.status = format!("Could not generate riffs for {from} → {to}: {error}");
                return;
            }
        };

        self.riffs = if let Some(tag) = self.tag_filter.as_deref() {
            apply_musical_filters(
                Fretboard::standard(),
                generated,
                &[MusicalFilter::RequiredTag(tag.to_owned())],
            )
        } else {
            generated
        };

        if self.riffs.is_empty() {
            if let Some(tag) = self.tag_filter.as_deref() {
                self.status = format!("No generated TABs for {from} → {to} match tag '{tag}'.");
            } else {
                self.status = format!("No generated TABs found for {from} → {to}.");
            }
        } else {
            self.status = format!(
                "Generated {} ranked TABs for {from} → {to}. Use j/k to scroll and Enter to select.",
                self.riffs.len()
            );
        }
    }

    fn move_selection_down(&mut self) {
        if !self.riffs.is_empty() {
            self.selected_riff = (self.selected_riff + 1).min(self.riffs.len() - 1);
        }
    }

    fn move_selection_up(&mut self) {
        self.selected_riff = self.selected_riff.saturating_sub(1);
    }

    fn select_riff(&mut self) {
        if let Some(riff) = self.riffs.get(self.selected_riff).cloned() {
            let cost = riff.physical_cost();
            self.saved_riff = Some(riff);
            self.status = format!(
                "Selected TAB #{} with score {cost}.",
                self.selected_riff + 1
            );
        } else if self.current_transition().is_some() {
            self.status =
                "No TAB is available to select; clear or change the tag filter.".to_owned();
        } else {
            self.status = "Add at least two chords before selecting a generated TAB.".to_owned();
        }
    }

    fn move_transition_next(&mut self) {
        if self.progression.len() < 2 {
            return;
        }
        self.transition_index = (self.transition_index + 1).min(self.progression.len() - 2);
        self.refresh_riffs();
    }

    fn move_transition_previous(&mut self) {
        if self.progression.len() < 2 {
            return;
        }
        self.transition_index = self.transition_index.saturating_sub(1);
        self.refresh_riffs();
    }

    fn start_tag_filter(&mut self) {
        self.prompt = CreatorPrompt::TagFilter;
        self.tag_filter_input.clear();
        self.status = "Filter by tag: target_root, target_third, target_fifth, net_ascending, net_descending, contains_third, contains_sixth.".to_owned();
    }

    fn apply_tag_filter(&mut self) {
        let tag = self.tag_filter_input.trim();
        self.tag_filter = (!tag.is_empty()).then(|| tag.to_owned());
        self.tag_filter_input.clear();
        self.prompt = CreatorPrompt::Chord;
        self.refresh_riffs();
    }

    fn clear_tag_filter(&mut self) {
        self.tag_filter = None;
        self.refresh_riffs();
    }
}

impl App {
    pub fn mode(&self) -> AppMode {
        self.mode
    }

    pub fn selected_menu_choice(&self) -> MainMenuChoice {
        match self.menu_index {
            0 => MainMenuChoice::CreatorMode,
            _ => MainMenuChoice::LiveMode,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn creator(&self) -> &CreatorState {
        &self.creator
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match self.mode {
            AppMode::MainMenu => self.handle_main_menu_key(key),
            AppMode::Creator => self.handle_creator_key(key),
            AppMode::Live => self.handle_live_key(key),
        }
    }

    fn handle_main_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.menu_index = (self.menu_index + 1).min(1),
            KeyCode::Char('k') | KeyCode::Up => self.menu_index = self.menu_index.saturating_sub(1),
            KeyCode::Enter => {
                self.mode = match self.selected_menu_choice() {
                    MainMenuChoice::CreatorMode => AppMode::Creator,
                    MainMenuChoice::LiveMode => AppMode::Live,
                }
            }
            _ => {}
        }
    }

    fn handle_creator_key(&mut self, key: KeyEvent) {
        if self.creator.prompt == CreatorPrompt::TagFilter {
            match key.code {
                KeyCode::Esc => {
                    self.creator.prompt = CreatorPrompt::Chord;
                    self.creator.tag_filter_input.clear();
                    self.creator.status = "Tag filter cancelled.".to_owned();
                }
                KeyCode::Enter => self.creator.apply_tag_filter(),
                KeyCode::Backspace => {
                    self.creator.tag_filter_input.pop();
                }
                KeyCode::Char(c) => self.creator.tag_filter_input.push(c),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc => self.mode = AppMode::MainMenu,
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.creator.move_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => self.creator.move_selection_up(),
            KeyCode::Char('h') | KeyCode::Left => self.creator.move_transition_previous(),
            KeyCode::Char('l') | KeyCode::Right => self.creator.move_transition_next(),
            KeyCode::Char('t') => self.creator.start_tag_filter(),
            KeyCode::Char('T') => self.creator.clear_tag_filter(),
            KeyCode::Enter => {
                if self.creator.chord_input.trim().is_empty() {
                    self.creator.select_riff();
                } else {
                    self.creator.add_chord_from_input();
                }
            }
            KeyCode::Backspace => {
                self.creator.chord_input.pop();
            }
            KeyCode::Char(c) => self.creator.chord_input.push(c),
            _ => {}
        }
    }

    fn handle_live_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = AppMode::MainMenu,
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, App::default());

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<(), Box<dyn Error>> {
    loop {
        terminal.draw(|frame| render(frame, &app))?;

        if app.should_quit() {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
    match app.mode {
        AppMode::MainMenu => render_main_menu(frame, app),
        AppMode::Creator => render_creator(frame, &app.creator),
        AppMode::Live => render_live(frame),
    }
}

fn render_main_menu(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect(62, 45, frame.area());
    frame.render_widget(Clear, area);

    let items = ["Creator Mode", "Live Mode"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let prefix = if index == app.menu_index {
                "▶ "
            } else {
                "  "
            };
            ListItem::new(format!("{prefix}{label}"))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(Block::default().title(" v-path ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Green),
        );

    frame.render_widget(list, area);

    let help =
        Paragraph::new("j/k or ↑/↓ move · Enter selects · q quits").alignment(Alignment::Center);
    let help_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(2),
        width: area.width,
        height: 1,
    };
    frame.render_widget(help, help_area);
}

fn render_creator(frame: &mut Frame<'_>, state: &CreatorState) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_creator_header(frame, vertical[0], state);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(vertical[1]);

    render_transition_pane(frame, panes[0], state);
    render_riff_list(frame, panes[1], state);

    let status = Paragraph::new(state.status.as_str())
        .block(Block::default().title(" Status ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(status, vertical[2]);
}

fn render_creator_header(frame: &mut Frame<'_>, area: Rect, state: &CreatorState) {
    let prompt_title = match state.prompt {
        CreatorPrompt::Chord => " Chord Builder ",
        CreatorPrompt::TagFilter => " Tag Filter ",
    };
    let input = match state.prompt {
        CreatorPrompt::Chord => state.chord_input.as_str(),
        CreatorPrompt::TagFilter => state.tag_filter_input.as_str(),
    };
    let filter = state.tag_filter.as_deref().unwrap_or("none");
    let progression = if state.progression.is_empty() {
        "(empty)".to_owned()
    } else {
        state
            .progression
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" → ")
    };

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(prompt_title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(input),
        ]),
        Line::from(format!("Progression: {progression}")),
        Line::from(format!(
            "Filter: {filter} · j/k scroll · Enter add/select · t filter · T clear · Esc menu"
        )),
    ])
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn render_transition_pane(frame: &mut Frame<'_>, area: Rect, state: &CreatorState) {
    let lines = if let Some((from, to)) = state.current_transition() {
        vec![
            Line::from(Span::styled(
                format!(
                    "Transition {} of {}",
                    state.transition_index + 1,
                    state.progression.len() - 1
                ),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("      {from}")),
            Line::from("       ↓"),
            Line::from(format!("      {to}")),
            Line::from(""),
            Line::from("Use h/l or ←/→ to move between chord transitions."),
            Line::from("The right pane ranks generated TABs by physical score."),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "Step 1",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from("Type a chord in the top input and press Enter."),
            Line::from(""),
            Line::from("Step 2"),
            Line::from("Add a second chord to generate transition TABs."),
            Line::from(""),
            Line::from("Examples: D, G, Em, A7, Cmaj7, Bdim, Caug."),
        ]
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Chord Transition ")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_riff_list(frame: &mut Frame<'_>, area: Rect, state: &CreatorState) {
    let items = if state.riffs.is_empty() {
        vec![ListItem::new("No generated TABs yet.")]
    } else {
        state
            .riffs
            .iter()
            .enumerate()
            .map(|(index, riff)| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!("#{:02} score {:>3}", index + 1, riff.physical_cost()),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("  tags: {}", riff.tags().join(", "))),
                    ]),
                    Line::from(render_tab(riff)),
                ])
            })
            .collect::<Vec<_>>()
    };

    let mut list_state = ListState::default();
    if !state.riffs.is_empty() {
        list_state.select(Some(state.selected_riff));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Generated TABs ")
                .borders(Borders::ALL),
        )
        .highlight_symbol("▶ ")
        .highlight_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_live(frame: &mut Frame<'_>) {
    let paragraph = Paragraph::new(vec![
        Line::from(Span::styled(
            "Live Mode",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("Profile loading and performance grid are planned next."),
        Line::from("Press Esc to return to the main menu or q to quit."),
    ])
    .block(Block::default().title(" The Stage ").borders(Borders::ALL))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, frame.area());
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_tab(riff: &Riff) -> String {
    riff.sequence()
        .iter()
        .map(|position| format!("s{}f{}", position.string(), position.fret()))
        .collect::<Vec<_>>()
        .join(" → ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordParseError(String);

impl std::fmt::Display for ChordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ChordParseError {}

pub fn parse_chord(input: &str) -> Result<Chord, ChordParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ChordParseError("empty chord".to_owned()));
    }

    let mut chars = input.char_indices();
    let (_, first) = chars
        .next()
        .ok_or_else(|| ChordParseError("empty chord".to_owned()))?;
    let first = first.to_ascii_uppercase();
    if !matches!(first, 'A'..='G') {
        return Err(ChordParseError("root must be A through G".to_owned()));
    }

    let mut root = first.to_string();
    let mut suffix_start = first.len_utf8();
    if let Some((index, accidental)) = chars.next() {
        if accidental == '#' || accidental == 'b' {
            root.push(accidental);
            suffix_start = index + accidental.len_utf8();
        }
    }

    let pitch = root
        .parse::<PitchClass>()
        .map_err(|error| ChordParseError(error.to_string()))?;
    let suffix = input[suffix_start..].trim().to_ascii_lowercase();
    let quality = match suffix.as_str() {
        "" | "maj" | "major" => ChordQuality::Major,
        "m" | "min" | "minor" => ChordQuality::Minor,
        "dim" | "°" => ChordQuality::Diminished,
        "aug" | "+" => ChordQuality::Augmented,
        "7" | "dom7" => ChordQuality::DominantSeventh,
        "maj7" | "major7" | "ma7" => ChordQuality::MajorSeventh,
        "m7" | "min7" | "minor7" => ChordQuality::MinorSeventh,
        other => {
            return Err(ChordParseError(format!(
                "unsupported chord quality '{other}'"
            )))
        }
    };

    Ok(Chord::new(pitch, quality))
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let suffix = match self.quality() {
            ChordQuality::Major => "",
            ChordQuality::Minor => "m",
            ChordQuality::Diminished => "dim",
            ChordQuality::Augmented => "aug",
            ChordQuality::DominantSeventh => "7",
            ChordQuality::MajorSeventh => "maj7",
            ChordQuality::MinorSeventh => "m7",
        };
        write!(f, "{}{}", self.root(), suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn main_menu_selects_creator_and_live_modes() {
        let mut app = App::default();

        assert_eq!(app.mode(), AppMode::MainMenu);
        assert_eq!(app.selected_menu_choice(), MainMenuChoice::CreatorMode);

        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected_menu_choice(), MainMenuChoice::LiveMode);

        app.handle_key(key(KeyCode::Char('k')));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.mode(), AppMode::Creator);
    }

    #[test]
    fn creator_adds_progression_steps_and_generates_ranked_riffs() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Enter));

        for code in [
            KeyCode::Char('D'),
            KeyCode::Enter,
            KeyCode::Char('G'),
            KeyCode::Enter,
        ] {
            app.handle_key(key(code));
        }

        let creator = app.creator();
        assert_eq!(creator.progression().len(), 2);
        assert!(!creator.riffs().is_empty());
        assert!(creator
            .riffs()
            .windows(2)
            .all(|pair| pair[0].physical_cost() <= pair[1].physical_cost()));
    }

    #[test]
    fn creator_supports_j_k_selection_and_enter_selection() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Enter));
        for code in [
            KeyCode::Char('D'),
            KeyCode::Enter,
            KeyCode::Char('G'),
            KeyCode::Enter,
        ] {
            app.handle_key(key(code));
        }

        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.creator().selected_riff_index(), 1);
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.creator().selected_riff_index(), 0);
        app.handle_key(key(KeyCode::Enter));
        assert!(app.creator().status().contains("Selected TAB #1"));
    }

    #[test]
    fn creator_filters_generated_tabs_by_tag() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Enter));
        for code in [
            KeyCode::Char('D'),
            KeyCode::Enter,
            KeyCode::Char('G'),
            KeyCode::Enter,
        ] {
            app.handle_key(key(code));
        }

        app.handle_key(key(KeyCode::Char('t')));
        for c in "target_root".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.creator().tag_filter(), Some("target_root"));
        assert!(app
            .creator()
            .riffs()
            .iter()
            .all(|riff| riff.has_tag("target_root")));
    }

    #[test]
    fn parses_common_chord_spellings() {
        assert_eq!(
            parse_chord("D").unwrap(),
            Chord::new(PitchClass::D, ChordQuality::Major)
        );
        assert_eq!(
            parse_chord("Bbmaj7").unwrap(),
            Chord::new(PitchClass::ASharp, ChordQuality::MajorSeventh)
        );
        assert_eq!(
            parse_chord("em7").unwrap(),
            Chord::new(PitchClass::E, ChordQuality::MinorSeventh)
        );
        assert!(parse_chord("H").is_err());
    }
}
