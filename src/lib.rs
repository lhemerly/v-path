//! Core library for v-path.
//!
//! The first layer of the engine is intentionally small and strict: musical
//! concepts are represented by typed values instead of free-form strings or raw
//! integers. Higher-level graph and pathfinding code can build on these types
//! without repeatedly validating theory invariants.

pub mod fingering;
pub mod fretboard;
pub mod pathfinding;
pub mod schema;
pub mod theory;
pub mod tui;

pub use fingering::{
    AnchorFinger, Finger, HandState, HandStateError, HandStateMachine, HandTransition,
};

pub use fretboard::{
    Fretboard, FretboardError, Position, MAX_FRET, MAX_STRING, MIN_FRET, MIN_STRING, STRING_COUNT,
};

pub use pathfinding::{
    annotate_musical_tags, apply_musical_filters, find_paths, find_paths_in_range,
    find_paths_with_limit, physical_cost, MusicalFilter, PathfindingError, PhysicalDistanceScorer,
    Riff, RiffScorer, TargetChordTone, DEFAULT_MAX_PATHS, MAX_PATH_NOTES, TAG_CONTAINS_SIXTH,
    TAG_CONTAINS_THIRD, TAG_STRICT_DIATONIC,
};

pub use schema::{
    SavedRiff, SongMetadata, SongProfile, Transition, Tuning, PROFILE_SCHEMA_VERSION,
    TOML_PROFILE_EXAMPLE, YAML_PROFILE_EXAMPLE,
};

pub use theory::{
    Chord, ChordQuality, Interval, IntervalError, Note, NoteError, PitchClass, Scale, ScaleKind,
};
