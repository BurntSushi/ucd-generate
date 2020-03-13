use std::collections::{BTreeMap, BTreeSet};

use ucd_parse::{self, CoreProperty, UnicodeData};

use crate::args::ArgMatches;
use crate::error::Result;
use crate::util::{print_property_values, PropertyValues};

// Bidi Class (listing UnicodeData.txt, field 4: see UAX #44:
// http://www.unicode.org/reports/tr44/) Unlike other properties, unassigned
// code points in blocks reserved for right-to-left scripts are given either
// types R or AL.
//
// The unassigned code points that default to AL are in the ranges:
//     [\u0600-\u07BF \u0860-\u086F \u08A0-\u08FF \uFB50-\uFDCF \uFDF0-\uFDFF
//     \uFE70-\uFEFF \U00010D00-\U00010D3F \U00010F30-\U00010F6F
//     \U0001EC70-\U0001ECBF \U0001ED00-\U0001ED4F \U0001EE00-\U0001EEFF]
//
//     This includes code points in the Arabic, Syriac, and Thaana blocks,
//     among others.
//
// The unassigned code points that default to R are in the ranges:
//     [\u0590-\u05FF \u07C0-\u085F \u0870-\u089F \uFB1D-\uFB4F
//     \U00010800-\U00010CFF \U00010D40-\U00010F2F \U00010F70-\U00010FFF
//     \U0001E800-\U0001EC6F \U0001ECC0-\U0001ECFF \U0001ED50-\U0001EDFF
//     \U0001EF00-\U0001EFFF]
//
//     This includes code points in the Hebrew, NKo, and Phoenician blocks,
//     among others.
//
// The unassigned code points that default to ET are in the range:
//     [\u20A0-\u20CF]
//
//     This consists of code points in the Currency Symbols block.
//
// The unassigned code points that default to BN have one of the following
// properties:
//     Default_Ignorable_Code_Point
//     Noncharacter_Code_Point
//
// For all other cases:
//
//  All code points not explicitly listed for Bidi_Class
//  have the value Left_To_Right (L).
const DEFAULT_CLASS_ASSIGNMENTS: &[(u32, u32, &str)] = &[
    (0x0600, 0x07BF, "AL"),
    (0x0860, 0x086F, "AL"),
    (0x08A0, 0x08FF, "AL"),
    (0xFB50, 0xFDCF, "AL"),
    (0xFDF0, 0xFDFF, "AL"),
    (0xFE70, 0xFEFF, "AL"),
    (0x00010D00, 0x00010D3F, "AL"),
    (0x00010F30, 0x00010F6F, "AL"),
    (0x0001EC70, 0x0001ECBF, "AL"),
    (0x0001ED00, 0x0001ED4F, "AL"),
    (0x0001EE00, 0x0001EEFF, "AL"),
    (0x0590, 0x05FF, "R"),
    (0x07C0, 0x085F, "R"),
    (0x0870, 0x089F, "R"),
    (0xFB1D, 0xFB4F, "R"),
    (0x00010800, 0x00010CFF, "R"),
    (0x00010D40, 0x00010F2F, "R"),
    (0x00010F70, 0x00010FFF, "R"),
    (0x0001E800, 0x0001EC6F, "R"),
    (0x0001ECC0, 0x0001ECFF, "R"),
    (0x0001ED50, 0x0001EDFF, "R"),
    (0x0001EF00, 0x0001EFFF, "R"),
    (0x20A0, 0x20CF, "ET"),
];

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let propvals = PropertyValues::from_ucd_dir(&dir)?;
    let rows: Vec<UnicodeData> = ucd_parse::parse(&dir)?;
    let core_prop: Vec<CoreProperty> = ucd_parse::parse(&dir)?;
    let use_short_names = args.is_present("short-names");
    let bidi_class_name = |short_name: &str| {
        if use_short_names {
            Ok(short_name.to_string())
        } else {
            propvals.canonical("bc", short_name)
        }
    };

    // If we were tasked with listing the available categories, then do that
    // and quit.
    if args.is_present("list-classes") {
        return print_property_values(&propvals, "Bidi_Class");
    }

    // Collect each bidi class into an ordered set.
    let mut by_type: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut assigned = BTreeSet::new();
    for row in rows {
        assigned.insert(row.codepoint.value());
        let bc = bidi_class_name(&row.bidi_class)?;
        by_type
            .entry(bc)
            .or_insert(BTreeSet::new())
            .insert(row.codepoint.value());
    }

    // Process the codepoints that are not listed as per the notes in
    // DerivedBidiClass.txt (UCD 12.1). See comment on
    // DEFAULT_CLASS_ASSIGNMENTS for more detail.
    //
    // Collect the codepoints that may default to BN
    let mut maybe_boundary_neutral = BTreeSet::new();
    for x in &core_prop {
        if &x.property == "Default_Ignorable_Code_Point"
            || &x.property == "Noncharacter_Code_Point"
        {
            maybe_boundary_neutral
                .extend(x.codepoints.into_iter().map(|c| c.value()));
        }
    }

    // Process unassigned codepoints
    let left_to_right_name = bidi_class_name("L")?;
    let boundary_neutral_name = bidi_class_name("BN")?;
    for cp in 0..=0x10FFFF {
        if assigned.contains(&cp) {
            continue;
        }
        // Check if this code point is in the default Bidi classes
        if let Some(class) = lookup_unassigned(cp, DEFAULT_CLASS_ASSIGNMENTS) {
            let name = bidi_class_name(class)?;
            by_type.get_mut(&name).unwrap().insert(cp);
        } else if maybe_boundary_neutral.contains(&cp) {
            by_type.get_mut(&boundary_neutral_name).unwrap().insert(cp);
        } else {
            // All others get assigned Left_To_Right
            by_type.get_mut(&left_to_right_name).unwrap().insert(cp);
        }
    }

    let mut wtr = args.writer("bidi_class")?;
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

/// Look up a code point in the unassigned default Bidi classes.
fn lookup_unassigned<'a>(
    codepoint: u32,
    defaults: &[(u32, u32, &'a str)],
) -> Option<&'a str> {
    defaults
        .iter()
        .find(|&&(start, end, _)| start <= codepoint && codepoint <= end)
        .map(|&(_, _, bidi_class)| bidi_class)
}
