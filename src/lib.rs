//! Core library for v-path.
//!
//! The first layer of the engine is intentionally small and strict: musical
//! concepts are represented by typed values instead of free-form strings or raw
//! integers. Higher-level graph and pathfinding code can build on these types
//! without repeatedly validating theory invariants.

pub mod theory;

pub use theory::{
    Chord, ChordQuality, Interval, IntervalError, Note, NoteError, PitchClass, Scale, ScaleKind,
};
