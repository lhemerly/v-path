# **v-path Development Roadmap**

This roadmap defines the implementation sequence for v-path. We are prioritizing core mathematical generation before expanding the UI or advanced biomechanical features.

## **Phase 1: The Core Engine (Music Theory & Graph Math)**

*Goal: Build the underlying logic that generates and scores riffs.*

Current status: the strict music-theory domain model and standard-tuned fretboard coordinate system are implemented and covered by unit tests. The next Phase 1 task is the first pathfinding algorithm over those validated positions.

* \[x\] **Domain Modeling:** Implement strict types for Note, PitchClass, Interval, Chord, and Scale.  
* \[x\] **Fretboard Coordinate System:** Map the 6 strings and 24 frets to a Cartesian grid (String, Fret). *Constraint: MVP strictly assumes Standard Tuning (EADGBE).*  
* \[ \] **Pathfinding Algorithm (V1):** Implement a depth-first search (DFS) or A\* algorithm to find paths of ![][image1] length between Chord A's shape and Chord B's root/3rd/5th.  
* \[ \] **Cost Function (Physical):** Implement distance scoring. A jump from fret 2 to 7 should score poorly; a progression from fret 2 to 3 to 4 should score well.  
* \[ \] **Cost Function (Musical):** Implement tag-based filters (e.g., keeping only paths that contain 3rds or 6ths, or strict diatonic walks).

## **Phase 2: State Management & Persistence**

*Goal: Allow users to save their curated riffs.*

* \[ \] **Data Schema:** Define the YAML/TOML structure for saving Song, Transitions, and Riff objects.  
* \[ \] **Serialization:** Implement serde to read/write these profiles seamlessly.  
* \[ \] **Variation Support:** Ensure the schema supports multiple riffs for the same chord transition (e.g., D \-\> G (Verse) vs D \-\> G (Chorus)).

## **Phase 3: The TUI (Ratatui Implementation)**

*Goal: Build the dual-mode interface.*

* \[ \] **Main Menu:** Simple selection between "Creator Mode" and "Live Mode".  
* \[ \] **Creator Mode UI:**  
  * Step-by-step chord progression builder.  
  * Split pane: Left side shows chord transition, Right side shows scrollable list of generated TABs ranked by score.  
  * Keyboard shortcuts (j/k to scroll, Enter to select, t to filter by tags).  
* \[ \] **Live Mode UI:**  
  * Dynamic grid layout. Reads the YAML file and displays high-contrast, large-text ASCII tabs for the selected song.  
  * Clean, distraction-free margin layout.

## **Phase 4: Biomechanical Model (Fingering) \- *Advanced***

*Goal: Tell the user exactly which finger to use.*

* \[ \] **Hand State Machine:** Track the anchor fingers of the current chord.  
* \[ \] **Anatomical Constraints:** Update the A\* algorithm to include Finger (1..4). Add heavy penalties for physically impossible stretches (e.g., Finger 1 on Fret 2, Finger 4 on Fret 8).  
* \[ \] **TAB Update:** Render suggested fingering below the ASCII TAB strings.

[image1]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABIAAAAYCAYAAAD3Va0xAAAAyUlEQVR4XmNgGAWkgolA/A+I/0OxG6o0GMDkYLgcVRoVwBT9RZeAgvtAvAFdEBt4A8TfGSCGcaLJgcAXdAFswAqIs4HYiAFi0BlUaTAAiRMEO5HYMC8iA0Yg3ocmhhUga2yC8pEDNAeI7ZD4OMErND66q9DlsQJjIM5FE7vIADFIDcpH9ypWsIkBEgbIgIsBovkDVI7k8EEGsERaxEBk+HxCF4ACLwbMsMIJbIC4Dl0QCRA0qACIHzMgFF5GlYaDWQxEhs8oGHEAAITQNVQJ5ywJAAAAAElFTkSuQmCC>
