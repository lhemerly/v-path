use std::{collections::HashSet, error::Error, fmt};

use crate::{Chord, Fretboard, PitchClass, Position};

/// A fretting-hand finger that can hold an anchor note.
///
/// Finger numbers follow common guitar notation: `1` is index, `2` is middle,
/// `3` is ring, and `4` is pinky. Open strings are intentionally excluded from
/// anchor tracking because they are not held by the fretting hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Finger {
    Index = 1,
    Middle = 2,
    Ring = 3,
    Pinky = 4,
}

impl Finger {
    pub const fn number(self) -> u8 {
        self as u8
    }
}

/// A single anchored finger in a chord shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnchorFinger {
    finger: Finger,
    position: Position,
}

impl AnchorFinger {
    pub const fn new(finger: Finger, position: Position) -> Self {
        Self { finger, position }
    }

    pub const fn finger(self) -> Finger {
        self.finger
    }

    pub const fn position(self) -> Position {
        self.position
    }
}

/// Snapshot of the fretting hand while a chord is held.
///
/// The state stores only anchored fingers, not every sounding note in the chord.
/// This lets later biomechanical scoring ask which fingers should remain planted
/// while a riff leaves the current chord shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandState {
    chord: Chord,
    anchors: Vec<AnchorFinger>,
}

impl HandState {
    pub fn new(
        fretboard: Fretboard,
        chord: Chord,
        anchors: Vec<AnchorFinger>,
    ) -> Result<Self, HandStateError> {
        validate_anchors(fretboard, chord, &anchors)?;

        let mut anchors = anchors;
        anchors.sort_unstable();

        Ok(Self { chord, anchors })
    }

    pub const fn chord(&self) -> Chord {
        self.chord
    }

    pub fn anchors(&self) -> &[AnchorFinger] {
        &self.anchors
    }

    pub fn anchor_for_finger(&self, finger: Finger) -> Option<AnchorFinger> {
        self.anchors
            .iter()
            .copied()
            .find(|anchor| anchor.finger() == finger)
    }

    pub fn anchored_position(&self, position: Position) -> Option<AnchorFinger> {
        self.anchors
            .iter()
            .copied()
            .find(|anchor| anchor.position() == position)
    }

    pub fn anchored_pitch_classes(&self, fretboard: Fretboard) -> Vec<PitchClass> {
        self.anchors
            .iter()
            .map(|anchor| fretboard.note_at(anchor.position()).pitch_class())
            .collect()
    }
}

/// Small state machine for the current chord's fretting-hand anchors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandStateMachine {
    current: Option<HandState>,
}

impl HandStateMachine {
    pub const fn new() -> Self {
        Self { current: None }
    }

    pub const fn current(&self) -> Option<&HandState> {
        self.current.as_ref()
    }

    pub fn anchor_chord(
        &mut self,
        fretboard: Fretboard,
        chord: Chord,
        anchors: Vec<AnchorFinger>,
    ) -> Result<&HandState, HandStateError> {
        self.current = Some(HandState::new(fretboard, chord, anchors)?);
        Ok(self
            .current
            .as_ref()
            .expect("state was just set after validation"))
    }

    pub fn release(&mut self) -> Option<HandState> {
        self.current.take()
    }

    pub fn transition_to(
        &mut self,
        fretboard: Fretboard,
        chord: Chord,
        anchors: Vec<AnchorFinger>,
    ) -> Result<HandTransition, HandStateError> {
        let next = HandState::new(fretboard, chord, anchors)?;
        let previous = self.current.take();
        let retained_anchors = previous
            .as_ref()
            .map(|previous| retained_anchors(previous, &next))
            .unwrap_or_default();

        self.current = Some(next.clone());

        Ok(HandTransition {
            previous,
            current: next,
            retained_anchors,
        })
    }
}

/// Result of moving the hand state from one chord shape to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandTransition {
    previous: Option<HandState>,
    current: HandState,
    retained_anchors: Vec<AnchorFinger>,
}

impl HandTransition {
    pub fn previous(&self) -> Option<&HandState> {
        self.previous.as_ref()
    }

    pub const fn current(&self) -> &HandState {
        &self.current
    }

    pub fn retained_anchors(&self) -> &[AnchorFinger] {
        &self.retained_anchors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandStateError {
    NoAnchors,
    TooManyAnchors {
        count: usize,
    },
    DuplicateFinger {
        finger: Finger,
    },
    DuplicatePosition {
        position: Position,
    },
    OpenStringAnchor {
        position: Position,
    },
    AnchorOutsideChord {
        position: Position,
        pitch_class: PitchClass,
        chord: Chord,
    },
}

impl fmt::Display for HandStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAnchors => write!(f, "hand state requires at least one anchor finger"),
            Self::TooManyAnchors { count } => write!(
                f,
                "hand state has {count} anchors but only four fretting fingers are available"
            ),
            Self::DuplicateFinger { finger } => write!(
                f,
                "finger {} is assigned to more than one anchor",
                finger.number()
            ),
            Self::DuplicatePosition { position } => write!(
                f,
                "position string {} fret {} is assigned to more than one anchor",
                position.string(),
                position.fret()
            ),
            Self::OpenStringAnchor { position } => write!(
                f,
                "open string position string {} fret {} cannot be a fretting-hand anchor",
                position.string(),
                position.fret()
            ),
            Self::AnchorOutsideChord {
                position,
                pitch_class,
                chord,
            } => write!(
                f,
                "anchor at string {} fret {} sounds {pitch_class}, which is not in {} {}",
                position.string(),
                position.fret(),
                chord.root(),
                chord.quality()
            ),
        }
    }
}

impl Error for HandStateError {}

fn validate_anchors(
    fretboard: Fretboard,
    chord: Chord,
    anchors: &[AnchorFinger],
) -> Result<(), HandStateError> {
    if anchors.is_empty() {
        return Err(HandStateError::NoAnchors);
    }

    if anchors.len() > 4 {
        return Err(HandStateError::TooManyAnchors {
            count: anchors.len(),
        });
    }

    let chord_pitch_classes = chord.pitch_classes();
    let mut fingers = HashSet::new();
    let mut positions = HashSet::new();

    for anchor in anchors {
        if !fingers.insert(anchor.finger()) {
            return Err(HandStateError::DuplicateFinger {
                finger: anchor.finger(),
            });
        }

        if !positions.insert(anchor.position()) {
            return Err(HandStateError::DuplicatePosition {
                position: anchor.position(),
            });
        }

        if anchor.position().fret() == 0 {
            return Err(HandStateError::OpenStringAnchor {
                position: anchor.position(),
            });
        }

        let pitch_class = fretboard.note_at(anchor.position()).pitch_class();
        if !chord_pitch_classes.contains(&pitch_class) {
            return Err(HandStateError::AnchorOutsideChord {
                position: anchor.position(),
                pitch_class,
                chord,
            });
        }
    }

    Ok(())
}

fn retained_anchors(previous: &HandState, current: &HandState) -> Vec<AnchorFinger> {
    previous
        .anchors()
        .iter()
        .copied()
        .filter(|previous_anchor| {
            current.anchors().iter().any(|current_anchor| {
                current_anchor.finger() == previous_anchor.finger()
                    && current_anchor.position() == previous_anchor.position()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChordQuality, PitchClass};

    fn pos(string: u8, fret: u8) -> Position {
        Position::new(string, fret).expect("test position should be valid")
    }

    #[test]
    fn hand_state_tracks_sorted_anchor_fingers_for_current_chord() {
        let fretboard = Fretboard::standard();
        let d_major = Chord::new(PitchClass::D, ChordQuality::Major);

        let state = HandState::new(
            fretboard,
            d_major,
            vec![
                AnchorFinger::new(Finger::Ring, pos(2, 3)),
                AnchorFinger::new(Finger::Index, pos(3, 2)),
                AnchorFinger::new(Finger::Middle, pos(1, 2)),
            ],
        )
        .expect("D major anchor shape should be valid");

        assert_eq!(state.chord(), d_major);
        assert_eq!(
            state.anchors(),
            &[
                AnchorFinger::new(Finger::Index, pos(3, 2)),
                AnchorFinger::new(Finger::Middle, pos(1, 2)),
                AnchorFinger::new(Finger::Ring, pos(2, 3)),
            ]
        );
        assert_eq!(
            state.anchor_for_finger(Finger::Ring),
            Some(AnchorFinger::new(Finger::Ring, pos(2, 3)))
        );
        assert_eq!(
            state.anchored_pitch_classes(fretboard),
            vec![PitchClass::A, PitchClass::FSharp, PitchClass::D]
        );
    }

    #[test]
    fn state_machine_reports_retained_anchors_between_d_major_and_b_minor() {
        let fretboard = Fretboard::standard();
        let mut machine = HandStateMachine::new();

        machine
            .anchor_chord(
                fretboard,
                Chord::new(PitchClass::D, ChordQuality::Major),
                vec![
                    AnchorFinger::new(Finger::Index, pos(3, 2)),
                    AnchorFinger::new(Finger::Ring, pos(2, 3)),
                ],
            )
            .expect("D major anchors should be valid");

        let transition = machine
            .transition_to(
                fretboard,
                Chord::new(PitchClass::B, ChordQuality::Minor),
                vec![
                    AnchorFinger::new(Finger::Middle, pos(1, 2)),
                    AnchorFinger::new(Finger::Ring, pos(2, 3)),
                    AnchorFinger::new(Finger::Pinky, pos(5, 2)),
                ],
            )
            .expect("B minor anchors should be valid");

        assert_eq!(
            transition.retained_anchors(),
            &[AnchorFinger::new(Finger::Ring, pos(2, 3)),]
        );
        assert_eq!(
            machine
                .current()
                .expect("current state should be set")
                .chord(),
            Chord::new(PitchClass::B, ChordQuality::Minor)
        );
    }

    #[test]
    fn hand_state_rejects_invalid_anchor_assignments() {
        let fretboard = Fretboard::standard();
        let c_major = Chord::new(PitchClass::C, ChordQuality::Major);

        assert_eq!(
            HandState::new(fretboard, c_major, Vec::new()),
            Err(HandStateError::NoAnchors)
        );
        assert_eq!(
            HandState::new(
                fretboard,
                c_major,
                vec![
                    AnchorFinger::new(Finger::Index, pos(5, 3)),
                    AnchorFinger::new(Finger::Index, pos(4, 2)),
                ],
            ),
            Err(HandStateError::DuplicateFinger {
                finger: Finger::Index,
            })
        );
        assert_eq!(
            HandState::new(
                fretboard,
                c_major,
                vec![AnchorFinger::new(Finger::Index, pos(1, 0))],
            ),
            Err(HandStateError::OpenStringAnchor {
                position: pos(1, 0)
            })
        );
        let outside_chord_error = HandState::new(
            fretboard,
            c_major,
            vec![AnchorFinger::new(Finger::Index, pos(6, 2))],
        )
        .expect_err("F# should not be accepted as a C major anchor");

        assert_eq!(
            outside_chord_error,
            HandStateError::AnchorOutsideChord {
                position: pos(6, 2),
                pitch_class: PitchClass::FSharp,
                chord: c_major,
            }
        );
        assert_eq!(
            outside_chord_error.to_string(),
            "anchor at string 6 fret 2 sounds F#, which is not in C major"
        );
    }
}
