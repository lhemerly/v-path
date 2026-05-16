use std::{collections::HashSet, error::Error, fmt};

use crate::{Chord, Fretboard, PitchClass, Position, MAX_FRET, MAX_STRING, MIN_FRET, MIN_STRING};

/// Default cap used by the convenience search functions so V1 DFS remains
/// responsive even when a transition has many possible local walks.
pub const DEFAULT_MAX_PATHS: usize = 256;

/// Maximum number of notes a V1 DFS path may contain.
///
/// This keeps the first engine iteration intentionally bounded; longer phrase
/// generation can be added once scoring and pruning are more sophisticated.
pub const MAX_PATH_NOTES: usize = 16;

/// A sequence of fretboard positions connecting two chords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Riff {
    sequence: Vec<Position>,
    tags: Vec<String>,
    physical_cost: u32,
}

impl Riff {
    /// Builds a riff and derives its physical cost from the supplied sequence.
    ///
    /// The cost is intentionally not caller-supplied so sorting and future
    /// filtering cannot observe a stale or inconsistent movement score.
    pub fn new(sequence: Vec<Position>, tags: Vec<String>) -> Self {
        let physical_cost = physical_cost(&sequence);

        Self {
            sequence,
            tags,
            physical_cost,
        }
    }

    pub fn sequence(&self) -> &[Position] {
        &self.sequence
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
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
    LOCAL_NEIGHBOR_OFFSETS.into_iter().filter_map(move |(string_delta, fret_delta)| {
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

    Riff::new(path.to_vec(), tags)
}

fn physical_cost(path: &[Position]) -> u32 {
    path.windows(2)
        .map(|positions| {
            let current = positions[0];
            let next = positions[1];
            current.fret().abs_diff(next.fret()) as u32
                + current.string().abs_diff(next.string()) as u32
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChordQuality, PitchClass};

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
    fn riff_constructor_derives_physical_cost_from_sequence() {
        let riff = Riff::new(
            vec![
                Position::new(6, 2).expect("start position should be valid"),
                Position::new(5, 3).expect("neighbor position should be valid"),
                Position::new(4, 5).expect("target position should be valid"),
            ],
            vec!["target_root".to_owned()],
        );

        assert_eq!(riff.physical_cost(), 5);
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
