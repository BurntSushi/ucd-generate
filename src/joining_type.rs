use std::collections::{BTreeMap, BTreeSet};

use ucd_parse::{self, ArabicShaping};

use crate::args::ArgMatches;
use crate::error::Result;
use crate::general_category;
use crate::util::PropertyValues;

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let propvals = PropertyValues::from_ucd_dir(&dir)?;
    let rows: Vec<ArabicShaping> = ucd_parse::parse(&dir)?;
    let unexpanded_gc = ucd_parse::parse(&dir)?;
    let gc =
        general_category::expand_into_categories(unexpanded_gc, &propvals)?;

    // Collect each joining type into an ordered set.
    let mut by_type: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    for row in rows {
        assigned.insert(row.codepoint.value());
        let jt =
            propvals.canonical("jt", row.joining_type.as_str())?.to_string();
        by_type
            .entry(jt)
            .or_insert(BTreeSet::new())
            .insert(row.codepoint.value());
    }
    // Process the codepoints that are not listed as per the note in
    // ArabicShaping.txt:
    //
    // Note: Code points that are not explicitly listed in this file are either
    // of joining type T or U:
    //
    // - Those that are not explicitly listed and that are of General Category
    //   Mn, Me, or Cf have joining type T.
    // - All others not explicitly listed have joining type U.
    let transparent_name = propvals.canonical("jt", "transparent")?;
    let non_joining_name = propvals.canonical("jt", "non_joining")?;
    let transparent_categories = ["Mn", "Me", "Cf"]
        .iter()
        .map(|cat| propvals.canonical("gc", cat).map(|name| &gc[&name]))
        .collect::<Result<Vec<_>>>()?;
    for cp in 0..=0x10FFFF {
        if assigned.contains(&cp) {
            continue;
        }
        // See if the code point is in any of the general categories that
        // map to the Transparent joining type. Otherwise add to the
        // Non_Joining type.
        if transparent_categories.iter().any(|cat| cat.contains(&cp)) {
            by_type.get_mut(&transparent_name).unwrap().insert(cp);
        } else {
            by_type.get_mut(&non_joining_name).unwrap().insert(cp);
        }
    }

    let mut wtr = args.writer("joining_type")?;
    if args.is_present("enum") {
        wtr.ranges_to_enum(args.name(), &by_type)?;
    } else if args.is_present("rust-enum") {
        let variants = by_type.keys().map(String::as_str).collect::<Vec<_>>();
        wtr.ranges_to_rust_enum(args.name(), &variants, &by_type)?;
    } else {
        wtr.names(by_type.keys())?;
        for (name, set) in by_type {
            wtr.ranges(&name, &set)?;
        }
    }

    Ok(())
}
