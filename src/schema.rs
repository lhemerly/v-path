//! Persistent profile schema for saved songs, chord transitions, and curated riffs.
//!
//! This module defines the in-memory shape that the upcoming serde layer will
//! read from and write to YAML/TOML profile files. The schema deliberately uses
//! small, explicit objects so Creator Mode can save several named riff
//! variations for the same chord transition and Live Mode can render them
//! without re-running pathfinding.
//!
//! Canonical YAML shape:
//!
//! ```yaml
//! schema_version: 1
//! song:
//!   title: "Example Tune"
//!   artist: "v-path"
//!   key:
//!     tonic: D
//!     kind: major
//!   tuning: standard
//! progression:
//!   - root: D
//!     quality: major
//!   - root: G
//!     quality: major
//! transitions:
//!   - id: d_to_g
//!     from:
//!       root: D
//!       quality: major
//!     to:
//!       root: G
//!       quality: major
//!     riffs:
//!       - name: Verse walk-up
//!         variation: verse
//!         tags: [target_root, net_ascending]
//!         physical_cost: 9
//!         positions:
//!           - { string: 4, fret: 0 }
//!           - { string: 4, fret: 2 }
//!           - { string: 3, fret: 0 }
//! ```
//!
//! Equivalent TOML shape:
//!
//! ```toml
//! schema_version = 1
//!
//! [song]
//! title = "Example Tune"
//! artist = "v-path"
//! tuning = "standard"
//!
//! [song.key]
//! tonic = "D"
//! kind = "major"
//!
//! [[progression]]
//! root = "D"
//! quality = "major"
//!
//! [[progression]]
//! root = "G"
//! quality = "major"
//!
//! [[transitions]]
//! id = "d_to_g"
//! from = { root = "D", quality = "major" }
//! to = { root = "G", quality = "major" }
//!
//! [[transitions.riffs]]
//! name = "Verse walk-up"
//! variation = "verse"
//! tags = ["target_root", "net_ascending"]
//! physical_cost = 9
//! positions = [
//!   { string = 4, fret = 0 },
//!   { string = 4, fret = 2 },
//!   { string = 3, fret = 0 },
//! ]
//! ```

use crate::{physical_cost, Chord, Position, Riff, Scale};

/// Current persisted profile schema version.
///
/// Serialization should write this value into `schema_version`; readers should
/// reject unsupported future versions rather than silently misreading profiles.
pub const PROFILE_SCHEMA_VERSION: u16 = 1;

/// Canonical YAML profile skeleton for documentation and future snapshot tests.
pub const YAML_PROFILE_EXAMPLE: &str = r#"schema_version: 1
song:
  title: "Example Tune"
  artist: "v-path"
  key:
    tonic: D
    kind: major
  tuning: standard
progression:
  - root: D
    quality: major
  - root: G
    quality: major
transitions:
  - id: d_to_g
    from:
      root: D
      quality: major
    to:
      root: G
      quality: major
    riffs:
      - name: Verse walk-up
        variation: verse
        tags: [target_root, net_ascending]
        physical_cost: 9
        positions:
          - { string: 4, fret: 0 }
          - { string: 4, fret: 2 }
          - { string: 3, fret: 0 }
      - name: Chorus answer
        variation: chorus
        tags: [target_third, contains_third]
        physical_cost: 6
        positions:
          - { string: 3, fret: 2 }
          - { string: 2, fret: 1 }
          - { string: 2, fret: 3 }
"#;

/// Canonical TOML profile skeleton for documentation and future snapshot tests.
pub const TOML_PROFILE_EXAMPLE: &str = r#"schema_version = 1

[song]
title = "Example Tune"
artist = "v-path"
tuning = "standard"

[song.key]
tonic = "D"
kind = "major"

[[progression]]
root = "D"
quality = "major"

[[progression]]
root = "G"
quality = "major"

[[transitions]]
id = "d_to_g"
from = { root = "D", quality = "major" }
to = { root = "G", quality = "major" }

[[transitions.riffs]]
name = "Verse walk-up"
variation = "verse"
tags = ["target_root", "net_ascending"]
physical_cost = 9
positions = [
  { string = 4, fret = 0 },
  { string = 4, fret = 2 },
  { string = 3, fret = 0 },
]

[[transitions.riffs]]
name = "Chorus answer"
variation = "chorus"
tags = ["target_third", "contains_third"]
physical_cost = 6
positions = [
  { string = 3, fret = 2 },
  { string = 2, fret = 1 },
  { string = 2, fret = 3 },
]
"#;

/// Top-level saved song profile.
///
/// `progression` stores the song-level chord sequence. `transitions` stores the
/// curated riff choices for any adjacent or repeated chord movement in that
/// progression. Multiple transitions may share the same `from`/`to` chords when
/// a song needs section-specific choices, but the preferred representation is a
/// single transition with multiple named [`SavedRiff`] variations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongProfile {
    pub schema_version: u16,
    pub song: SongMetadata,
    pub progression: Vec<Chord>,
    pub transitions: Vec<Transition>,
}

impl SongProfile {
    pub fn new(song: SongMetadata, progression: Vec<Chord>, transitions: Vec<Transition>) -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            song,
            progression,
            transitions,
        }
    }
}

/// Human-readable song metadata and global musical context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub key: Scale,
    pub tuning: Tuning,
}

impl SongMetadata {
    pub fn new(title: impl Into<String>, key: Scale) -> Self {
        Self {
            title: title.into(),
            artist: None,
            key,
            tuning: Tuning::Standard,
        }
    }

    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }
}

/// Fretboard tuning for a saved profile.
///
/// The MVP engine supports standard tuning only, so the schema captures that
/// assumption explicitly. This enum leaves room for backwards-compatible
/// alternate-tuning support in a future schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tuning {
    Standard,
}

/// A curated set of riff variations between two chords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub id: String,
    pub from: Chord,
    pub to: Chord,
    pub riffs: Vec<SavedRiff>,
}

impl Transition {
    pub fn new(id: impl Into<String>, from: Chord, to: Chord, riffs: Vec<SavedRiff>) -> Self {
        Self {
            id: id.into(),
            from,
            to,
            riffs,
        }
    }
}

/// A saved riff variation selected or authored in Creator Mode.
///
/// `name` is meant for display, while `variation` groups multiple riffs for the
/// same transition by arrangement role such as `verse`, `chorus`, `solo`, or
/// `ending`. `physical_cost` is persisted so Live Mode can sort or display saved
/// choices without recomputing engine scores, but constructors derive it from
/// positions to avoid stale costs in newly-created profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRiff {
    pub name: String,
    pub variation: Option<String>,
    pub positions: Vec<Position>,
    pub tags: Vec<String>,
    pub physical_cost: u32,
}

impl SavedRiff {
    pub fn new(
        name: impl Into<String>,
        variation: Option<String>,
        positions: Vec<Position>,
        tags: Vec<String>,
    ) -> Self {
        let physical_cost = physical_cost(&positions);

        Self {
            name: name.into(),
            variation,
            positions,
            tags,
            physical_cost,
        }
    }

    pub fn from_riff(name: impl Into<String>, variation: Option<String>, riff: &Riff) -> Self {
        Self::new(
            name,
            variation,
            riff.sequence().to_vec(),
            riff.tags().to_vec(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChordQuality, PitchClass, ScaleKind};

    #[test]
    fn saved_riff_derives_cost_from_positions() {
        let saved = SavedRiff::new(
            "Verse walk-up",
            Some("verse".to_owned()),
            vec![
                Position::new(4, 0).expect("D string open should be valid"),
                Position::new(4, 2).expect("D string fret 2 should be valid"),
                Position::new(3, 0).expect("G string open should be valid"),
            ],
            vec!["target_root".to_owned()],
        );

        assert_eq!(saved.physical_cost, 9);
        assert_eq!(saved.variation.as_deref(), Some("verse"));
    }

    #[test]
    fn saved_riff_can_be_built_from_engine_riff() {
        let riff = Riff::new(
            vec![
                Position::new(6, 2).expect("low E fret 2 should be valid"),
                Position::new(6, 3).expect("low E fret 3 should be valid"),
            ],
            vec!["target_root".to_owned(), "net_ascending".to_owned()],
        );

        let saved = SavedRiff::from_riff("Chorus answer", Some("chorus".to_owned()), &riff);

        assert_eq!(saved.name, "Chorus answer");
        assert_eq!(saved.positions, riff.sequence());
        assert_eq!(saved.tags, riff.tags());
        assert_eq!(saved.physical_cost, riff.physical_cost());
    }

    #[test]
    fn profile_groups_multiple_variations_under_one_transition() {
        let from = Chord::new(PitchClass::D, ChordQuality::Major);
        let to = Chord::new(PitchClass::G, ChordQuality::Major);
        let verse = SavedRiff::new(
            "Verse walk-up",
            Some("verse".to_owned()),
            vec![
                Position::new(4, 0).expect("D string open should be valid"),
                Position::new(4, 2).expect("D string fret 2 should be valid"),
            ],
            Vec::new(),
        );
        let chorus = SavedRiff::new(
            "Chorus answer",
            Some("chorus".to_owned()),
            vec![
                Position::new(3, 2).expect("G string fret 2 should be valid"),
                Position::new(2, 3).expect("B string fret 3 should be valid"),
            ],
            Vec::new(),
        );

        let profile = SongProfile::new(
            SongMetadata::new("Example Tune", Scale::new(PitchClass::D, ScaleKind::Major))
                .with_artist("v-path"),
            vec![from, to],
            vec![Transition::new("d_to_g", from, to, vec![verse, chorus])],
        );

        assert_eq!(profile.schema_version, PROFILE_SCHEMA_VERSION);
        assert_eq!(profile.transitions[0].riffs.len(), 2);
        assert_eq!(
            profile.transitions[0].riffs[0].variation.as_deref(),
            Some("verse")
        );
        assert_eq!(
            profile.transitions[0].riffs[1].variation.as_deref(),
            Some("chorus")
        );
    }

    #[test]
    fn schema_examples_document_required_top_level_objects() {
        for example in [YAML_PROFILE_EXAMPLE, TOML_PROFILE_EXAMPLE] {
            assert!(example.contains("schema_version"));
            assert!(example.contains("song"));
            assert!(example.contains("progression"));
            assert!(example.contains("transitions"));
            assert!(example.contains("riffs"));
            assert!(example.contains("variation"));
        }
    }
}
