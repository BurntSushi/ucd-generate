use std::{collections::BTreeMap, path::Path};

use ucd_parse::{self, JamoShortName};

use crate::args::ArgMatches;
use crate::error::Result;

pub fn command(args: ArgMatches<'_>) -> Result<()> {
    let dir = args.ucd_dir()?;
    let map = jamo_map(&Path::new(dir))?;
    let mut wtr = args.writer("jamo_short_name")?;
    wtr.codepoint_to_string(args.name(), &map)?;
    Ok(())
}

fn jamo_map(dir: &Path) -> Result<BTreeMap<u32, String>> {
    let jamo_map = ucd_parse::parse_by_codepoint::<_, JamoShortName>(dir)?;
    let mut map = BTreeMap::new();
    for (cp, jamo) in jamo_map {
        map.insert(cp.value(), jamo.name);
    }
    Ok(map)
}

pub fn table(dir: &Path) -> Result<Vec<(u32, String)>> {
    Ok(jamo_map(dir)?.into_iter().collect())
}

pub fn table_ref<'a>(table: &'a [(u32, String)]) -> Vec<(u32, &'a str)> {
    table.iter().map(|&(cp, ref name)| (cp, &**name)).collect()
}
