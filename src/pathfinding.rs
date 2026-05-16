use std::{collections::HashSet, error::Error, fmt};

use crate::{
    Chord, Finger, Fretboard, PitchClass, Position, Scale, MAX_FRET, MAX_STRING, MIN_FRET,
    MIN_STRING,
};

/// Default cap used by the convenience search functions so V1 DFS remains
/// responsive even when a transition has many possible local walks.
pub const DEFAULT_MAX_PATHS: usize = 256;

/// Maximum number of notes a V1 DFS path may contain.
///
/// This keeps the first engine iteration intentionally bounded; longer phrase
/// generation can be added once scoring and pruning are more sophisticated.
pub const MAX_PATH_NOTES: usize = 16;

/// Tag added to riffs that contain at least one melodic third.
pub const TAG_CONTAINS_THIRD: &str = "contains_third";

/// Tag added to riffs that contain at least one melodic sixth.
pub const TAG_CONTAINS_SIXTH: &str = "contains_sixth";

/// Tag added when a riff has been verified to stay entirely inside a scale.
pub const TAG_STRICT_DIATONIC: &str = "strict_diatonic";

/// Musical constraints that can be applied after DFS generation.
///
/// These filters deliberately sit outside the core graph walk: generation can
/// stay broad and cheap, while callers can keep only musically relevant paths
/// such as third-heavy motion, sixth-heavy motion, or fully diatonic walks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicalFilter {
    /// Keep only riffs that contain this exact tag.
    RequiredTag(String),
    /// Keep riffs that contain at least one of these tags.
    AnyTag(Vec<String>),
    /// Keep only riffs whose every note belongs to the supplied scale.
    StrictDiatonic(Scale),
}

impl MusicalFilter {
    pub fn required_tag(tag: impl Into<String>) -> Self {
        Self::RequiredTag(tag.into())
    }

    pub fn any_tag(tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::AnyTag(tags.into_iter().map(Into::into).collect())
    }

    pub const fn strict_diatonic(scale: Scale) -> Self {
        Self::StrictDiatonic(scale)
    }
}

/// Heavy additive cost used when inferred fingering asks the hand to exceed a
/// plausible four-fret span.
pub const ANATOMICAL_STRETCH_PENALTY: u32 = 1_000;

const MAX_COMFORTABLE_FRET_SPAN: u8 = 4;
const FRETS_PER_HAND_BOX: u8 = 4;
const MAX_NATURAL_FRET_SPAN_BUFFER: u8 = 2;
const SOFT_FINGER_STRETCH_PENALTY: u32 = 10;

/// A sequence of fretboard positions connecting two chords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Riff {
    sequence: Vec<Position>,
    fingers: Vec<Option<Finger>>,
    tags: Vec<String>,
    physical_cost: u32,
}

/// Calculates a cost for a riff in the context of a chord transition.
///
/// Scorers keep path generation separate from ranking. Lower costs are better,
/// allowing callers to sort candidate riffs from easiest/most idiomatic to
/// hardest.
pub trait RiffScorer {
    fn calculate_cost(&self, riff: &Riff, current_chord: &Chord, next_chord: &Chord) -> u32;
}

/// Scores a riff by physical fretboard distance.
///
/// Fret movement is intentionally squared per step: adjacent movement stays
/// cheap, while large same-string jumps become disproportionately expensive.
/// String movement remains linear because crossing one or two strings is much
/// less disruptive than moving the fretting hand several frets at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalDistanceScorer;

impl RiffScorer for PhysicalDistanceScorer {
    fn calculate_cost(&self, riff: &Riff, _current_chord: &Chord, _next_chord: &Chord) -> u32 {
        physical_cost(riff.sequence())
    }
}

impl Riff {
    /// Builds a riff and derives its fingering plus physical/anatomical cost
    /// from the supplied sequence.
    ///
    /// The cost is intentionally not caller-supplied so sorting and future
    /// filtering cannot observe a stale or inconsistent movement score. Fretted
    /// notes receive an inferred `Finger` (`1..=4`) inside the lowest viable hand
    /// position; open strings have no fretting-hand finger.
    pub fn new(sequence: Vec<Position>, tags: Vec<String>) -> Self {
        let fingers = infer_fingering(&sequence);
        let physical_cost = physical_cost_with_fingering(&sequence, &fingers);

        Self {
            sequence,
            fingers,
            tags,
            physical_cost,
        }
    }

    fn with_added_tag(mut self, tag: &str) -> Self {
        if !self.has_tag(tag) {
            self.tags.push(tag.to_owned());
        }

        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|candidate| candidate == tag)
    }

    pub fn sequence(&self) -> &[Position] {
        &self.sequence
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Inferred fretting-hand fingers aligned with [`Riff::sequence`].
    ///
    /// `None` means the note is an open string; `Some(Finger)` means use that
    /// numbered finger (`1` index through `4` pinky).
    pub fn fingers(&self) -> &[Option<Finger>] {
        &self.fingers
    }

    pub const fn physical_cost(&self) -> u32 {
        self.physical_cost
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetChordTone {
    Root,
    Third,
    Fifth,
}

impl TargetChordTone {
    const fn tag(self) -> &'static str {
        match self {
            Self::Root => "target_root",
            Self::Third => "target_third",
            Self::Fifth => "target_fifth",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathfindingError {
    PathTooShort { notes: usize },
    InvalidLengthRange { min_notes: usize, max_notes: usize },
    PathTooLong { notes: usize, max_notes: usize },
}

impl fmt::Display for PathfindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathTooShort { notes } => {
                write!(
                    f,
                    "path length {notes} must include at least start and target notes"
                )
            }
            Self::InvalidLengthRange {
                min_notes,
                max_notes,
            } => write!(
                f,
                "path length range {min_notes}..={max_notes} must have min <= max"
            ),
            Self::PathTooLong { notes, max_notes } => write!(
                f,
                "path length {notes} exceeds the V1 DFS limit of {max_notes} notes"
            ),
        }
    }
}

impl Error for PathfindingError {}

/// Finds local DFS paths with exactly `notes` positions.
///
/// A valid path starts on any fretboard position whose pitch class belongs to
/// `current_chord` and ends on a root, third, or fifth pitch class from
/// `next_chord`. Intermediate notes are unconstrained in V1; future musical
/// cost functions can add diatonic or interval filters on top of this core
/// graph walk.
pub fn find_paths(
    fretboard: Fretboard,
    current_chord: Chord,
    next_chord: Chord,
    notes: usize,
) -> Result<Vec<Riff>, PathfindingError> {
    find_paths_with_limit(
        fretboard,
        current_chord,
        next_chord,
        notes,
        notes,
        DEFAULT_MAX_PATHS,
    )
}

/// Finds local DFS paths whose note counts fall within `min_notes..=max_notes`.
pub fn find_paths_in_range(
    fretboard: Fretboard,
    current_chord: Chord,
    next_chord: Chord,
    min_notes: usize,
    max_notes: usize,
) -> Result<Vec<Riff>, PathfindingError> {
    find_paths_with_limit(
        fretboard,
        current_chord,
        next_chord,
        min_notes,
        max_notes,
        DEFAULT_MAX_PATHS,
    )
}

/// Finds DFS paths with an explicit result cap.
pub fn find_paths_with_limit(
    fretboard: Fretboard,
    current_chord: Chord,
    next_chord: Chord,
    min_notes: usize,
    max_notes: usize,
    max_paths: usize,
) -> Result<Vec<Riff>, PathfindingError> {
    validate_lengths(min_notes, max_notes)?;

    if max_paths == 0 {
        return Ok(Vec::new());
    }

    let start_pitch_classes = current_chord.pitch_classes();
    let target_pitch_classes = target_pitch_classes(next_chord);
    let mut starts: Vec<_> = fretboard
        .all_positions()
        .into_iter()
        .filter(|position| {
            start_pitch_classes.contains(&fretboard.note_at(*position).pitch_class())
        })
        .collect();
    starts.sort_by_key(|position| (position.fret(), position.string()));

    let mut riffs = Vec::new();
    let context = DfsContext {
        fretboard,
        target_pitch_classes,
        min_notes,
        max_notes,
        max_paths,
    };

    for start in starts {
        if riffs.len() >= context.max_paths {
            break;
        }

        let mut path = vec![start];
        let mut visited = HashSet::from([start]);
        dfs(&context, &mut path, &mut visited, &mut riffs);
    }

    riffs.sort_by_key(|riff| (riff.physical_cost(), riff.sequence().to_vec()));
    Ok(riffs)
}

fn validate_lengths(min_notes: usize, max_notes: usize) -> Result<(), PathfindingError> {
    if min_notes < 2 {
        return Err(PathfindingError::PathTooShort { notes: min_notes });
    }

    if min_notes > max_notes {
        return Err(PathfindingError::InvalidLengthRange {
            min_notes,
            max_notes,
        });
    }

    if max_notes > MAX_PATH_NOTES {
        return Err(PathfindingError::PathTooLong {
            notes: max_notes,
            max_notes: MAX_PATH_NOTES,
        });
    }

    Ok(())
}

struct DfsContext {
    fretboard: Fretboard,
    target_pitch_classes: Vec<(PitchClass, TargetChordTone)>,
    min_notes: usize,
    max_notes: usize,
    max_paths: usize,
}

fn dfs(
    context: &DfsContext,
    path: &mut Vec<Position>,
    visited: &mut HashSet<Position>,
    riffs: &mut Vec<Riff>,
) {
    if riffs.len() >= context.max_paths {
        return;
    }

    let current = *path
        .last()
        .expect("DFS path should always contain a starting position");
    if path.len() >= context.min_notes {
        if let Some(tone) = target_tone(context.fretboard, current, &context.target_pitch_classes) {
            riffs.push(build_riff(context.fretboard, path, tone));
            if riffs.len() >= context.max_paths {
                return;
            }
        }
    }

    if path.len() == context.max_notes {
        return;
    }

    for next in local_neighbors(current) {
        if visited.contains(&next) {
            continue;
        }

        visited.insert(next);
        path.push(next);
        dfs(context, path, visited, riffs);
        path.pop();
        visited.remove(&next);

        if riffs.len() >= context.max_paths {
            return;
        }
    }
}

const LOCAL_NEIGHBOR_OFFSETS: [(i8, i8); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

fn local_neighbors(position: Position) -> impl Iterator<Item = Position> {
    LOCAL_NEIGHBOR_OFFSETS
        .into_iter()
        .filter_map(move |(string_delta, fret_delta)| {
            let string = position.string() as i8 + string_delta;
            let fret = position.fret() as i8 + fret_delta;
            ((MIN_STRING as i8..=MAX_STRING as i8).contains(&string)
                && (MIN_FRET as i8..=MAX_FRET as i8).contains(&fret))
            .then(|| {
                Position::new(string as u8, fret as u8)
                    .expect("bounded neighbor coordinates should be valid")
            })
        })
}

fn target_pitch_classes(chord: Chord) -> Vec<(PitchClass, TargetChordTone)> {
    let intervals = chord.quality().intervals();
    [
        (intervals[0], TargetChordTone::Root),
        (intervals[1], TargetChordTone::Third),
        (intervals[2], TargetChordTone::Fifth),
    ]
    .into_iter()
    .map(|(interval, tone)| (chord.root().transpose(interval), tone))
    .collect()
}

fn target_tone(
    fretboard: Fretboard,
    position: Position,
    target_pitch_classes: &[(PitchClass, TargetChordTone)],
) -> Option<TargetChordTone> {
    let pitch_class = fretboard.note_at(position).pitch_class();
    target_pitch_classes
        .iter()
        .find_map(|(target_pitch_class, tone)| {
            (*target_pitch_class == pitch_class).then_some(*tone)
        })
}

fn build_riff(fretboard: Fretboard, path: &[Position], target_tone: TargetChordTone) -> Riff {
    let mut tags = vec![target_tone.tag().to_owned()];
    let first_note = fretboard.note_at(path[0]);
    let last_note = fretboard.note_at(
        *path
            .last()
            .expect("a riff must contain at least start and target notes"),
    );

    if last_note > first_note {
        tags.push("net_ascending".to_owned());
    } else if last_note < first_note {
        tags.push("net_descending".to_owned());
    }

    annotate_musical_tags(fretboard, Riff::new(path.to_vec(), tags))
}

/// Adds derived musical tags to a riff without changing its note sequence or cost.
///
/// This is useful for user-authored riffs as well as generated paths: once a
/// riff has been annotated, tag filters such as `contains_third` and
/// `contains_sixth` can be applied uniformly.
pub fn annotate_musical_tags(fretboard: Fretboard, riff: Riff) -> Riff {
    let mut has_third = false;
    let mut has_sixth = false;

    for positions in riff.sequence().windows(2) {
        let current = fretboard.note_at(positions[0]);
        let next = fretboard.note_at(positions[1]);
        let interval = current.midi_number().abs_diff(next.midi_number()) % PitchClass::COUNT;

        match interval {
            3 | 4 => has_third = true,
            8 | 9 => has_sixth = true,
            _ => {}
        }
    }

    let mut riff = riff;

    if has_third {
        riff = riff.with_added_tag(TAG_CONTAINS_THIRD);
    }

    if has_sixth {
        riff = riff.with_added_tag(TAG_CONTAINS_SIXTH);
    }

    riff
}

/// Applies musical filters to an existing riff list.
///
/// Required tag filters use the tags already attached to a riff by generation
/// or previous filters. `StrictDiatonic` inspects every pitch in the sequence
/// against the supplied scale and annotates kept riffs with `strict_diatonic`
/// so later tag-only passes can reuse that result.
pub fn apply_musical_filters(
    fretboard: Fretboard,
    riffs: impl IntoIterator<Item = Riff>,
    filters: &[MusicalFilter],
) -> Vec<Riff> {
    riffs
        .into_iter()
        .filter_map(|riff| apply_filters_to_riff(fretboard, riff, filters))
        .collect()
}

fn apply_filters_to_riff(
    fretboard: Fretboard,
    mut riff: Riff,
    filters: &[MusicalFilter],
) -> Option<Riff> {
    for filter in filters {
        match filter {
            MusicalFilter::RequiredTag(tag) => {
                if !riff.has_tag(tag) {
                    return None;
                }
            }
            MusicalFilter::AnyTag(tags) => {
                if !tags.iter().any(|tag| riff.has_tag(tag)) {
                    return None;
                }
            }
            MusicalFilter::StrictDiatonic(scale) => {
                if !is_strict_diatonic(fretboard, &riff, *scale) {
                    return None;
                }

                riff = riff.with_added_tag(TAG_STRICT_DIATONIC);
            }
        }
    }

    Some(riff)
}

fn is_strict_diatonic(fretboard: Fretboard, riff: &Riff, scale: Scale) -> bool {
    riff.sequence()
        .iter()
        .all(|position| scale.contains(fretboard.note_at(*position).pitch_class()))
}

/// Returns the physical movement cost for a sequence of fretboard positions.
///
/// The score is additive over every move in the path. Per move, fret distance
/// is squared and string distance is linear, so a single five-fret jump costs
/// much more than a controlled walk through adjacent frets. Lower is better.
///
/// Fretted notes are also assigned an inferred guitar finger (`1..=4`), and
/// anatomically implausible hand spans receive a heavy penalty so rankings avoid
/// stretches such as finger 1 on fret 2 while finger 4 reaches fret 8.
pub fn physical_cost(path: &[Position]) -> u32 {
    let fingers = infer_fingering(path);
    physical_cost_with_fingering(path, &fingers)
}

fn physical_cost_with_fingering(path: &[Position], fingers: &[Option<Finger>]) -> u32 {
    physical_movement_cost(path) + anatomical_cost_with_fingering(path, fingers)
}

pub(crate) fn physical_movement_cost(path: &[Position]) -> u32 {
    path.windows(2)
        .map(|positions| {
            let current = positions[0];
            let next = positions[1];
            let fret_distance = current.fret().abs_diff(next.fret()) as u32;
            let string_distance = current.string().abs_diff(next.string()) as u32;

            fret_distance.pow(2) + string_distance
        })
        .sum()
}

/// Infers one fretting-hand finger per position in the path.
///
/// Open strings return `None`; fretted notes map into a local four-fret hand box
/// that can shift as the line moves, while abrupt reaches are clamped to the
/// index or pinky so [`anatomical_cost`] can penalize them.
pub fn infer_fingering(path: &[Position]) -> Vec<Option<Finger>> {
    let mut hand_anchor = None;
    let mut previous_finger = None;
    let mut fingers = Vec::with_capacity(path.len());

    for position in path {
        if position.fret() == 0 {
            previous_finger = None;
            fingers.push(None);
            continue;
        }

        let anchor = local_hand_anchor(position.fret(), hand_anchor, previous_finger);
        hand_anchor = Some(anchor);

        let finger = finger_for_fret_in_box(position.fret(), anchor);
        previous_finger = Some(finger);
        fingers.push(Some(finger));
    }

    fingers
}

fn local_hand_anchor(fret: u8, current_anchor: Option<u8>, previous_finger: Option<Finger>) -> u8 {
    let Some(anchor) = current_anchor else {
        return fret;
    };

    let box_end = anchor + FRETS_PER_HAND_BOX - 1;
    if fret < anchor {
        if previous_finger == Some(Finger::Index) {
            fret.saturating_sub(FRETS_PER_HAND_BOX - 1)
        } else {
            fret
        }
    } else if fret > box_end {
        if previous_finger == Some(Finger::Pinky) {
            fret
        } else {
            fret.saturating_sub(FRETS_PER_HAND_BOX - 1)
        }
    } else {
        anchor
    }
}

fn finger_for_fret_in_box(fret: u8, anchor: u8) -> Finger {
    match fret.saturating_sub(anchor) {
        0 => Finger::Index,
        1 => Finger::Middle,
        2 => Finger::Ring,
        _ => Finger::Pinky,
    }
}

/// Returns only the biomechanical penalty component for a path.
pub fn anatomical_cost(path: &[Position]) -> u32 {
    let fingers = infer_fingering(path);
    anatomical_cost_with_fingering(path, &fingers)
}

fn anatomical_cost_with_fingering(path: &[Position], fingers: &[Option<Finger>]) -> u32 {
    path.iter()
        .zip(fingers.iter())
        .filter_map(|(position, finger)| finger.map(|finger| (*position, finger)))
        .collect::<Vec<_>>()
        .windows(2)
        .map(|fretted| {
            let (current_position, current_finger) = fretted[0];
            let (next_position, next_finger) = fretted[1];

            if current_finger == next_finger {
                0
            } else {
                anatomical_pair_cost(current_position, current_finger, next_position, next_finger)
            }
        })
        .sum()
}

fn anatomical_pair_cost(
    current_position: Position,
    current_finger: Finger,
    next_position: Position,
    next_finger: Finger,
) -> u32 {
    let fret_span = current_position.fret().abs_diff(next_position.fret());
    let finger_span = current_finger.number().abs_diff(next_finger.number());

    let stretch_excess = fret_span.saturating_sub(MAX_COMFORTABLE_FRET_SPAN);

    // Softly discourage finger pairs that reach beyond their natural spacing
    // without being outright impossible. The buffer allows, for example, index
    // to pinky over four frets before charging this smaller per-fret penalty.
    let soft_stretch_excess = fret_span.saturating_sub(finger_span + MAX_NATURAL_FRET_SPAN_BUFFER);

    (stretch_excess as u32 * ANATOMICAL_STRETCH_PENALTY)
        + (soft_stretch_excess as u32 * SOFT_FINGER_STRETCH_PENALTY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChordQuality, PitchClass, ScaleKind};

    #[test]
    fn dfs_finds_exact_length_paths_from_current_chord_shape_to_next_chord_tones() {
        let fretboard = Fretboard::standard();
        let current_chord = Chord::new(PitchClass::D, ChordQuality::Major);
        let next_chord = Chord::new(PitchClass::G, ChordQuality::Major);

        let riffs = find_paths_with_limit(fretboard, current_chord, next_chord, 3, 3, 32)
            .expect("D to G three-note paths should be searchable");

        assert!(!riffs.is_empty());
        let current_pitch_classes = current_chord.pitch_classes();
        let target_pitch_classes: Vec<_> = target_pitch_classes(next_chord)
            .into_iter()
            .map(|(pitch_class, _)| pitch_class)
            .collect();

        for riff in riffs {
            assert_eq!(riff.sequence().len(), 3);
            assert!(current_pitch_classes
                .contains(&fretboard.note_at(riff.sequence()[0]).pitch_class()));
            assert!(target_pitch_classes.contains(
                &fretboard
                    .note_at(*riff.sequence().last().expect("riff should have an end"))
                    .pitch_class()
            ));
            assert!(riff
                .sequence()
                .windows(2)
                .all(|positions| positions[0] != positions[1]));
            assert!(riff.tags().iter().any(|tag| tag.starts_with("target_")));
        }
    }

    #[test]
    fn range_search_keeps_paths_inside_the_requested_lengths() {
        let riffs = find_paths_with_limit(
            Fretboard::standard(),
            Chord::new(PitchClass::A, ChordQuality::Minor),
            Chord::new(PitchClass::C, ChordQuality::Major),
            2,
            4,
            64,
        )
        .expect("A minor to C major range search should be valid");

        assert!(!riffs.is_empty());
        assert!(riffs
            .iter()
            .all(|riff| (2..=4).contains(&riff.sequence().len())));
    }

    #[test]
    fn generated_riffs_receive_melodic_interval_tags() {
        let riffs = find_paths_with_limit(
            Fretboard::standard(),
            Chord::new(PitchClass::D, ChordQuality::Major),
            Chord::new(PitchClass::G, ChordQuality::Major),
            3,
            3,
            128,
        )
        .expect("D to G paths should be searchable");

        assert!(riffs.iter().any(|riff| riff.has_tag(TAG_CONTAINS_THIRD)));
    }

    #[test]
    fn musical_tag_annotation_detects_thirds_and_sixths() {
        let fretboard = Fretboard::standard();
        let third_riff = annotate_musical_tags(
            fretboard,
            Riff::new(
                vec![
                    Position::new(6, 0).expect("E2 should be valid"),
                    Position::new(6, 3).expect("G2 should be valid"),
                ],
                Vec::new(),
            ),
        );
        let sixth_riff = annotate_musical_tags(
            fretboard,
            Riff::new(
                vec![
                    Position::new(6, 0).expect("E2 should be valid"),
                    Position::new(5, 4).expect("C#3 should be valid"),
                ],
                Vec::new(),
            ),
        );

        assert!(third_riff.has_tag(TAG_CONTAINS_THIRD));
        assert!(!third_riff.has_tag(TAG_CONTAINS_SIXTH));
        assert!(sixth_riff.has_tag(TAG_CONTAINS_SIXTH));
        assert!(!sixth_riff.has_tag(TAG_CONTAINS_THIRD));
    }

    #[test]
    fn musical_filters_keep_required_interval_tags() {
        let fretboard = Fretboard::standard();
        let third_riff = annotate_musical_tags(
            fretboard,
            Riff::new(
                vec![
                    Position::new(6, 0).expect("E2 should be valid"),
                    Position::new(6, 3).expect("G2 should be valid"),
                ],
                Vec::new(),
            ),
        );
        let stepwise_riff = annotate_musical_tags(
            fretboard,
            Riff::new(
                vec![
                    Position::new(6, 0).expect("E2 should be valid"),
                    Position::new(6, 1).expect("F2 should be valid"),
                ],
                Vec::new(),
            ),
        );

        let filtered = apply_musical_filters(
            fretboard,
            vec![third_riff.clone(), stepwise_riff],
            &[MusicalFilter::required_tag(TAG_CONTAINS_THIRD)],
        );

        assert_eq!(filtered, vec![third_riff]);
    }

    #[test]
    fn musical_filters_keep_riffs_with_any_matching_tag() {
        let fretboard = Fretboard::standard();
        let third_riff = Riff::new(
            vec![
                Position::new(6, 0).expect("E2 should be valid"),
                Position::new(6, 3).expect("G2 should be valid"),
            ],
            vec![TAG_CONTAINS_THIRD.to_owned()],
        );
        let sixth_riff = Riff::new(
            vec![
                Position::new(6, 0).expect("E2 should be valid"),
                Position::new(5, 4).expect("C#3 should be valid"),
            ],
            vec![TAG_CONTAINS_SIXTH.to_owned()],
        );
        let target_only_riff = Riff::new(
            vec![
                Position::new(5, 3).expect("C3 should be valid"),
                Position::new(5, 5).expect("D3 should be valid"),
            ],
            vec!["target_root".to_owned()],
        );

        let filtered = apply_musical_filters(
            fretboard,
            vec![
                third_riff.clone(),
                sixth_riff.clone(),
                target_only_riff.clone(),
            ],
            &[MusicalFilter::any_tag([TAG_CONTAINS_SIXTH, "target_root"])],
        );

        assert_eq!(filtered, vec![sixth_riff, target_only_riff]);

        let no_match = apply_musical_filters(
            fretboard,
            vec![third_riff],
            &[MusicalFilter::any_tag(["chromatic", "pedal_point"])],
        );

        assert!(no_match.is_empty());
    }

    #[test]
    fn strict_diatonic_filter_keeps_scale_walks_and_adds_a_tag() {
        let fretboard = Fretboard::standard();
        let diatonic_riff = Riff::new(
            vec![
                Position::new(5, 3).expect("C3 should be valid"),
                Position::new(5, 5).expect("D3 should be valid"),
                Position::new(4, 2).expect("E3 should be valid"),
            ],
            Vec::new(),
        );
        let chromatic_riff = Riff::new(
            vec![
                Position::new(5, 3).expect("C3 should be valid"),
                Position::new(4, 4).expect("F#3 should be valid"),
            ],
            Vec::new(),
        );

        let filtered = apply_musical_filters(
            fretboard,
            vec![diatonic_riff.clone(), chromatic_riff],
            &[MusicalFilter::strict_diatonic(Scale::new(
                PitchClass::C,
                ScaleKind::Major,
            ))],
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].sequence(), diatonic_riff.sequence());
        assert!(filtered[0].has_tag(TAG_STRICT_DIATONIC));
    }

    #[test]
    fn riff_constructor_derives_physical_cost_from_sequence() {
        let riff = Riff::new(
            vec![
                Position::new(6, 2).expect("start position should be valid"),
                Position::new(5, 3).expect("neighbor position should be valid"),
                Position::new(4, 5).expect("target position should be valid"),
            ],
            vec!["target_root".to_owned()],
        );

        assert_eq!(riff.physical_cost(), 7);
    }

    #[test]
    fn physical_cost_penalizes_large_fret_jumps_more_than_stepwise_motion() {
        let jump = vec![
            Position::new(6, 2).expect("start position should be valid"),
            Position::new(6, 7).expect("large jump position should be valid"),
        ];
        let stepwise = vec![
            Position::new(6, 2).expect("start position should be valid"),
            Position::new(6, 3).expect("neighbor position should be valid"),
            Position::new(6, 4).expect("target position should be valid"),
        ];

        assert_eq!(physical_cost(&jump), 1025);
        assert_eq!(physical_cost(&stepwise), 2);
        assert!(physical_cost(&jump) > physical_cost(&stepwise));
    }

    #[test]
    fn riff_infers_fretting_fingers_for_physical_scoring() {
        let riff = Riff::new(
            vec![
                Position::new(6, 0).expect("open string should be valid"),
                Position::new(6, 2).expect("index position should be valid"),
                Position::new(6, 3).expect("middle position should be valid"),
                Position::new(6, 5).expect("pinky position should be valid"),
            ],
            Vec::new(),
        );

        assert_eq!(
            riff.fingers(),
            &[
                None,
                Some(Finger::Index),
                Some(Finger::Middle),
                Some(Finger::Pinky)
            ]
        );
    }

    #[test]
    fn anatomical_cost_adds_heavy_penalty_for_impossible_stretches() {
        let impossible_stretch = vec![
            Position::new(6, 2).expect("finger 1 fret should be valid"),
            Position::new(6, 8).expect("finger 4 fret should be valid"),
        ];
        let playable_stretch = vec![
            Position::new(6, 2).expect("finger 1 fret should be valid"),
            Position::new(6, 5).expect("finger 4 fret should be valid"),
        ];

        assert_eq!(
            infer_fingering(&impossible_stretch),
            vec![Some(Finger::Index), Some(Finger::Pinky)]
        );
        assert!(anatomical_cost(&impossible_stretch) >= ANATOMICAL_STRETCH_PENALTY);
        assert_eq!(anatomical_cost(&playable_stretch), 0);
        assert!(physical_cost(&impossible_stretch) > physical_cost(&playable_stretch));
    }

    #[test]
    fn anatomical_cost_allows_gradual_hand_shifts_across_a_longer_riff() {
        let gradual_shift = vec![
            Position::new(6, 2).expect("starting fret should be valid"),
            Position::new(6, 3).expect("neighbor fret should be valid"),
            Position::new(6, 4).expect("neighbor fret should be valid"),
            Position::new(6, 5).expect("neighbor fret should be valid"),
            Position::new(6, 6).expect("shifted fret should be valid"),
            Position::new(6, 7).expect("shifted fret should be valid"),
            Position::new(6, 8).expect("shifted fret should be valid"),
        ];

        assert!(anatomical_cost(&gradual_shift) < ANATOMICAL_STRETCH_PENALTY);
    }

    #[test]
    fn physical_distance_scorer_matches_riff_physical_cost() {
        let riff = Riff::new(
            vec![
                Position::new(4, 2).expect("start position should be valid"),
                Position::new(4, 3).expect("neighbor position should be valid"),
                Position::new(3, 4).expect("target position should be valid"),
            ],
            vec!["target_third".to_owned()],
        );
        let scorer = PhysicalDistanceScorer;

        assert_eq!(
            scorer.calculate_cost(
                &riff,
                &Chord::new(PitchClass::D, ChordQuality::Major),
                &Chord::new(PitchClass::G, ChordQuality::Major),
            ),
            riff.physical_cost()
        );
    }

    #[test]
    fn zero_max_paths_returns_an_empty_result_set() {
        let riffs = find_paths_with_limit(
            Fretboard::standard(),
            Chord::new(PitchClass::D, ChordQuality::Major),
            Chord::new(PitchClass::G, ChordQuality::Major),
            3,
            3,
            0,
        )
        .expect("zero result cap should still be a valid search request");

        assert!(riffs.is_empty());
    }

    #[test]
    fn dfs_rejects_paths_longer_than_the_v1_bound() {
        assert_eq!(
            find_paths_in_range(
                Fretboard::standard(),
                Chord::new(PitchClass::C, ChordQuality::Major),
                Chord::new(PitchClass::G, ChordQuality::Major),
                2,
                MAX_PATH_NOTES + 1,
            ),
            Err(PathfindingError::PathTooLong {
                notes: MAX_PATH_NOTES + 1,
                max_notes: MAX_PATH_NOTES,
            })
        );
    }

    #[test]
    fn dfs_rejects_unusable_lengths() {
        assert_eq!(
            find_paths(
                Fretboard::standard(),
                Chord::new(PitchClass::C, ChordQuality::Major),
                Chord::new(PitchClass::G, ChordQuality::Major),
                1,
            ),
            Err(PathfindingError::PathTooShort { notes: 1 })
        );

        assert_eq!(
            find_paths_in_range(
                Fretboard::standard(),
                Chord::new(PitchClass::C, ChordQuality::Major),
                Chord::new(PitchClass::G, ChordQuality::Major),
                5,
                4,
            ),
            Err(PathfindingError::InvalidLengthRange {
                min_notes: 5,
                max_notes: 4,
            })
        );
    }
}
