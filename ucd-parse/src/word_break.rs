use std::path::Path;
use std::str::FromStr;

use common::{
    UcdFile, UcdFileByCodepoint, Codepoints, CodepointIter,
    parse_codepoint_association,
};
use error::Error;

/// A single row in the `auxiliary/WordBreakProperty.txt` file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WordBreak {
    /// The codepoint or codepoint range for this entry.
    pub codepoints: Codepoints,
    /// The property value assigned to the codepoints in this entry.
    pub value: String,
}

impl UcdFile for WordBreak {
    fn relative_file_path() -> &'static Path {
        Path::new("auxiliary/WordBreakProperty.txt")
    }
}

impl UcdFileByCodepoint for WordBreak {
    fn codepoints(&self) -> CodepointIter {
        self.codepoints.into_iter()
    }
}

impl FromStr for WordBreak {
    type Err = Error;

    fn from_str(line: &str) -> Result<WordBreak, Error> {
        let (codepoints, value) = parse_codepoint_association(line)?;
        Ok(WordBreak {
            codepoints: codepoints,
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::WordBreak;

    #[test]
    fn parse_single() {
        let line = "0A83          ; Extend # Mc       GUJARATI SIGN VISARGA\n";
        let row: WordBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, 0x0A83);
        assert_eq!(row.value, "Extend");
    }

    #[test]
    fn parse_range() {
        let line = "104A0..104A9  ; Numeric # Nd  [10] OSMANYA DIGIT ZERO..OSMANYA DIGIT NINE\n";
        let row: WordBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, (0x104A0, 0x104A9));
        assert_eq!(row.value, "Numeric");
    }
}
