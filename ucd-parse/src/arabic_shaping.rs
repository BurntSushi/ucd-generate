use std::path::Path;
use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;

use crate::common::{Codepoint, CodepointIter, UcdFile, UcdFileByCodepoint};
use crate::error::Error;

/// Represents a single row in the `ArabicShaping.txt` file.
///
/// The field names were taken from the header of ArabicShaping.txt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArabicShaping {
    /// The codepoint corresponding to this row.
    pub codepoint: Codepoint,
    /// A short schematic name for the codepoint.
    ///
    /// The schematic name is descriptive of the shape, based as consistently as
    /// possible on a name for the skeleton and then the diacritic marks applied
    /// to the skeleton, if any.  Note that this schematic name is considered a
    /// comment, and does not constitute a formal property value.
    pub schematic_name: String,
    /// The "joining type" of this codepoint.
    pub joining_type: JoiningType,
    /// The "joining group" of this codepoint.
    pub joining_group: JoiningGroup,
}

/// The Joining_Group field read from ArabicShaping.txt
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoiningGroup {
    AfricanFeh,
    AfricanNoon,
    AfricanQaf,
    Ain,
    Alaph,
    Alef,
    Beh,
    Beth,
    BurushaskiYehBarree,
    Dal,
    DalathRish,
    E,
    FarsiYeh,
    Fe,
    Feh,
    FinalSemkath,
    Gaf,
    Gamal,
    Hah,
    HanifiRohingyaKinnaYa,
    HanifiRohingyaPa,
    He,
    Heh,
    HehGoal,
    Heth,
    Kaf,
    Kaph,
    Khaph,
    KnottedHeh,
    Lam,
    Lamadh,
    MalayalamBha,
    MalayalamJa,
    MalayalamLla,
    MalayalamLlla,
    MalayalamNga,
    MalayalamNna,
    MalayalamNnna,
    MalayalamNya,
    MalayalamRa,
    MalayalamSsa,
    MalayalamTta,
    ManichaeanAleph,
    ManichaeanAyin,
    ManichaeanBeth,
    ManichaeanDaleth,
    ManichaeanDhamedh,
    ManichaeanFive,
    ManichaeanGimel,
    ManichaeanHeth,
    ManichaeanHundred,
    ManichaeanKaph,
    ManichaeanLamedh,
    ManichaeanMem,
    ManichaeanNun,
    ManichaeanOne,
    ManichaeanPe,
    ManichaeanQoph,
    ManichaeanResh,
    ManichaeanSadhe,
    ManichaeanSamekh,
    ManichaeanTaw,
    ManichaeanTen,
    ManichaeanTeth,
    ManichaeanThamedh,
    ManichaeanTwenty,
    ManichaeanWaw,
    ManichaeanYodh,
    ManichaeanZayin,
    Meem,
    Mim,
    NoJoiningGroup,
    Noon,
    Nun,
    Nya,
    Pe,
    Qaf,
    Qaph,
    Reh,
    ReversedPe,
    RohingyaYeh,
    Sad,
    Sadhe,
    Seen,
    Semkath,
    Shin,
    StraightWaw,
    SwashKaf,
    SyriacWaw,
    Tah,
    Taw,
    TehMarbuta,
    TehMarbutaGoal,
    Teth,
    Waw,
    Yeh,
    YehBarree,
    YehWithTail,
    Yudh,
    YudhHe,
    Zain,
    Zhain,
}

/// The Joining_Type field read from ArabicShaping.txt
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoiningType {
    RightJoining,
    LeftJoining,
    DualJoining,
    JoinCausing,
    NonJoining,
    Transparent,
}

impl JoiningGroup {
    pub fn as_str(&self) -> &str {
        match self {
            JoiningGroup::AfricanFeh => "African_Feh",
            JoiningGroup::AfricanNoon => "African_Noon",
            JoiningGroup::AfricanQaf => "African_Qaf",
            JoiningGroup::Ain => "Ain",
            JoiningGroup::Alaph => "Alaph",
            JoiningGroup::Alef => "Alef",
            JoiningGroup::Beh => "Beh",
            JoiningGroup::Beth => "Beth",
            JoiningGroup::BurushaskiYehBarree => "Burushaski_Yeh_Barree",
            JoiningGroup::Dal => "Dal",
            JoiningGroup::DalathRish => "Dalath_Rish",
            JoiningGroup::E => "E",
            JoiningGroup::FarsiYeh => "Farsi_Yeh",
            JoiningGroup::Fe => "Fe",
            JoiningGroup::Feh => "Feh",
            JoiningGroup::FinalSemkath => "Final_Semkath",
            JoiningGroup::Gaf => "Gaf",
            JoiningGroup::Gamal => "Gamal",
            JoiningGroup::Hah => "Hah",
            JoiningGroup::HanifiRohingyaKinnaYa => "Hanifi_Rohingya_Kinna_Ya",
            JoiningGroup::HanifiRohingyaPa => "Hanifi_Rohingya_Pa",
            JoiningGroup::He => "He",
            JoiningGroup::Heh => "Heh",
            JoiningGroup::HehGoal => "Heh_Goal",
            JoiningGroup::Heth => "Heth",
            JoiningGroup::Kaf => "Kaf",
            JoiningGroup::Kaph => "Kaph",
            JoiningGroup::Khaph => "Khaph",
            JoiningGroup::KnottedHeh => "Knotted_Heh",
            JoiningGroup::Lam => "Lam",
            JoiningGroup::Lamadh => "Lamadh",
            JoiningGroup::MalayalamBha => "Malayalam_Bha",
            JoiningGroup::MalayalamJa => "Malayalam_Ja",
            JoiningGroup::MalayalamLla => "Malayalam_Lla",
            JoiningGroup::MalayalamLlla => "Malayalam_Llla",
            JoiningGroup::MalayalamNga => "Malayalam_Nga",
            JoiningGroup::MalayalamNna => "Malayalam_Nna",
            JoiningGroup::MalayalamNnna => "Malayalam_Nnna",
            JoiningGroup::MalayalamNya => "Malayalam_Nya",
            JoiningGroup::MalayalamRa => "Malayalam_Ra",
            JoiningGroup::MalayalamSsa => "Malayalam_Ssa",
            JoiningGroup::MalayalamTta => "Malayalam_Tta",
            JoiningGroup::ManichaeanAleph => "Manichaean_Aleph",
            JoiningGroup::ManichaeanAyin => "Manichaean_Ayin",
            JoiningGroup::ManichaeanBeth => "Manichaean_Beth",
            JoiningGroup::ManichaeanDaleth => "Manichaean_Daleth",
            JoiningGroup::ManichaeanDhamedh => "Manichaean_Dhamedh",
            JoiningGroup::ManichaeanFive => "Manichaean_Five",
            JoiningGroup::ManichaeanGimel => "Manichaean_Gimel",
            JoiningGroup::ManichaeanHeth => "Manichaean_Heth",
            JoiningGroup::ManichaeanHundred => "Manichaean_Hundred",
            JoiningGroup::ManichaeanKaph => "Manichaean_Kaph",
            JoiningGroup::ManichaeanLamedh => "Manichaean_Lamedh",
            JoiningGroup::ManichaeanMem => "Manichaean_Mem",
            JoiningGroup::ManichaeanNun => "Manichaean_Nun",
            JoiningGroup::ManichaeanOne => "Manichaean_One",
            JoiningGroup::ManichaeanPe => "Manichaean_Pe",
            JoiningGroup::ManichaeanQoph => "Manichaean_Qoph",
            JoiningGroup::ManichaeanResh => "Manichaean_Resh",
            JoiningGroup::ManichaeanSadhe => "Manichaean_Sadhe",
            JoiningGroup::ManichaeanSamekh => "Manichaean_Samekh",
            JoiningGroup::ManichaeanTaw => "Manichaean_Taw",
            JoiningGroup::ManichaeanTen => "Manichaean_Ten",
            JoiningGroup::ManichaeanTeth => "Manichaean_Teth",
            JoiningGroup::ManichaeanThamedh => "Manichaean_Thamedh",
            JoiningGroup::ManichaeanTwenty => "Manichaean_Twenty",
            JoiningGroup::ManichaeanWaw => "Manichaean_Waw",
            JoiningGroup::ManichaeanYodh => "Manichaean_Yodh",
            JoiningGroup::ManichaeanZayin => "Manichaean_Zayin",
            JoiningGroup::Meem => "Meem",
            JoiningGroup::Mim => "Mim",
            JoiningGroup::NoJoiningGroup => "No_Joining_Group",
            JoiningGroup::Noon => "Noon",
            JoiningGroup::Nun => "Nun",
            JoiningGroup::Nya => "Nya",
            JoiningGroup::Pe => "Pe",
            JoiningGroup::Qaf => "Qaf",
            JoiningGroup::Qaph => "Qaph",
            JoiningGroup::Reh => "Reh",
            JoiningGroup::ReversedPe => "Reversed_Pe",
            JoiningGroup::RohingyaYeh => "Rohingya_Yeh",
            JoiningGroup::Sad => "Sad",
            JoiningGroup::Sadhe => "Sadhe",
            JoiningGroup::Seen => "Seen",
            JoiningGroup::Semkath => "Semkath",
            JoiningGroup::Shin => "Shin",
            JoiningGroup::StraightWaw => "Straight_Waw",
            JoiningGroup::SwashKaf => "Swash_Kaf",
            JoiningGroup::SyriacWaw => "Syriac_Waw",
            JoiningGroup::Tah => "Tah",
            JoiningGroup::Taw => "Taw",
            JoiningGroup::TehMarbuta => "Teh_Marbuta",
            JoiningGroup::TehMarbutaGoal => "Teh_Marbuta_Goal",
            JoiningGroup::Teth => "Teth",
            JoiningGroup::Waw => "Waw",
            JoiningGroup::Yeh => "Yeh",
            JoiningGroup::YehBarree => "Yeh_Barree",
            JoiningGroup::YehWithTail => "Yeh_With_Tail",
            JoiningGroup::Yudh => "Yudh",
            JoiningGroup::YudhHe => "Yudh_He",
            JoiningGroup::Zain => "Zain",
            JoiningGroup::Zhain => "Zhain",
        }
    }
}

impl FromStr for JoiningGroup {
    type Err = Error;

    fn from_str(s: &str) -> Result<JoiningGroup, Error> {
        match s {
            "African_Feh" => Ok(JoiningGroup::AfricanFeh),
            "African_Noon" => Ok(JoiningGroup::AfricanNoon),
            "African_Qaf" => Ok(JoiningGroup::AfricanQaf),
            "Ain" => Ok(JoiningGroup::Ain),
            "Alaph" => Ok(JoiningGroup::Alaph),
            "Alef" => Ok(JoiningGroup::Alef),
            "Beh" => Ok(JoiningGroup::Beh),
            "Beth" => Ok(JoiningGroup::Beth),
            "Burushaski_Yeh_Barree" => Ok(JoiningGroup::BurushaskiYehBarree),
            "Dal" => Ok(JoiningGroup::Dal),
            "Dalath_Rish" => Ok(JoiningGroup::DalathRish),
            "E" => Ok(JoiningGroup::E),
            "Farsi_Yeh" => Ok(JoiningGroup::FarsiYeh),
            "Fe" => Ok(JoiningGroup::Fe),
            "Feh" => Ok(JoiningGroup::Feh),
            "Final_Semkath" => Ok(JoiningGroup::FinalSemkath),
            "Gaf" => Ok(JoiningGroup::Gaf),
            "Gamal" => Ok(JoiningGroup::Gamal),
            "Hah" => Ok(JoiningGroup::Hah),
            "Hanifi_Rohingya_Kinna_Ya" => {
                Ok(JoiningGroup::HanifiRohingyaKinnaYa)
            }
            "Hanifi_Rohingya_Pa" => Ok(JoiningGroup::HanifiRohingyaPa),
            "He" => Ok(JoiningGroup::He),
            "Heh" => Ok(JoiningGroup::Heh),
            "Heh_Goal" => Ok(JoiningGroup::HehGoal),
            "Heth" => Ok(JoiningGroup::Heth),
            "Kaf" => Ok(JoiningGroup::Kaf),
            "Kaph" => Ok(JoiningGroup::Kaph),
            "Khaph" => Ok(JoiningGroup::Khaph),
            "Knotted_Heh" => Ok(JoiningGroup::KnottedHeh),
            "Lam" => Ok(JoiningGroup::Lam),
            "Lamadh" => Ok(JoiningGroup::Lamadh),
            "Malayalam_Bha" => Ok(JoiningGroup::MalayalamBha),
            "Malayalam_Ja" => Ok(JoiningGroup::MalayalamJa),
            "Malayalam_Lla" => Ok(JoiningGroup::MalayalamLla),
            "Malayalam_Llla" => Ok(JoiningGroup::MalayalamLlla),
            "Malayalam_Nga" => Ok(JoiningGroup::MalayalamNga),
            "Malayalam_Nna" => Ok(JoiningGroup::MalayalamNna),
            "Malayalam_Nnna" => Ok(JoiningGroup::MalayalamNnna),
            "Malayalam_Nya" => Ok(JoiningGroup::MalayalamNya),
            "Malayalam_Ra" => Ok(JoiningGroup::MalayalamRa),
            "Malayalam_Ssa" => Ok(JoiningGroup::MalayalamSsa),
            "Malayalam_Tta" => Ok(JoiningGroup::MalayalamTta),
            "Manichaean_Aleph" => Ok(JoiningGroup::ManichaeanAleph),
            "Manichaean_Ayin" => Ok(JoiningGroup::ManichaeanAyin),
            "Manichaean_Beth" => Ok(JoiningGroup::ManichaeanBeth),
            "Manichaean_Daleth" => Ok(JoiningGroup::ManichaeanDaleth),
            "Manichaean_Dhamedh" => Ok(JoiningGroup::ManichaeanDhamedh),
            "Manichaean_Five" => Ok(JoiningGroup::ManichaeanFive),
            "Manichaean_Gimel" => Ok(JoiningGroup::ManichaeanGimel),
            "Manichaean_Heth" => Ok(JoiningGroup::ManichaeanHeth),
            "Manichaean_Hundred" => Ok(JoiningGroup::ManichaeanHundred),
            "Manichaean_Kaph" => Ok(JoiningGroup::ManichaeanKaph),
            "Manichaean_Lamedh" => Ok(JoiningGroup::ManichaeanLamedh),
            "Manichaean_Mem" => Ok(JoiningGroup::ManichaeanMem),
            "Manichaean_Nun" => Ok(JoiningGroup::ManichaeanNun),
            "Manichaean_One" => Ok(JoiningGroup::ManichaeanOne),
            "Manichaean_Pe" => Ok(JoiningGroup::ManichaeanPe),
            "Manichaean_Qoph" => Ok(JoiningGroup::ManichaeanQoph),
            "Manichaean_Resh" => Ok(JoiningGroup::ManichaeanResh),
            "Manichaean_Sadhe" => Ok(JoiningGroup::ManichaeanSadhe),
            "Manichaean_Samekh" => Ok(JoiningGroup::ManichaeanSamekh),
            "Manichaean_Taw" => Ok(JoiningGroup::ManichaeanTaw),
            "Manichaean_Ten" => Ok(JoiningGroup::ManichaeanTen),
            "Manichaean_Teth" => Ok(JoiningGroup::ManichaeanTeth),
            "Manichaean_Thamedh" => Ok(JoiningGroup::ManichaeanThamedh),
            "Manichaean_Twenty" => Ok(JoiningGroup::ManichaeanTwenty),
            "Manichaean_Waw" => Ok(JoiningGroup::ManichaeanWaw),
            "Manichaean_Yodh" => Ok(JoiningGroup::ManichaeanYodh),
            "Manichaean_Zayin" => Ok(JoiningGroup::ManichaeanZayin),
            "Meem" => Ok(JoiningGroup::Meem),
            "Mim" => Ok(JoiningGroup::Mim),
            "No_Joining_Group" => Ok(JoiningGroup::NoJoiningGroup),
            "Noon" => Ok(JoiningGroup::Noon),
            "Nun" => Ok(JoiningGroup::Nun),
            "Nya" => Ok(JoiningGroup::Nya),
            "Pe" => Ok(JoiningGroup::Pe),
            "Qaf" => Ok(JoiningGroup::Qaf),
            "Qaph" => Ok(JoiningGroup::Qaph),
            "Reh" => Ok(JoiningGroup::Reh),
            "Reversed_Pe" => Ok(JoiningGroup::ReversedPe),
            "Rohingya_Yeh" => Ok(JoiningGroup::RohingyaYeh),
            "Sad" => Ok(JoiningGroup::Sad),
            "Sadhe" => Ok(JoiningGroup::Sadhe),
            "Seen" => Ok(JoiningGroup::Seen),
            "Semkath" => Ok(JoiningGroup::Semkath),
            "Shin" => Ok(JoiningGroup::Shin),
            "Straight_Waw" => Ok(JoiningGroup::StraightWaw),
            "Swash_Kaf" => Ok(JoiningGroup::SwashKaf),
            "Syriac_Waw" => Ok(JoiningGroup::SyriacWaw),
            "Tah" => Ok(JoiningGroup::Tah),
            "Taw" => Ok(JoiningGroup::Taw),
            "Teh_Marbuta" => Ok(JoiningGroup::TehMarbuta),
            "Teh_Marbuta_Goal" => Ok(JoiningGroup::TehMarbutaGoal),
            "Teth" => Ok(JoiningGroup::Teth),
            "Waw" => Ok(JoiningGroup::Waw),
            "Yeh" => Ok(JoiningGroup::Yeh),
            "Yeh_Barree" => Ok(JoiningGroup::YehBarree),
            "Yeh_With_Tail" => Ok(JoiningGroup::YehWithTail),
            "Yudh" => Ok(JoiningGroup::Yudh),
            "Yudh_He" => Ok(JoiningGroup::YudhHe),
            "Zain" => Ok(JoiningGroup::Zain),
            "Zhain" => Ok(JoiningGroup::Zhain),
            _ => err!("unrecognized joining group: '{}'", s),
        }
    }
}

impl Default for JoiningGroup {
    fn default() -> JoiningGroup {
        JoiningGroup::NoJoiningGroup
    }
}

impl JoiningType {
    pub fn as_str(&self) -> &str {
        match self {
            JoiningType::RightJoining => "R",
            JoiningType::LeftJoining => "L",
            JoiningType::DualJoining => "D",
            JoiningType::JoinCausing => "C",
            JoiningType::NonJoining => "U",
            JoiningType::Transparent => "T",
        }
    }
}

impl Default for JoiningType {
    fn default() -> JoiningType {
        JoiningType::NonJoining
    }
}

impl FromStr for JoiningType {
    type Err = Error;

    fn from_str(s: &str) -> Result<JoiningType, Error> {
        match s {
            "R" => Ok(JoiningType::RightJoining),
            "L" => Ok(JoiningType::LeftJoining),
            "D" => Ok(JoiningType::DualJoining),
            "C" => Ok(JoiningType::JoinCausing),
            "U" => Ok(JoiningType::NonJoining),
            "T" => Ok(JoiningType::Transparent),
            _ => err!(
                "unrecognized joining type: '{}' \
                 (must be one of R, L, D, C, U or T)",
                s
            ),
        }
    }
}

impl UcdFile for ArabicShaping {
    fn relative_file_path() -> &'static Path {
        Path::new("ArabicShaping.txt")
    }
}

impl UcdFileByCodepoint for ArabicShaping {
    fn codepoints(&self) -> CodepointIter {
        self.codepoint.into_iter()
    }
}

impl FromStr for ArabicShaping {
    type Err = Error;

    fn from_str(line: &str) -> Result<ArabicShaping, Error> {
        lazy_static! {
            static ref PARTS: Regex = Regex::new(
                r"(?x)
                ^
                \s*(?P<codepoint>[A-F0-9]+)\s*;
                \s*(?P<name>[^;]+)\s*;
                \s*(?P<joining_type>[^;]+)\s*;
                \s*(?P<joining_group>[^;]+)
                $
                "
            )
            .unwrap();
        };
        let caps = match PARTS.captures(line.trim()) {
            Some(caps) => caps,
            None => return err!("invalid ArabicShaping line"),
        };

        Ok(ArabicShaping {
            codepoint: caps["codepoint"].parse()?,
            schematic_name: caps["name"].to_string(),
            joining_type: caps["joining_type"].parse()?,
            joining_group: formal_name(&caps["joining_group"]).parse()?,
        })
    }
}

// For whatever reason the "formal" Joining_Group property name is not stored
// in the file. Instead the value is based on the schematic_name (all
// uppercase, space separated). This function transforms those into the formal
// form, as present in PropertyValueAliases.txt.
fn formal_name(s: &str) -> String {
    // Convert to Pascal_Snake_Case
    s.split(|c: char| c.is_whitespace() || c == '_')
        .map(|component| {
            // Upper first char
            let lower = component.to_ascii_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => {
                    f.to_uppercase().collect::<String>() + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use crate::common::Codepoint;

    use super::{ArabicShaping, JoiningType};
    use crate::arabic_shaping::JoiningGroup;

    fn codepoint(n: u32) -> Codepoint {
        Codepoint::from_u32(n).unwrap()
    }

    fn s(string: &str) -> String {
        string.to_string()
    }

    #[test]
    fn parse1() {
        let line = "0600; ARABIC NUMBER SIGN; U; No_Joining_Group\n";
        let data: ArabicShaping = line.parse().unwrap();
        assert_eq!(
            data,
            ArabicShaping {
                codepoint: codepoint(0x0600),
                schematic_name: s("ARABIC NUMBER SIGN"),
                joining_type: JoiningType::NonJoining,
                joining_group: JoiningGroup::NoJoiningGroup,
            }
        );
    }

    #[test]
    fn parse2() {
        let line = "063D; FARSI YEH WITH INVERTED V ABOVE; D; FARSI YEH\n";
        let data: ArabicShaping = line.parse().unwrap();
        assert_eq!(
            data,
            ArabicShaping {
                codepoint: codepoint(0x063D),
                schematic_name: s("FARSI YEH WITH INVERTED V ABOVE"),
                joining_type: JoiningType::DualJoining,
                joining_group: JoiningGroup::FarsiYeh,
            }
        );
    }

    #[test]
    fn parse3() {
        let line =
            "10D23; HANIFI ROHINGYA DOTLESS KINNA YA WITH DOT ABOVE; D; HANIFI ROHINGYA KINNA YA\n";
        let data: ArabicShaping = line.parse().unwrap();
        assert_eq!(
            data,
            ArabicShaping {
                codepoint: codepoint(0x10D23),
                schematic_name: s(
                    "HANIFI ROHINGYA DOTLESS KINNA YA WITH DOT ABOVE"
                ),
                joining_type: JoiningType::DualJoining,
                joining_group: JoiningGroup::HanifiRohingyaKinnaYa,
            }
        );
    }
}
