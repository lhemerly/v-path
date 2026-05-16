use std::{cmp::Ordering, error::Error, fmt, str::FromStr};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// The twelve pitch classes in 12-tone equal temperament.
///
/// Enharmonic spellings are normalized to a single canonical variant so the
/// engine can compare, transpose, and deduplicate pitches cheaply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PitchClass {
    C = 0,
    CSharp = 1,
    D = 2,
    DSharp = 3,
    E = 4,
    F = 5,
    FSharp = 6,
    G = 7,
    GSharp = 8,
    A = 9,
    ASharp = 10,
    B = 11,
}

impl PitchClass {
    pub const COUNT: u8 = 12;

    /// Converts any signed semitone value to its octave-equivalent pitch class.
    pub const fn from_semitones(semitones: i16) -> Self {
        match semitones.rem_euclid(Self::COUNT as i16) as u8 {
            0 => Self::C,
            1 => Self::CSharp,
            2 => Self::D,
            3 => Self::DSharp,
            4 => Self::E,
            5 => Self::F,
            6 => Self::FSharp,
            7 => Self::G,
            8 => Self::GSharp,
            9 => Self::A,
            10 => Self::ASharp,
            _ => Self::B,
        }
    }

    pub const fn semitone(self) -> u8 {
        self as u8
    }

    pub const fn transpose(self, interval: Interval) -> Self {
        Self::from_semitones(self.semitone() as i16 + interval.semitones() as i16)
    }
}

impl fmt::Display for PitchClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::C => "C",
            Self::CSharp => "C#",
            Self::D => "D",
            Self::DSharp => "D#",
            Self::E => "E",
            Self::F => "F",
            Self::FSharp => "F#",
            Self::G => "G",
            Self::GSharp => "G#",
            Self::A => "A",
            Self::ASharp => "A#",
            Self::B => "B",
        };
        f.write_str(name)
    }
}

impl Serialize for PitchClass {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PitchClass {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

impl FromStr for PitchClass {
    type Err = NoteError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "C" | "B#" => Ok(Self::C),
            "C#" | "Db" => Ok(Self::CSharp),
            "D" => Ok(Self::D),
            "D#" | "Eb" => Ok(Self::DSharp),
            "E" | "Fb" => Ok(Self::E),
            "F" | "E#" => Ok(Self::F),
            "F#" | "Gb" => Ok(Self::FSharp),
            "G" => Ok(Self::G),
            "G#" | "Ab" => Ok(Self::GSharp),
            "A" => Ok(Self::A),
            "A#" | "Bb" => Ok(Self::ASharp),
            "B" | "Cb" => Ok(Self::B),
            other => Err(NoteError::InvalidPitchClass(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteError {
    InvalidPitchClass(String),
    OctaveOutOfRange { octave: i8 },
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPitchClass(value) => write!(f, "invalid pitch class: {value}"),
            Self::OctaveOutOfRange { octave } => {
                write!(f, "octave {octave} is outside the supported range 0..=8")
            }
        }
    }
}

impl Error for NoteError {}

/// An absolute note with a pitch class and scientific-pitch octave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    pitch_class: PitchClass,
    octave: i8,
}

impl Note {
    pub const MIN_OCTAVE: i8 = 0;
    pub const MAX_OCTAVE: i8 = 8;

    pub const fn new(pitch_class: PitchClass, octave: i8) -> Result<Self, NoteError> {
        if octave < Self::MIN_OCTAVE || octave > Self::MAX_OCTAVE {
            return Err(NoteError::OctaveOutOfRange { octave });
        }

        Ok(Self {
            pitch_class,
            octave,
        })
    }

    pub const fn pitch_class(self) -> PitchClass {
        self.pitch_class
    }

    pub const fn octave(self) -> i8 {
        self.octave
    }

    pub const fn transpose(self, interval: Interval) -> Result<Self, NoteError> {
        let total = self.midi_number() as u16 + interval.semitones() as u16;
        let octave = (total / PitchClass::COUNT as u16) as i8 - 1;
        Self::new(PitchClass::from_semitones(total as i16), octave)
    }

    /// MIDI note number using C4 = 60.
    pub const fn midi_number(self) -> u8 {
        ((self.octave + 1) as u8 * PitchClass::COUNT) + self.pitch_class.semitone()
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.pitch_class, self.octave)
    }
}

impl Ord for Note {
    fn cmp(&self, other: &Self) -> Ordering {
        self.midi_number().cmp(&other.midi_number())
    }
}

impl PartialOrd for Note {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalError {
    TooLarge { semitones: u8 },
}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { semitones } => write!(
                f,
                "interval of {semitones} semitones exceeds the two-octave limit"
            ),
        }
    }
}

impl Error for IntervalError {}

/// A non-negative interval capped at two octaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Interval(u8);

impl Interval {
    pub const MAX_SEMITONES: u8 = 24;

    pub const UNISON: Self = Self(0);
    pub const MINOR_SECOND: Self = Self(1);
    pub const MAJOR_SECOND: Self = Self(2);
    pub const MINOR_THIRD: Self = Self(3);
    pub const MAJOR_THIRD: Self = Self(4);
    pub const PERFECT_FOURTH: Self = Self(5);
    pub const TRITONE: Self = Self(6);
    pub const PERFECT_FIFTH: Self = Self(7);
    pub const MINOR_SIXTH: Self = Self(8);
    pub const MAJOR_SIXTH: Self = Self(9);
    pub const MINOR_SEVENTH: Self = Self(10);
    pub const MAJOR_SEVENTH: Self = Self(11);
    pub const OCTAVE: Self = Self(12);

    pub const fn new(semitones: u8) -> Result<Self, IntervalError> {
        if semitones > Self::MAX_SEMITONES {
            return Err(IntervalError::TooLarge { semitones });
        }

        Ok(Self(semitones))
    }

    pub const fn semitones(self) -> u8 {
        self.0
    }
}

/// Common chord formulas supported by the MVP engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
    DominantSeventh,
    MajorSeventh,
    MinorSeventh,
}

impl ChordQuality {
    pub const fn intervals(self) -> &'static [Interval] {
        match self {
            Self::Major => &[
                Interval::UNISON,
                Interval::MAJOR_THIRD,
                Interval::PERFECT_FIFTH,
            ],
            Self::Minor => &[
                Interval::UNISON,
                Interval::MINOR_THIRD,
                Interval::PERFECT_FIFTH,
            ],
            Self::Diminished => &[Interval::UNISON, Interval::MINOR_THIRD, Interval::TRITONE],
            Self::Augmented => &[
                Interval::UNISON,
                Interval::MAJOR_THIRD,
                Interval::MINOR_SIXTH,
            ],
            Self::DominantSeventh => &[
                Interval::UNISON,
                Interval::MAJOR_THIRD,
                Interval::PERFECT_FIFTH,
                Interval::MINOR_SEVENTH,
            ],
            Self::MajorSeventh => &[
                Interval::UNISON,
                Interval::MAJOR_THIRD,
                Interval::PERFECT_FIFTH,
                Interval::MAJOR_SEVENTH,
            ],
            Self::MinorSeventh => &[
                Interval::UNISON,
                Interval::MINOR_THIRD,
                Interval::PERFECT_FIFTH,
                Interval::MINOR_SEVENTH,
            ],
        }
    }
}

/// A chord as a root pitch class and typed quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Chord {
    root: PitchClass,
    quality: ChordQuality,
}

impl Chord {
    pub const fn new(root: PitchClass, quality: ChordQuality) -> Self {
        Self { root, quality }
    }

    pub const fn root(self) -> PitchClass {
        self.root
    }

    pub const fn quality(self) -> ChordQuality {
        self.quality
    }

    pub fn pitch_classes(self) -> Vec<PitchClass> {
        self.quality
            .intervals()
            .iter()
            .map(|interval| self.root.transpose(*interval))
            .collect()
    }
}

/// Scale formulas supported by the MVP engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleKind {
    Major,
    NaturalMinor,
    HarmonicMinor,
    MajorPentatonic,
    MinorPentatonic,
}

impl ScaleKind {
    pub const fn intervals(self) -> &'static [Interval] {
        match self {
            Self::Major => &[
                Interval::UNISON,
                Interval::MAJOR_SECOND,
                Interval::MAJOR_THIRD,
                Interval::PERFECT_FOURTH,
                Interval::PERFECT_FIFTH,
                Interval::MAJOR_SIXTH,
                Interval::MAJOR_SEVENTH,
            ],
            Self::NaturalMinor => &[
                Interval::UNISON,
                Interval::MAJOR_SECOND,
                Interval::MINOR_THIRD,
                Interval::PERFECT_FOURTH,
                Interval::PERFECT_FIFTH,
                Interval::MINOR_SIXTH,
                Interval::MINOR_SEVENTH,
            ],
            Self::HarmonicMinor => &[
                Interval::UNISON,
                Interval::MAJOR_SECOND,
                Interval::MINOR_THIRD,
                Interval::PERFECT_FOURTH,
                Interval::PERFECT_FIFTH,
                Interval::MINOR_SIXTH,
                Interval::MAJOR_SEVENTH,
            ],
            Self::MajorPentatonic => &[
                Interval::UNISON,
                Interval::MAJOR_SECOND,
                Interval::MAJOR_THIRD,
                Interval::PERFECT_FIFTH,
                Interval::MAJOR_SIXTH,
            ],
            Self::MinorPentatonic => &[
                Interval::UNISON,
                Interval::MINOR_THIRD,
                Interval::PERFECT_FOURTH,
                Interval::PERFECT_FIFTH,
                Interval::MINOR_SEVENTH,
            ],
        }
    }
}

/// A scale as a tonic pitch class and a typed scale formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scale {
    tonic: PitchClass,
    kind: ScaleKind,
}

impl Scale {
    pub const fn new(tonic: PitchClass, kind: ScaleKind) -> Self {
        Self { tonic, kind }
    }

    pub const fn tonic(self) -> PitchClass {
        self.tonic
    }

    pub const fn kind(self) -> ScaleKind {
        self.kind
    }

    pub fn pitch_classes(self) -> Vec<PitchClass> {
        self.kind
            .intervals()
            .iter()
            .map(|interval| self.tonic.transpose(*interval))
            .collect()
    }

    pub fn contains(self, pitch_class: PitchClass) -> bool {
        self.pitch_classes().contains(&pitch_class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_class_parses_enharmonic_spellings() {
        assert_eq!("Bb".parse::<PitchClass>(), Ok(PitchClass::ASharp));
        assert_eq!("E#".parse::<PitchClass>(), Ok(PitchClass::F));
        assert!("H".parse::<PitchClass>().is_err());
    }

    #[test]
    fn note_rejects_octaves_outside_supported_range() {
        assert!(Note::new(PitchClass::C, -1).is_err());
        assert_eq!(
            Note::new(PitchClass::C, 4)
                .expect("C4 should be valid")
                .midi_number(),
            60
        );
    }

    #[test]
    fn notes_sort_by_absolute_pitch_not_pitch_class() {
        let c_sharp_zero = Note::new(PitchClass::CSharp, 0).expect("C#0 should be valid");
        let c_five = Note::new(PitchClass::C, 5).expect("C5 should be valid");

        assert!(c_sharp_zero < c_five);
        assert_eq!(c_sharp_zero.cmp(&c_five), Ordering::Less);
    }

    #[test]
    fn interval_rejects_values_larger_than_two_octaves() {
        assert!(Interval::new(25).is_err());
        assert_eq!(Interval::new(24), Ok(Interval::new(24).unwrap()));
    }

    #[test]
    fn chord_expands_to_pitch_classes() {
        let chord = Chord::new(PitchClass::D, ChordQuality::MinorSeventh);
        assert_eq!(
            chord.pitch_classes(),
            vec![PitchClass::D, PitchClass::F, PitchClass::A, PitchClass::C]
        );
    }

    #[test]
    fn scale_expands_to_pitch_classes_and_checks_membership() {
        let scale = Scale::new(PitchClass::G, ScaleKind::Major);
        assert_eq!(
            scale.pitch_classes(),
            vec![
                PitchClass::G,
                PitchClass::A,
                PitchClass::B,
                PitchClass::C,
                PitchClass::D,
                PitchClass::E,
                PitchClass::FSharp,
            ]
        );
        assert!(scale.contains(PitchClass::FSharp));
        assert!(!scale.contains(PitchClass::F));
    }
}
