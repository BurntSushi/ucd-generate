use std::collections::{BTreeMap, BTreeSet};

use ucd_parse::{self, UnicodeData};

use crate::args::ArgMatches;
use crate::error::Result;
use crate::util::{print_property_values, PropertyValues};

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let propvals = PropertyValues::from_ucd_dir(&dir)?;
    let rows: Vec<UnicodeData> = ucd_parse::parse(&dir)?;
    let ccc_name = |ccc: u8| {
        propvals.canonical("canonicalcombiningclass", &ccc.to_string())
    };

    // If we were tasked with listing the available categories, then do that
    // and quit.
    if args.is_present("list-classes") {
        return print_property_values(&propvals, "Canonical_Combining_Class");
    }

    // Collect each canonical combining class into an ordered set.
    let mut by_name: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    for row in rows {
        assigned.insert(row.codepoint.value());
        let ccc = ccc_name(row.canonical_combining_class)?;
        by_name
            .entry(ccc)
            .or_insert(BTreeSet::new())
            .insert(row.codepoint.value());
    }

    // Process the codepoints that are not listed as per the note in
    // DerivedCombiningClass.txt (UCD 13.0):
    //
    // - All code points not explicitly listed for Canonical_Combining_Class
    //   have the value Not_Reordered (0).
    let not_reordered_name = ccc_name(0)?;
    for cp in 0..=0x10FFFF {
        if !assigned.contains(&cp) {
            by_name.get_mut(&not_reordered_name).unwrap().insert(cp);
        }
    }

    let mut wtr = args.writer("canonical_combining_class")?;
    if args.is_present("enum") {
        wtr.ranges_to_enum(args.name(), &by_name)?;
    } else if args.is_present("rust-enum") {
        let variants = by_name.keys().map(String::as_str).collect::<Vec<_>>();
        wtr.ranges_to_rust_enum(args.name(), &variants, &by_name)?;
    } else {
        wtr.names(by_name.keys())?;
        for (name, set) in by_name {
            wtr.ranges(&name, &set)?;
        }
    }

    Ok(())
}
