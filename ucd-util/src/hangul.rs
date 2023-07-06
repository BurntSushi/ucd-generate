// This implementation should correspond to the algorithms described in
// Unicode 3.12.

/// A set of ranges that corresponds to the set of all Hangul syllable
/// codepoints.
///
/// These ranges are defined in Unicode 4.8 Table 4-13.
pub const RANGE_HANGUL_SYLLABLE: &'static [(u32, u32)] = &[(0xAC00, 0xD7A3)];

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = 588;

/// Return the character name of the given precomposed Hangul codepoint.
///
/// If the given codepoint does not correspond to a precomposed Hangul
/// codepoint in the inclusive range `AC00..D7A3`, then this returns `None`.
///
/// This implements the algorithms described in Unicode 3.12 and Unicode 4.8.
///
/// The `table` given should be a map from codepoint to the corresponding
/// Jamo short name for that codepoint. If you're using `ucd-generate`, then
/// the table can be generated via the `jamo-short-name` sub-command.
pub fn hangul_name<'a>(
    table: &'a [(u32, &'a str)],
    cp: u32,
) -> Option<String> {
    let mut name = "HANGUL SYLLABLE ".to_string();
    let (lpart, vpart, tpart) = match hangul_full_canonical_decomposition(cp) {
        None => return None,
        Some(triple) => triple,
    };

    name.push_str(jamo_short_name(table, lpart));
    name.push_str(jamo_short_name(table, vpart));
    name.push_str(tpart.map_or("", |cp| jamo_short_name(table, cp)));
    Some(name)
}

/// Return the full canonical decomposition of the given precomposed Hangul
/// codepoint.
///
/// If the decomposition does not have any trailing consonant, then the third
/// part of the tuple returned is `None`.
///
/// If the given codepoint does not correspond to a precomposed Hangul
/// codepoint in the inclusive range `AC00..D7A3`, then this returns `None`.
///
/// This implements the algorithms described in Unicode 3.12 and Unicode 4.8.
pub fn hangul_full_canonical_decomposition(
    cp: u32,
) -> Option<(u32, u32, Option<u32>)> {
    if !(0xAC00 <= cp && cp <= 0xD7A3) {
        return None;
    }

    let s_index = cp - S_BASE;
    let l_index = s_index / N_COUNT;
    let v_index = (s_index % N_COUNT) / T_COUNT;
    let t_index = s_index % T_COUNT;

    let l_part = L_BASE + l_index;
    let v_part = V_BASE + v_index;
    let t_part = if t_index == 0 { None } else { Some(T_BASE + t_index) };
    Some((l_part, v_part, t_part))
}

type JamoShortName<'a> = &'a [(u32, &'a str)];

fn jamo_short_name<'a>(table: JamoShortName<'a>, cp: u32) -> &'a str {
    let i = table.binary_search_by_key(&cp, |p| p.0).unwrap();
    table[i].1
}

#[cfg(test)]
mod tests {
    use crate::unicode_tables::jamo_short_name::JAMO_SHORT_NAME as TABLE;

    use super::{hangul_full_canonical_decomposition, hangul_name};

    #[test]
    fn canon_decomp() {
        assert_eq!(
            hangul_full_canonical_decomposition(0xD4DB),
            Some((0x1111, 0x1171, Some(0x11B6)))
        );
    }

    #[test]
    fn name() {
        assert_eq!(
            hangul_name(TABLE, 0xD4DB).unwrap(),
            "HANGUL SYLLABLE PWILH"
        );
    }

    #[test]
    fn all() {
        for cp in 0xAC00..(0xD7A3 + 1) {
            hangul_name(TABLE, cp).unwrap();
        }
    }

    #[test]
    fn invalid() {
        assert!(hangul_name(TABLE, 0).is_none());
    }
}
