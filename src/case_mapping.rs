use std::collections::BTreeMap;

use ucd_parse::{SpecialCaseMapping, UcdFile, UnicodeData};

use crate::args::ArgMatches;
use crate::error::Result;

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let mut lower_map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut upper_map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut title_map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut wtr = args.writer("case_mapping")?;
    for item in UnicodeData::from_dir(dir)? {
        let item = item?;
        if let Some(lower) = item.simple_lowercase_mapping {
            lower_map.insert(item.codepoint.value(), vec![lower.value()]);
        }
        if let Some(upper) = item.simple_uppercase_mapping {
            upper_map.insert(item.codepoint.value(), vec![upper.value()]);
        }
        if let Some(title) = item.simple_titlecase_mapping {
            title_map.insert(item.codepoint.value(), vec![title.value()]);
        }
    }
    if args.is_present("simple") {
        let upper_map =
            upper_map.into_iter().map(|(k, v)| (k, v[0])).collect();
        let lower_map =
            lower_map.into_iter().map(|(k, v)| (k, v[0])).collect();
        let title_map =
            title_map.into_iter().map(|(k, v)| (k, v[0])).collect();
        wtr.codepoint_to_codepoint("LOWER", &upper_map)?;
        wtr.codepoint_to_codepoint("UPPER", &lower_map)?;
        wtr.codepoint_to_codepoint("TITLE", &title_map)?;
    } else {
        for special in SpecialCaseMapping::from_dir(&dir)? {
            let special = special?;
            if !special.conditions.is_empty() {
                // There should probably be an option to output these too, but
                // I'm not sure how they're typically used...
                continue;
            }
            if !special.lowercase.is_empty() {
                lower_map.insert(
                    special.codepoint.value(),
                    special.lowercase.iter().map(|v| v.value()).collect(),
                );
            }
            if !special.uppercase.is_empty() {
                upper_map.insert(
                    special.codepoint.value(),
                    special.uppercase.iter().map(|v| v.value()).collect(),
                );
            }
            if !special.titlecase.is_empty() {
                title_map.insert(
                    special.codepoint.value(),
                    special.titlecase.iter().map(|v| v.value()).collect(),
                );
            }
        }
        let flat = args.is_present("flat-table");
        wtr.codepoint_to_codepoints("LOWER", &lower_map, flat)?;
        wtr.codepoint_to_codepoints("UPPER", &upper_map, flat)?;
        wtr.codepoint_to_codepoints("TITLE", &upper_map, flat)?;
    }
    Ok(())
}
