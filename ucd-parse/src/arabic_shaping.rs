use std::fmt;
use std::path::Path;
use std::str::FromStr;

use regex::Regex;

use common::{Codepoint, CodepointIter, UcdFile, UcdFileByCodepoint};
use error::Error;

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
    pub joining_type: String,
    /// The "joining group" of this codepoint.
    pub joining_group: String,
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
            joining_type: caps["joining_type"].to_string(),
            joining_group: caps["joining_group"].to_string(),
        })
    }
}

impl fmt::Display for ArabicShaping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{};", self.codepoint)?;
        write!(f, "{};", self.schematic_name)?;
        write!(f, "{};", self.joining_type)?;
        write!(f, "{}", self.joining_group)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use common::Codepoint;

    use super::ArabicShaping;

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
                joining_type: s("U"),
                joining_group: s("No_Joining_Group")
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
                joining_type: s("D"),
                joining_group: s("FARSI YEH")
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
                joining_type: s("D"),
                joining_group: s("HANIFI ROHINGYA KINNA YA")
            }
        );
    }
}
