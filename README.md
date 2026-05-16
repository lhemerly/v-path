# v-path 🎸🦀

v-path is a terminal-based, mathematical pathfinding engine for the guitar.

Instead of relying on static chord charts or memorizing hundreds of "fill-in" licks, v-path treats the fretboard as a directed acyclic graph. It calculates the most efficient, musically pleasant pathways (riffs) between two chords based on user-defined constraints, voice leading, and physical fretboard distance.

Built for minimalists. No bloated UIs, no subscriptions. Just pure Rust, music theory, and a terminal.

# Two Modes of Operation

v-path operates entirely within a Ratatui-based TUI, split into two distinct workflows:

1. Creator Mode (The Forge)

Before a gig, you need to plan your transitions.

Select a key and input a sequence of chords.

For every transition (e.g., D to G), v-path generates possible riffs.

Filter by tags: ascending, descending, thirds, chromatic, pedal_point.

The engine calculates the Path Cost (minimizing finger movement and maximizing open string resonance).

You preview the ASCII Tabs, select your favorites (or create variations like "Chorus Transition"), and save the profile to a lightweight .yml file.

2. Live Mode (The Stage)

When it's time to play, the math gets out of the way.

Load a saved song profile.

The TUI displays a highly optimized, high-contrast grid of your chosen transition TABs.

Designed to be readable from a distance on a laptop or tablet terminal while you play.

# The Theory Behind the Engine

v-path doesn't use a static database of "cool licks." It calculates them on the fly using:

- Diatonic Filtering: Isolating the vector space to the current key/mode.

- Voice Leading Continuity: Ensuring the exit note of a riff is a half or whole step away from the target chord's root or 3rd.

- Physical Cost Optimization: Using an algorithm to minimize $\Delta \text{Fret}^2 + \Delta \text{String}$ per move, making adjacent fret motion cheap while large fret jumps become disproportionately expensive.

# Getting Started

The core crate now exposes strict music-theory domain types for `Note`, `PitchClass`, `Interval`, `Chord`, and `Scale`, plus a standard-tuned fretboard coordinate system via `Position` and `Fretboard`. These types live in `src/theory.rs` and `src/fretboard.rs` and are re-exported from `v_path`, giving future pathfinding code validated building blocks instead of raw strings or integers.

(Installation instructions to be added upon release of v0.1.0)

To run from source:

```
cargo run --release
```

To open Live Mode with a saved YAML profile path preloaded, pass the `.yml` file as the first argument:

```
cargo run --release -- path/to/song.yml
```


# Current Engine Surface

The first implemented layers are the domain model, the MVP fretboard grid, the V1 pathfinding engine, and the saved-profile data schema:

- `PitchClass`: canonical twelve-tone pitch classes with enharmonic parsing and transposition.
- `Note`: an absolute pitch-class plus octave value using scientific pitch notation, constrained to the supported octave range.
- `Interval`: a validated non-negative semitone distance capped at two octaves for MVP-scale generation.
- `Chord`: a root plus typed `ChordQuality`, expandable to pitch classes.
- `Scale`: a tonic plus typed `ScaleKind`, expandable to pitch classes with membership checks for diatonic filtering.
- `Position`: a validated `(String, Fret)` coordinate where strings are `1..=6` and frets are `0..=24`.
- `Fretboard`: a 6-string, 24-fret standard-tuned guitar grid only; open strings are EADGBE from low to high pitch (`6=E2`, `5=A2`, `4=D3`, `3=G3`, `2=B3`, `1=E4`).
- `Finger`, `AnchorFinger`, `HandState`, and `HandStateMachine`: Phase 4 fingering primitives that validate and track the fretting-hand anchors of the current chord shape, including retained anchors across chord transitions.
- `Riff`: a DFS-generated sequence of fretboard positions with target-tone/motion/melodic-interval tags and a derived physical movement cost.
- `physical_cost`, `RiffScorer`, and `PhysicalDistanceScorer`: physical ranking primitives where adjacent fret movement stays cheap and large fret jumps are penalized quadratically while string crossing remains linear.
- `annotate_musical_tags`, `MusicalFilter`, and `apply_musical_filters`: musical filtering primitives for keeping riffs with derived tags such as `contains_third` or `contains_sixth`, or for requiring strict diatonic walks inside a supplied `Scale`.
- `find_paths`, `find_paths_in_range`, and `find_paths_with_limit`: V1 DFS helpers that walk local fretboard neighbors from Chord A pitch-class positions to Chord B root/3rd/5th targets.
- `SongProfile`, `SongMetadata`, `Transition`, `SavedRiff`, and `Tuning`: serde-backed YAML/TOML persistence types, including multiple named riff variations per chord transition.
- `PROFILE_SCHEMA_VERSION`, `YAML_PROFILE_EXAMPLE`, and `TOML_PROFILE_EXAMPLE`: canonical schema version and example profile shapes covered by serializer round-trip tests.
- Ratatui/Crossterm TUI entry point with a main menu for Creator Mode vs Live Mode, a Creator Mode progression builder that generates, ranks, scrolls, filters, previews, and selects transition TABs, and a Live Mode performance grid that loads YAML profiles into high-contrast ASCII TAB cards.

# Saved Profile Schema

v-path profiles are versioned song documents. The canonical shape is available from the library as `YAML_PROFILE_EXAMPLE` and `TOML_PROFILE_EXAMPLE`; both forms save the same objects:

- `schema_version`: currently `1`, used to reject unsupported future profile formats.
- `song`: title, optional artist, global key, and MVP `standard` tuning.
- `progression`: the song-level chord sequence.
- `transitions`: curated `from`/`to` chord movements, each with one or more saved riffs.
- `riffs`: named variations such as `verse` and `chorus`, with tags, derived physical cost, and fretboard `positions` using conventional guitar string numbers (`1` high E through `6` low E).

Minimal YAML example:

```yaml
schema_version: 1
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
```

Equivalent TOML files use the same field names with `[[progression]]`, `[[transitions]]`, and `[[transitions.riffs]]` arrays of tables. `SongProfile::from_yaml_str`, `SongProfile::to_yaml_string`, `SongProfile::from_toml_str`, and `SongProfile::to_toml_string_pretty` provide the current read/write API.

Run the engine tests with:

```
cargo test
```
