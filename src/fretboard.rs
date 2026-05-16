use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{Interval, Note, PitchClass};

/// Number of strings on the MVP guitar fretboard.
pub const STRING_COUNT: u8 = 6;

/// Open-string coordinates use conventional guitar string numbers: 1 is the
/// highest-pitched string and 6 is the lowest-pitched string.
pub const MIN_STRING: u8 = 1;
pub const MAX_STRING: u8 = STRING_COUNT;

/// The MVP fretboard includes the open string and frets 1 through 24.
pub const MIN_FRET: u8 = 0;
pub const MAX_FRET: u8 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FretboardError {
    StringOutOfRange { string: u8 },
    FretOutOfRange { fret: u8 },
}

impl fmt::Display for FretboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringOutOfRange { string } => write!(
                f,
                "string {string} is outside the supported range {MIN_STRING}..={MAX_STRING}"
            ),
            Self::FretOutOfRange { fret } => write!(
                f,
                "fret {fret} is outside the supported range {MIN_FRET}..={MAX_FRET}"
            ),
        }
    }
}

impl Error for FretboardError {}

/// A Cartesian coordinate on the MVP fretboard grid.
///
/// `string` uses conventional guitar numbering (`1` = high E, `6` = low E).
/// `fret` is zero for an open string and otherwise the physical fret number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "RawPosition", into = "RawPosition")]
pub struct Position {
    string: u8,
    fret: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RawPosition {
    string: u8,
    fret: u8,
}

impl TryFrom<RawPosition> for Position {
    type Error = FretboardError;

    fn try_from(value: RawPosition) -> Result<Self, Self::Error> {
        Self::new(value.string, value.fret)
    }
}

impl From<Position> for RawPosition {
    fn from(value: Position) -> Self {
        Self {
            string: value.string,
            fret: value.fret,
        }
    }
}

impl Position {
    pub const fn new(string: u8, fret: u8) -> Result<Self, FretboardError> {
        if string == 0 || string > MAX_STRING {
            return Err(FretboardError::StringOutOfRange { string });
        }

        if fret > MAX_FRET {
            return Err(FretboardError::FretOutOfRange { fret });
        }

        Ok(Self { string, fret })
    }

    pub const fn string(self) -> u8 {
        self.string
    }

    pub const fn fret(self) -> u8 {
        self.fret
    }
}

/// A 6-string, 24-fret guitar fretboard in standard tuning only.
///
/// The MVP deliberately has no alternate-tuning constructor. Open strings are
/// EADGBE from low to high pitch, represented with conventional string numbers:
/// string 6 = E2, string 5 = A2, string 4 = D3, string 3 = G3, string 2 = B3,
/// and string 1 = E4.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fretboard;

impl Fretboard {
    pub const fn standard() -> Self {
        Self
    }

    pub fn open_note(self, string: u8) -> Result<Note, FretboardError> {
        validate_string(string)?;
        Ok(standard_open_note(string))
    }

    pub fn note_at(self, position: Position) -> Note {
        standard_open_note(position.string)
            .transpose(Interval::new(position.fret).expect("validated fret is <= 24 semitones"))
            .expect("standard 24-fret guitar range fits supported note octaves")
    }

    pub fn all_positions(self) -> Vec<Position> {
        (MIN_STRING..=MAX_STRING)
            .flat_map(|string| {
                (MIN_FRET..=MAX_FRET).map(move |fret| {
                    Position::new(string, fret)
                        .expect("generated fretboard coordinates should be valid")
                })
            })
            .collect()
    }

    pub fn positions_for_pitch_class(self, pitch_class: PitchClass) -> Vec<Position> {
        self.all_positions()
            .into_iter()
            .filter(|position| self.note_at(*position).pitch_class() == pitch_class)
            .collect()
    }
}

fn validate_string(string: u8) -> Result<(), FretboardError> {
    if !(MIN_STRING..=MAX_STRING).contains(&string) {
        return Err(FretboardError::StringOutOfRange { string });
    }

    Ok(())
}

fn standard_open_note(string: u8) -> Note {
    match string {
        1 => Note::new(PitchClass::E, 4),
        2 => Note::new(PitchClass::B, 3),
        3 => Note::new(PitchClass::G, 3),
        4 => Note::new(PitchClass::D, 3),
        5 => Note::new(PitchClass::A, 2),
        6 => Note::new(PitchClass::E, 2),
        _ => unreachable!("string should be validated before looking up its open note"),
    }
    .expect("standard tuning open notes should be inside supported octave range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_rejects_coordinates_outside_six_by_twenty_four_grid() {
        assert_eq!(
            Position::new(0, 0),
            Err(FretboardError::StringOutOfRange { string: 0 })
        );
        assert_eq!(
            Position::new(7, 0),
            Err(FretboardError::StringOutOfRange { string: 7 })
        );
        assert_eq!(
            Position::new(1, 25),
            Err(FretboardError::FretOutOfRange { fret: 25 })
        );
        assert_eq!(
            Position::new(6, 24)
                .expect("string 6 fret 24 should be valid")
                .fret(),
            24
        );
    }

    #[test]
    fn standard_tuning_maps_open_strings_to_eadgbe() {
        let fretboard = Fretboard::standard();

        assert_eq!(
            fretboard.open_note(6),
            Ok(Note::new(PitchClass::E, 2).unwrap())
        );
        assert_eq!(
            fretboard.open_note(5),
            Ok(Note::new(PitchClass::A, 2).unwrap())
        );
        assert_eq!(
            fretboard.open_note(4),
            Ok(Note::new(PitchClass::D, 3).unwrap())
        );
        assert_eq!(
            fretboard.open_note(3),
            Ok(Note::new(PitchClass::G, 3).unwrap())
        );
        assert_eq!(
            fretboard.open_note(2),
            Ok(Note::new(PitchClass::B, 3).unwrap())
        );
        assert_eq!(
            fretboard.open_note(1),
            Ok(Note::new(PitchClass::E, 4).unwrap())
        );
    }

    #[test]
    fn frets_transpose_from_the_standard_tuning_open_note() {
        let fretboard = Fretboard::standard();
        let low_e_third_fret = Position::new(6, 3).expect("G on low E should be valid");
        let b_string_first_fret = Position::new(2, 1).expect("C on B string should be valid");
        let high_e_twelfth_fret = Position::new(1, 12).expect("E5 should be valid");

        assert_eq!(
            fretboard.note_at(low_e_third_fret),
            Note::new(PitchClass::G, 2).unwrap()
        );
        assert_eq!(
            fretboard.note_at(b_string_first_fret),
            Note::new(PitchClass::C, 4).unwrap()
        );
        assert_eq!(
            fretboard.note_at(high_e_twelfth_fret),
            Note::new(PitchClass::E, 5).unwrap()
        );
    }

    #[test]
    fn all_positions_returns_complete_cartesian_grid() {
        let positions = Fretboard::standard().all_positions();

        assert_eq!(positions.len(), 150);
        assert_eq!(
            positions.first().copied(),
            Some(Position::new(1, 0).unwrap())
        );
        assert_eq!(
            positions.last().copied(),
            Some(Position::new(6, 24).unwrap())
        );
        assert!(positions.contains(&Position::new(4, 12).unwrap()));
    }

    #[test]
    fn positions_for_pitch_class_finds_notes_across_the_grid() {
        let fretboard = Fretboard::standard();
        let c_positions = fretboard.positions_for_pitch_class(PitchClass::C);

        assert!(c_positions.contains(&Position::new(2, 1).unwrap()));
        assert!(c_positions.contains(&Position::new(5, 3).unwrap()));
        assert!(c_positions
            .iter()
            .all(|position| { fretboard.note_at(*position).pitch_class() == PitchClass::C }));
    }
}
