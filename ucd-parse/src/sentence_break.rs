use std::path::Path;
use std::str::FromStr;

use common::{
    UcdFile, UcdFileByCodepoint, Codepoints, CodepointIter,
    parse_codepoint_association,
};
use error::Error;

/// A single row in the `auxiliary/SentenceBreakProperty.txt` file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SentenceBreak {
    /// The codepoint or codepoint range for this entry.
    pub codepoints: Codepoints,
    /// The property value assigned to the codepoints in this entry.
    pub value: String,
}

impl UcdFile for SentenceBreak {
    fn relative_file_path() -> &'static Path {
        Path::new("auxiliary/SentenceBreakProperty.txt")
    }
}

impl UcdFileByCodepoint for SentenceBreak {
    fn codepoints(&self) -> CodepointIter {
        self.codepoints.into_iter()
    }
}

impl FromStr for SentenceBreak {
    type Err = Error;

    fn from_str(line: &str) -> Result<SentenceBreak, Error> {
        let (codepoints, value) = parse_codepoint_association(line)?;
        Ok(SentenceBreak {
            codepoints: codepoints,
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SentenceBreak;

    #[test]
    fn parse_single() {
        let line = "11445         ; Extend # Mc       NEWA SIGN VISARGA\n";
        let row: SentenceBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, 0x11445);
        assert_eq!(row.value, "Extend");
    }

    #[test]
    fn parse_range() {
        let line = "FE31..FE32    ; SContinue # Pd   [2] PRESENTATION FORM FOR VERTICAL EM DASH..PRESENTATION FORM FOR VERTICAL EN DASH\n";
        let row: SentenceBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, (0xFE31, 0xFE32));
        assert_eq!(row.value, "SContinue");
    }
}
