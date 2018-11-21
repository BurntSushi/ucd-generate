use std::path::Path;
use std::str::FromStr;

use common::{
    UcdFile, UcdFileByCodepoint, Codepoints, CodepointIter,
    parse_codepoint_association,
};
use error::Error;

/// A single row in the `auxiliary/GraphemeBreakProperty.txt` file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GraphemeClusterBreak {
    /// The codepoint or codepoint range for this entry.
    pub codepoints: Codepoints,
    /// The property value assigned to the codepoints in this entry.
    pub value: String,
}

impl UcdFile for GraphemeClusterBreak {
    fn relative_file_path() -> &'static Path {
        Path::new("auxiliary/GraphemeBreakProperty.txt")
    }
}

impl UcdFileByCodepoint for GraphemeClusterBreak {
    fn codepoints(&self) -> CodepointIter {
        self.codepoints.into_iter()
    }
}

impl FromStr for GraphemeClusterBreak {
    type Err = Error;

    fn from_str(line: &str) -> Result<GraphemeClusterBreak, Error> {
        let (codepoints, value) = parse_codepoint_association(line)?;
        Ok(GraphemeClusterBreak {
            codepoints: codepoints,
            value: value.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::GraphemeClusterBreak;

    #[test]
    fn parse_single() {
        let line = "093B          ; SpacingMark # Mc       DEVANAGARI VOWEL SIGN OOE\n";
        let row: GraphemeClusterBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, 0x093B);
        assert_eq!(row.value, "SpacingMark");
    }

    #[test]
    fn parse_range() {
        let line = "1F1E6..1F1FF  ; Regional_Indicator # So  [26] REGIONAL INDICATOR SYMBOL LETTER A..REGIONAL INDICATOR SYMBOL LETTER Z\n";
        let row: GraphemeClusterBreak = line.parse().unwrap();
        assert_eq!(row.codepoints, (0x1F1E6, 0x1F1FF));
        assert_eq!(row.value, "Regional_Indicator");
    }
}
