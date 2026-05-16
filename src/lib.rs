//! Core library for v-path.
//!
//! The first layer of the engine is intentionally small and strict: musical
//! concepts are represented by typed values instead of free-form strings or raw
//! integers. Higher-level graph and pathfinding code can build on these types
//! without repeatedly validating theory invariants.

pub mod fretboard;
pub mod pathfinding;
pub mod theory;

pub use fretboard::{
    Fretboard, FretboardError, Position, MAX_FRET, MAX_STRING, MIN_FRET, MIN_STRING, STRING_COUNT,
};

pub use pathfinding::{
    find_paths, find_paths_in_range, find_paths_with_limit, physical_cost, PathfindingError,
    PhysicalDistanceScorer, Riff, RiffScorer, TargetChordTone, DEFAULT_MAX_PATHS, MAX_PATH_NOTES,
};

pub use theory::{
    Chord, ChordQuality, Interval, IntervalError, Note, NoteError, PitchClass, Scale, ScaleKind,
};
