use std::collections::{BTreeMap, BTreeSet};

use ucd_parse::{self, ArabicShaping};

use crate::args::ArgMatches;
use crate::error::Result;
use crate::util::PropertyValues;

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let propvals = PropertyValues::from_ucd_dir(&dir)?;
    let rows: Vec<ArabicShaping> = ucd_parse::parse(&dir)?;

    // Collect each joining group into an ordered set.
    let mut by_group: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    for row in rows {
        assigned.insert(row.codepoint.value());
        let jg =
            propvals.canonical("jg", row.joining_group.as_str())?.to_string();
        by_group
            .entry(jg)
            .or_insert(BTreeSet::new())
            .insert(row.codepoint.value());
    }

    // Process unassigned codepoints
    let no_joining_group = propvals.canonical("jg", "no_joining_group")?;
    for cp in 0..=0x10FFFF {
        if assigned.contains(&cp) {
            continue;
        }
        by_group.get_mut(&no_joining_group).unwrap().insert(cp);
    }

    let mut wtr = args.writer("joining_group")?;
    if args.is_present("enum") {
        wtr.ranges_to_enum(args.name(), &by_group)?;
    } else if args.is_present("rust-enum") {
        let variants = by_group.keys().map(String::as_str).collect::<Vec<_>>();
        wtr.ranges_to_rust_enum(args.name(), &variants, &by_group)?;
    } else {
        wtr.names(by_group.keys())?;
        for (name, set) in by_group {
            wtr.ranges(&name, &set)?;
        }
    }

    Ok(())
}
