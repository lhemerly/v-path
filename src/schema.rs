//! Persistent profile schema for saved songs, chord transitions, and curated riffs.
//!
//! This module defines the in-memory shape that serde reads from and writes to
//! YAML/TOML profile files. The schema deliberately uses
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

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::{pathfinding::physical_movement_cost, physical_cost, Chord, Position, Riff, Scale};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

    /// Reads a song profile from a YAML document.
    pub fn from_yaml_str(input: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(input)
    }

    /// Writes a song profile as a YAML document.
    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Reads a song profile from a TOML document.
    pub fn from_toml_str(input: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(input)
    }

    /// Writes a song profile as a pretty TOML document.
    pub fn to_toml_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

impl<'de> Deserialize<'de> for SongProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSongProfile {
            schema_version: u16,
            song: SongMetadata,
            progression: Vec<Chord>,
            transitions: Vec<Transition>,
        }

        let raw = RawSongProfile::deserialize(deserializer)?;
        if raw.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported profile schema_version {}; expected {}",
                raw.schema_version, PROFILE_SCHEMA_VERSION
            )));
        }

        Ok(Self {
            schema_version: raw.schema_version,
            song: raw.song,
            progression: raw.progression,
            transitions: raw.transitions,
        })
    }
}

/// Human-readable song metadata and global musical context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongMetadata {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tuning {
    Standard,
}

/// A curated set of riff variations between two chords.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedRiff {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variation: Option<String>,
    positions: Vec<Position>,
    tags: Vec<String>,
    physical_cost: u32,
}

impl<'de> Deserialize<'de> for SavedRiff {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSavedRiff {
            name: String,
            variation: Option<String>,
            positions: Vec<Position>,
            tags: Vec<String>,
            physical_cost: u32,
        }

        let raw = RawSavedRiff::deserialize(deserializer)?;
        let expected_cost = physical_cost(&raw.positions);
        let legacy_v1_cost = physical_movement_cost(&raw.positions);
        if raw.physical_cost != expected_cost && raw.physical_cost != legacy_v1_cost {
            return Err(de::Error::custom(format!(
                "saved riff physical_cost {} does not match positions cost {}",
                raw.physical_cost, expected_cost
            )));
        }

        Ok(Self {
            name: raw.name,
            variation: raw.variation,
            positions: raw.positions,
            tags: raw.tags,
            physical_cost: raw.physical_cost,
        })
    }
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn variation(&self) -> Option<&str> {
        self.variation.as_deref()
    }

    pub fn set_variation(&mut self, variation: Option<String>) {
        self.variation = variation;
    }

    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    /// Replaces this riff's saved position sequence and recomputes the derived cost.
    ///
    /// Creator Mode should use this whenever an edited saved riff changes notes;
    /// this preserves the invariant that [`SavedRiff::physical_cost`] always
    /// describes the current positions.
    pub fn set_positions(&mut self, positions: Vec<Position>) {
        self.physical_cost = physical_cost(&positions);
        self.positions = positions;
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags;
    }

    pub const fn physical_cost(&self) -> u32 {
        self.physical_cost
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

        assert_eq!(saved.physical_cost(), 9);
        assert_eq!(saved.variation(), Some("verse"));
    }

    #[test]
    fn saved_riff_position_edits_recompute_physical_cost() {
        let mut saved = SavedRiff::new(
            "Editable walk",
            None,
            vec![
                Position::new(6, 2).expect("low E fret 2 should be valid"),
                Position::new(6, 3).expect("low E fret 3 should be valid"),
            ],
            Vec::new(),
        );

        assert_eq!(saved.physical_cost(), 1);

        saved.set_positions(vec![
            Position::new(6, 2).expect("low E fret 2 should be valid"),
            Position::new(6, 7).expect("low E fret 7 should be valid"),
        ]);

        assert_eq!(saved.physical_cost(), 1025);
        assert_eq!(saved.positions()[1].fret(), 7);
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

        assert_eq!(saved.name(), "Chorus answer");
        assert_eq!(saved.positions(), riff.sequence());
        assert_eq!(saved.tags(), riff.tags());
        assert_eq!(saved.physical_cost(), riff.physical_cost());
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
        assert_eq!(profile.transitions[0].riffs[0].variation(), Some("verse"));
        assert_eq!(profile.transitions[0].riffs[1].variation(), Some("chorus"));
    }

    #[test]
    fn yaml_profile_example_round_trips_through_serde() {
        let profile = SongProfile::from_yaml_str(YAML_PROFILE_EXAMPLE)
            .expect("canonical YAML profile should deserialize");

        assert_eq!(profile.schema_version, PROFILE_SCHEMA_VERSION);
        assert_eq!(profile.song.title, "Example Tune");
        assert_eq!(profile.progression.len(), 2);
        assert_eq!(profile.transitions[0].riffs.len(), 2);
        assert_eq!(
            profile.transitions[0].riffs[0].positions()[2],
            Position::new(3, 0).unwrap()
        );

        let serialized = profile
            .to_yaml_string()
            .expect("profile should serialize back to YAML");
        let round_tripped = SongProfile::from_yaml_str(&serialized)
            .expect("serialized YAML should deserialize again");

        assert_eq!(round_tripped, profile);
    }

    #[test]
    fn toml_profile_example_round_trips_through_serde() {
        let profile = SongProfile::from_toml_str(TOML_PROFILE_EXAMPLE)
            .expect("canonical TOML profile should deserialize");

        assert_eq!(profile.song.key.tonic(), PitchClass::D);
        assert_eq!(profile.transitions[0].from.root(), PitchClass::D);
        assert_eq!(profile.transitions[0].to.root(), PitchClass::G);
        assert_eq!(profile.transitions[0].riffs[1].variation(), Some("chorus"));

        let serialized = profile
            .to_toml_string_pretty()
            .expect("profile should serialize back to TOML");
        let round_tripped = SongProfile::from_toml_str(&serialized)
            .expect("serialized TOML should deserialize again");

        assert_eq!(round_tripped, profile);
    }

    #[test]
    fn deserialization_rejects_future_schema_versions() {
        let future_profile =
            YAML_PROFILE_EXAMPLE.replacen("schema_version: 1", "schema_version: 2", 1);

        let error = SongProfile::from_yaml_str(&future_profile)
            .expect_err("future profile versions should be rejected");

        assert!(error
            .to_string()
            .contains("unsupported profile schema_version 2"));
    }

    #[test]
    fn deserialization_accepts_legacy_v1_movement_only_costs() {
        let legacy_profile = r#"schema_version: 1
song:
  title: "Legacy Stretch"
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
  - id: legacy_d_to_g
    from:
      root: D
      quality: major
    to:
      root: G
      quality: major
    riffs:
      - name: Old wide jump
        tags: [target_root]
        physical_cost: 25
        positions:
          - { string: 6, fret: 2 }
          - { string: 6, fret: 7 }
"#;

        let profile = SongProfile::from_yaml_str(legacy_profile)
            .expect("legacy v1 movement-only costs should remain readable");

        assert_eq!(profile.transitions[0].riffs[0].physical_cost(), 25);
    }

    #[test]
    fn deserialization_rejects_stale_saved_riff_costs() {
        let stale_cost = YAML_PROFILE_EXAMPLE.replacen("physical_cost: 9", "physical_cost: 99", 1);

        let error = SongProfile::from_yaml_str(&stale_cost)
            .expect_err("stale saved riff costs should be rejected");

        assert!(error.to_string().contains("does not match positions cost"));
    }

    #[test]
    fn deserialization_validates_fretboard_positions() {
        let bad_position =
            YAML_PROFILE_EXAMPLE.replacen("string: 4, fret: 0", "string: 7, fret: 0", 1);

        let error = SongProfile::from_yaml_str(&bad_position)
            .expect_err("invalid fretboard coordinates should be rejected");

        assert!(error.to_string().contains("outside the supported range"));
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
