use std::collections::BTreeMap;

use ucd_parse::{self, BidiMirroring};

use crate::args::ArgMatches;
use crate::error::Result;

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let rows: Vec<BidiMirroring> = ucd_parse::parse(&dir)?;

    let table: BTreeMap<_, _> = rows
        .into_iter()
        .map(|mapping| {
            (mapping.codepoint.value(), mapping.bidi_mirroring_glyph.value())
        })
        .collect();

    let mut wtr = args.writer("bidi_mirroring_glyph")?;
    if args.is_present("rust-match") {
        wtr.codepoint_to_codepoint_fn(args.name(), &table)?;
    } else {
        wtr.codepoint_to_codepoint(args.name(), &table)?;
    }

    Ok(())
}
