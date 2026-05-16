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

- Physical Cost Optimization: Using an algorithm to minimize $\Delta \text{Fret} + \Delta \text{String}$, ensuring the hand doesn't make impossible jumps.

# Getting Started

The core crate now exposes strict music-theory domain types for `Note`, `PitchClass`, `Interval`, `Chord`, and `Scale`. These types live in `src/theory.rs` and are re-exported from `v_path`, giving future fretboard and pathfinding code validated building blocks instead of raw strings or integers.

(Installation instructions to be added upon release of v0.1.0)

To run from source:

```
cargo run --release
```


# Current Engine Surface

The first implemented layer is the domain model:

- `PitchClass`: canonical twelve-tone pitch classes with enharmonic parsing and transposition.
- `Note`: an absolute pitch-class plus octave value using scientific pitch notation, constrained to the supported octave range.
- `Interval`: a validated non-negative semitone distance capped at two octaves for MVP-scale generation.
- `Chord`: a root plus typed `ChordQuality`, expandable to pitch classes.
- `Scale`: a tonic plus typed `ScaleKind`, expandable to pitch classes with membership checks for diatonic filtering.

Run the domain model tests with:

```
cargo test
```
