# **Contributing to v-path**

Welcome to v-path\! If you are a Rust developer with an interest in music theory, you are in the right place.  
To ensure the project remains fast, minimalist, and mathematically sound, please read this architecture guide before opening a PR.

## **Architecture Overview**

v-path is not a static database; it is a constraint solver. When contributing to the core engine, keep the following mental model in mind.

### **1\. The Fretboard is a Graph**

Do not hardcode tabs. Think of the guitar neck as a graph where nodes are (String, Fret) and edges represent the transition to the next note.  
When writing generation logic, you are writing pathfinding algorithms. For the MVP, assume the graph is always mapped to Standard Tuning (EADGBE).

### **2\. Core Structs (Mental Model)**

If you are working on the engine, you will interact with variants of these structures. Ensure your code respects these boundaries:  
// A point on the fretboard  
pub struct Position {  
    pub string: u8, // 1 to 6  
    pub fret: u8,   // 0 (open) to 24  
}

// A sequence of positions connecting two chords  
pub struct Riff {  
    pub sequence: Vec\<Position\>,  
    pub tags: Vec\<String\>,  
    pub physical\_cost: u32,  
}

### **3\. The Scoring Trait**

We use a plugin-style architecture for scoring riffs. If you want to add a new way to evaluate a riff (e.g., "how jazzy does this sound?"), implement a trait rather than cluttering the core generation loop:  
pub trait RiffScorer {  
    fn calculate\_cost(\&self, riff: \&Riff, current\_chord: \&Chord, next\_chord: \&Chord) \-\> u32;  
}

## **UI Guidelines (Ratatui)**

* **Minimalism over flash:** Do not use colors just because you can. Rely on terminal defaults where possible, using bolding or single accent colors (like green for target notes) to convey information.  
* **Vim Bindings:** All scrollable areas must support j/k navigation.

## **Submitting a PR**

1. Ensure your code passes all tests: cargo test  
2. Run cargo clippy \-- \-D warnings to ensure idiomatic Rust.  
3. If you are touching the core engine, you *must* include a unit test verifying that your pathfinding logic actually produces a valid musical path between two predefined chords.
