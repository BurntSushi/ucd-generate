use std::char;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::str;

use byteorder::{ByteOrder, BigEndian as BE};
use fst::{Map, MapBuilder, Set, SetBuilder};
use fst::raw::Fst;
use regex_automata::{StateID, DenseDFA, SparseDFA, Regex};
use ucd_trie::TrieSetOwned;

use error::Result;
use util;

#[derive(Clone, Debug)]
pub struct WriterBuilder(WriterOptions);

#[derive(Clone, Debug)]
struct WriterOptions {
    name: String,
    table_name: String,
    columns: u64,
    char_literals: bool,
    fst_dir: Option<PathBuf>,
    trie_set: bool,
    dfa_dir: Option<PathBuf>,
    emit_c: bool,
}

impl WriterBuilder {
    /// Create a new builder Unicode writers.
    ///
    /// The name given corresponds to the Rust module name to use when
    /// applicable.
    pub fn new(name: &str) -> WriterBuilder {
        WriterBuilder(WriterOptions {
            name: name.to_string(),
            table_name: name.to_uppercase(),
            columns: 79,
            char_literals: false,
            fst_dir: None,
            trie_set: false,
            dfa_dir: None,
            emit_c: false,
        })
    }

    /// Create a new Unicode writer from this builder's configuration.
    pub fn from_writer<W: io::Write + 'static>(&self, wtr: W) -> Writer {
        Writer {
            wtr: LineWriter::new(Box::new(wtr)),
            wrote_header: false,
            opts: self.0.clone(),
        }
    }

    /// Create a new Unicode writer that writes to stdout.
    pub fn from_stdout(&self) -> Writer {
        self.from_writer(io::stdout())
    }

    /// Create a new Unicode writer that writes FSTs to a directory.
    pub fn from_fst_dir<P: AsRef<Path>>(&self, fst_dir: P) -> Result<Writer> {
        let mut opts = self.0.clone();
        opts.fst_dir = Some(fst_dir.as_ref().to_path_buf());
        let mut fpath = fst_dir.as_ref().join(rust_module_name(&opts.name));
        fpath.set_extension("rs");
        Ok(Writer {
            wtr: LineWriter::new(Box::new(File::create(fpath)?)),
            wrote_header: false,
            opts: opts,
        })
    }

    /// Create a new writer that writes DFAs to a directory.
    pub fn from_dfa_dir<P: AsRef<Path>>(&self, dfa_dir: P) -> Result<Writer> {
        let mut opts = self.0.clone();
        opts.dfa_dir = Some(dfa_dir.as_ref().to_path_buf());
        let mut fpath = dfa_dir.as_ref().join(rust_module_name(&opts.name));
        fpath.set_extension("rs");
        Ok(Writer {
            wtr: LineWriter::new(Box::new(File::create(fpath)?)),
            wrote_header: false,
            opts: opts,
        })
    }

    /// Set the value to use as the table name where applicable. If not
    /// provided, an uppercase version of the string passed to the constructor
    /// is used.
    ///
    /// When emitting C code, this is also used as the prefix for all
    /// identifiers emitted by the writer (Unless it is empty, in which case
    /// they will be emitted unprefixed).
    pub fn table_name(&mut self, name: &str) -> &mut WriterBuilder {
        self.0.table_name = name.into();
        self
    }

    /// Set the column limit to use when writing Rust source code.
    ///
    /// Note that this is adhered to on a "best effort" basis.
    pub fn columns(&mut self, columns: u64) -> &mut WriterBuilder {
        self.0.columns = columns;
        self
    }

    /// When printing Rust source code, emit `char` literals instead of `u32`
    /// literals. Any codepoints that aren't Unicode scalar values (i.e.,
    /// surrogate codepoints) are silently dropped when writing.
    pub fn char_literals(&mut self, yes: bool) -> &mut WriterBuilder {
        self.0.char_literals = yes;
        self
    }

    /// Emit a trie when writing sets of codepoints instead of a slice of
    /// ranges.
    pub fn trie_set(&mut self, yes: bool) -> &mut WriterBuilder {
        self.0.trie_set = yes;
        self
    }

    /// Emit C code, instead of Rust code.
    pub fn emit_c(&mut self, yes: bool) -> &mut WriterBuilder {
        self.0.emit_c = yes;
        self
    }
}

/// A writer of various kinds of Unicode data.
///
/// A writer takes as input various forms of Unicode data and writes that data
/// in a number of different output formats.
pub struct Writer {
    wtr: LineWriter<Box<dyn io::Write + 'static>>,
    wrote_header: bool,
    opts: WriterOptions,
}

impl Writer {
    /// Write a sorted sequence of string names that map to Unicode set names.
    pub fn names<I: IntoIterator<Item=T>, T: AsRef<str>>(
        &mut self,
        names: I,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;
        let mut names: Vec<String> = names
            .into_iter()
            .map(|name| name.as_ref().to_string())
            .collect();
        names.sort();

        if self.opts.emit_c {
            return self.emit_c_name_data(names);
        }
        let ty =
            if self.opts.fst_dir.is_some() {
                "::fst::Set".to_string()
            } else if self.opts.trie_set {
                "&'static ::ucd_trie::TrieSet".to_string()
            } else {
                let charty = self.rust_codepoint_type();
                format!("&'static [({}, {})]", charty, charty)
            };


        writeln!(
            self.wtr,
            "pub const BY_NAME: &'static [(&'static str, {})] = &[",
            ty,
        )?;
        for name in names {
            let rustname = rust_const_name(&name);
            self.wtr.write_str(&format!("({:?}, {}), ", name, rustname))?;
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    fn c_const_name(&self, name: &str) -> String {
        // The rust code doesn't need to do this, but collision is a bigger
        // issue in C than in rust, so we prefix all tables with the name of the
        // "main" table (that is, whatever got passed into --name, if anything,
        // or the name that was given to the writer on construction).
        let name = rust_const_name(name);
        // the is_empty check is required, as all identifiers starting with `_`
        // followed by a capital letter are reserved in C. We also protect
        // aginst both this getting called multiple times, and this being the
        // table we're interested in generating in the first place.
        if  !self.opts.table_name.is_empty() && !self.opts.table_name.starts_with(&name) {
            return format!("{}_{}", self.opts.table_name, name);
        }
        return name;
    }

    // C has some... Rather annoying rules about empty arrays. Specifically,
    // arrays of length 0 are illegal (accepted by gcc/clang, but rejected by
    // msvc, even in C++ mode). Additionally, you aren't allowed to use empty
    // braces to initialize an array in C (although you are in C++), you're
    // supposed to use a constant `0` (even for an array of structs). As a
    // result all our arrays have a FOO_SIZE constant that's the real size, and
    // then an array which is 1 item longer. We could instead special-case empty
    // arrays even further, and have them be the only case where these two
    // differ, but that would be even more annoying. Anyway, this function
    // handles all of that, at least for arrays at the top level.
    //
    // Note that if the item_type we're outputting is an array,
    // `type_array_length` should be non-zero.
    fn begin_c_array(&mut self, name: &str, size: usize, item_type: &str, type_array_length: Option<usize>) -> Result<()> {
        let id = self.c_const_name(name);
        writeln!(self.wtr, "#define {}_SIZE {}", id, size)?;
        writeln!(self.wtr)?;
        if let Some(len) = type_array_length {
            // We should know the item type statically. If you need to support
            // zero-length arrays here, then, well, come up with a scheme
            // similar to the above.
            assert!(len != 0);
            // Yes, the nested array size coming after is correct. C declaration
            // syntax is a mess, I know.
            writeln!(self.wtr, "UCD_GENERATE_DATA_LINKAGE const {} {}[{}_SIZE + 1][{}] = {{", item_type, id, id, len)?;
        } else {
            writeln!(self.wtr, "UCD_GENERATE_DATA_LINKAGE const {} {}[{}_SIZE + 1] = {{", item_type, id, id)?;
        }
        // Add the zero item here, so theat the caller can just call this and
        // loop, then close it with end_c_array.
        if size == 0 {
            self.wtr.write_str("0")?
        }
        Ok(())
    }
    // This has no special rules, but we do it for symmetry with the above function.
    fn end_c_array(&mut self) -> Result<()> {
        // C has weird rules around the last line of a file, so
        writeln!(self.wtr, "}};")?;
        Ok(())
    }

    fn emit_c_name_data(&mut self, names: Vec<String>) -> Result<()> {
        // No tuples in C. We could define an anon struct, but then you can't
        // pass it to functions, etc.
        let struct_ty_name = self.c_const_name("name_entry").to_ascii_lowercase();
        writeln!(self.wtr, "struct {} {{", struct_ty_name)?;
        writeln!(self.wtr, "  char const *name;")?;
        writeln!(self.wtr, "  unsigned int table_size;")?;
        writeln!(self.wtr, "  const unsigned int (*table)[2];")?;
        writeln!(self.wtr, "}};\n")?;
        self.begin_c_array("BY_NAME", names.len(), &format!("struct {}", struct_ty_name), None)?;
        for name in names {
            let tabname = self.c_const_name(&name);
            self.wtr.write_str(&format!("{{{:?}, {}_SIZE, {}}}, ", name, tabname, tabname))?;
        }
        self.end_c_array()
    }

    fn ensure_not_c(&self, emitting: &str) -> Result<()> {
        if self.opts.emit_c {
            err!("cannot emit a {} as C code", emitting)
        } else {
            Ok(())
        }
    }

    /// Write a sorted sequence of codepoints.
    ///
    /// Note that the specific representation of ranges may differ with the
    /// output format. For example, if the output format is a slice, then a
    /// straight-forward slice of sorted codepoint ranges is emitted. But if
    /// the output format is an FST or similar, then all codepoints are
    /// explicitly represented.
    pub fn ranges(
        &mut self,
        name: &str,
        codepoints: &BTreeSet<u32>,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = SetBuilder::memory();
            builder.extend_iter(codepoints.iter().cloned().map(u32_key))?;
            let set = Set::from_bytes(builder.into_inner()?)?;
            self.fst(&name, set.as_fst(), false)?;
        } else if self.opts.trie_set {
            let set: Vec<u32> = codepoints.iter().cloned().collect();
            let trie = TrieSetOwned::from_codepoints(&set)?;
            self.trie_set(&name, &trie)?;
        } else {
            let ranges = util::to_ranges(codepoints.iter().cloned());
            self.ranges_slice(&name, &ranges)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    fn c_ranges_array(
        &mut self,
        name: &str,
        table: &[(u32, u32)],
    ) -> Result<()> {
        self.begin_c_array(name, table.len(), "unsigned int", Some(2))?;
        for &(start, end) in table {
            self.wtr.write_str(&format!("{{{}, {}}}, ", start, end))?;
        }
        self.end_c_array()?;
        Ok(())
    }

    fn ranges_slice(
        &mut self,
        name: &str,
        table: &[(u32, u32)],
    ) -> Result<()> {
        if self.opts.emit_c {
            return self.c_ranges_array(name, table);
        }
        let ty = self.rust_codepoint_type();
        writeln!(
            self.wtr,
            "pub const {}: &'static [({}, {})] = &[",
            name, ty, ty)?;
        for &(start, end) in table {
            let range = (self.rust_codepoint(start), self.rust_codepoint(end));
            if let (Some(start), Some(end)) = range {
                self.wtr.write_str(&format!("({}, {}), ", start, end))?;
            }
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    fn trie_set(
        &mut self,
        name: &str,
        trie: &TrieSetOwned,
    ) -> Result<()> {
        self.ensure_not_c("trie set")?;
        let trie = trie.as_slice();
        writeln!(
            self.wtr,
            "pub const {}: &'static ::ucd_trie::TrieSet = \
                &::ucd_trie::TrieSet {{",
            name)?;

        self.wtr.indent("    ");

        writeln!(self.wtr, "  tree1_level1: &[")?;
        self.write_slice_u64(&trie.tree1_level1)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "  tree2_level1: &[")?;
        self.write_slice_u8(&trie.tree2_level1)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "  tree2_level2: &[")?;
        self.write_slice_u64(&trie.tree2_level2)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "  tree3_level1: &[")?;
        self.write_slice_u8(&trie.tree3_level1)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "  tree3_level2: &[")?;
        self.write_slice_u8(&trie.tree3_level2)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "  tree3_level3: &[")?;
        self.write_slice_u64(&trie.tree3_level3)?;
        writeln!(self.wtr, "  ],")?;

        writeln!(self.wtr, "}};")?;
        Ok(())
    }

    /// Write a map that associates codepoint ranges to a single value in an
    /// enumeration. This usually emits two items: a map from codepoint range
    /// to index and a map from index to one of the enum variants.
    ///
    /// The given map should be a map from the enum variant value to the set
    /// of codepoints that have that value.
    pub fn ranges_to_enum(
        &mut self,
        name: &str,
        enum_map: &BTreeMap<String, BTreeSet<u32>>,
    ) -> Result<()> {
        self.ensure_not_c("range->enum map")?;
        self.header()?;
        self.separator()?;

        writeln!(
            self.wtr,
            "pub const {}_ENUM: &'static [&'static str] = &[",
            rust_const_name(name))?;
        for variant in enum_map.keys() {
            self.wtr.write_str(&format!("{:?}, ", variant))?;
        }
        writeln!(self.wtr, "];")?;

        let mut map = BTreeMap::new();
        for (i, (_, ref set)) in enum_map.iter().enumerate() {
            map.extend(set.iter().cloned().map(|cp| (cp, i as u64)));
        }
        self.ranges_to_unsigned_integer(name, &map)?;
        self.wtr.flush()?;
        Ok(())
    }

    /// Write a map that associates ranges of codepoints with an arbitrary
    /// integer.
    ///
    /// The smallest numeric type is used when applicable.
    pub fn ranges_to_unsigned_integer(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, u64>,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = MapBuilder::memory();
            for (&k, &v) in map {
                builder.insert(u32_key(k), v)?;
            }
            let map = Map::from_bytes(builder.into_inner()?)?;
            self.fst(&name, map.as_fst(), true)?;
        } else {
            let ranges = util::to_range_values(
                map.iter().map(|(&k, &v)| (k, v)));
            self.ranges_to_unsigned_integer_slice(&name, &ranges)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    fn ranges_to_unsigned_integer_slice(
        &mut self,
        name: &str,
        table: &[(u32, u32, u64)],
    ) -> Result<()> {
        self.ensure_not_c("range->unsigned int map")?;
        let cp_ty = self.rust_codepoint_type();
        let num_ty = match table.iter().map(|&(_, _, n)| n).max() {
            None => "u8",
            Some(max_num) => smallest_unsigned_type(max_num),
        };

        writeln!(
            self.wtr,
            "pub const {}: &'static [({}, {}, {})] = &[",
            name, cp_ty, cp_ty, num_ty)?;
        for &(start, end, num) in table {
            let range = (self.rust_codepoint(start), self.rust_codepoint(end));
            if let (Some(start), Some(end)) = range {
                let src = format!("({}, {}, {}), ", start, end, num);
                self.wtr.write_str(&src)?;
            }
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    /// Write a map that associates strings to strings.
    ///
    /// The only supported output format is a sorted slice, which can be
    /// binary searched.
    pub fn string_to_string(
        &mut self,
        name: &str,
        map: &BTreeMap<String, String>,
    ) -> Result<()> {
        if self.opts.fst_dir.is_some() {
            return err!("cannot emit string->string map as an FST");
        }
        self.ensure_not_c("string->string map")?;

        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        writeln!(
            self.wtr,
            "pub const {}: &'static [(&'static str, &'static str)] = &[",
            name)?;
        for (k, v) in map {
            self.wtr.write_str(&format!("({:?}, {:?}), ", k, v))?;
        }
        writeln!(self.wtr, "];")?;

        self.wtr.flush()?;
        Ok(())
    }

    /// Write a map that associates strings to another map from strings to
    /// strings.
    ///
    /// The only supported output format is a sorted slice, which can be
    /// binary searched.
    pub fn string_to_string_to_string(
        &mut self,
        name: &str,
        map: &BTreeMap<String, BTreeMap<String, String>>,
    ) -> Result<()> {
        if self.opts.fst_dir.is_some() {
            return err!("cannot emit string->string map as an FST");
        }
        self.ensure_not_c("string->string map")?;

        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        writeln!(
            self.wtr,
            "pub const {}: &'static \
                [(&'static str, \
                    &'static [(&'static str, &'static str)])] = &[",
            name)?;
        let mut first = true;
        for (k1, kv) in map {
            if !first {
                writeln!(self.wtr, "")?;
            }
            first = false;

            self.wtr.write_str(&format!("({:?}, &[", k1))?;
            for (k2, v) in kv {
                self.wtr.write_str(&format!("({:?}, {:?}), ", k2, v))?;
            }
            self.wtr.write_str("]), ")?;
        }
        writeln!(self.wtr, "];")?;

        self.wtr.flush()?;
        Ok(())
    }

    /// Write a map that associates codepoints with other codepoints.
    ///
    /// This supports the FST format in addition to the standard sorted slice
    /// format. When using an FST, the keys and values are 32-bit big endian
    /// unsigned integers.
    pub fn codepoint_to_codepoint(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, u32>,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = MapBuilder::memory();
            for (&k, &v) in map {
                builder.insert(u32_key(k), v as u64)?;
            }
            let map = Map::from_bytes(builder.into_inner()?)?;
            self.fst(&name, map.as_fst(), true)?;
        } else {
            let table: Vec<(u32, u32)> =
                map.iter().map(|(&k, &v)| (k, v)).collect();
            self.ranges_slice(&name, &table)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    /// Write a map that associates codepoints with other codepoints, where
    /// each codepoint can be associated with possibly many other codepoints.
    ///
    /// This does not support the FST format.
    pub fn multi_codepoint_to_codepoint(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, BTreeSet<u32>>,
    ) -> Result<()> {
        if self.opts.fst_dir.is_some() {
            return err!("cannot emit codepoint multimaps as an FST");
        }

        let mut map2: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (&k, vs) in map {
            let vs2 = vs.iter().cloned().collect();
            map2.insert(k, vs2);
        }
        self.codepoint_to_codepoints(name, &map2)
    }

    /// Write a map that associates codepoints with a sequence of other
    /// codepoints.
    ///
    /// This does not support the FST format.
    pub fn codepoint_to_codepoints(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, Vec<u32>>,
    ) -> Result<()> {
        if self.opts.fst_dir.is_some() {
            return err!("cannot emit codepoint->codepoints map as an FST");
        }

        self.header()?;
        self.separator()?;

        if self.opts.emit_c {
            return self.c_codepoint_to_codepoints(name, map);
        }

        let name = rust_const_name(name);
        let ty = self.rust_codepoint_type();
        writeln!(
            self.wtr,
            "pub const {}: &'static [({}, &'static [{}])] = &[",
            name, ty, ty)?;
    'LOOP:
        for (&k, vs) in map {
            // Make sure both our keys and values can be represented in the
            // user's chosen codepoint format.
            let kstr = match self.rust_codepoint(k) {
                None => continue 'LOOP,
                Some(k) => k,
            };
            let mut vstrs = vec![];
            for &v in vs {
                match self.rust_codepoint(v) {
                    None => continue 'LOOP,
                    Some(v) => vstrs.push(v),
                }
            }

            self.wtr.write_str(&format!("({}, &[", kstr))?;
            if vstrs.len() == 1 {
                self.wtr.write_str(&format!("{}", &vstrs[0]))?;
            } else {
                for v in vstrs {
                    self.wtr.write_str(&format!("{}, ", v))?;
                }
            }
            self.wtr.write_str("]), ")?;
        }
        writeln!(self.wtr, "];")?;

        self.wtr.flush()?;
        Ok(())
    }

    fn c_codepoint_to_codepoints(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, Vec<u32>>,
    ) -> Result<()> {
        let map_entry_size = map.values().map(|v| v.len()).max().unwrap_or_default().max(1);

        let struct_ty_name = self.c_const_name("entry").to_ascii_lowercase();
        writeln!(self.wtr, "struct {} {{", struct_ty_name)?;
        writeln!(self.wtr, "  unsigned int key;")?;
        writeln!(self.wtr, "  unsigned int num_vals;")?;
        writeln!(self.wtr, "  unsigned int vals[{}];", map_entry_size)?;
        writeln!(self.wtr, "}};\n")?;
        self.begin_c_array(name, map.len(), &format!("struct {}", struct_ty_name), None)?;
        for (&k, vs) in map {
            let items = vs.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
            // Note that omitting the braces is legal, and makes us not have to
            // special case empty arrays for once.
            self.wtr.write_str(&format!("{{{}, {}, {}}}, ", k, vs.len(), items))?;
        }
        self.end_c_array()
    }

    /// Write a map that associates codepoints to strings.
    ///
    /// When the output format is an FST, then the FST map emitted is from
    /// codepoint to u64, where the string is encoded into the u64. The least
    /// significant byte of the u64 corresponds to the first byte in the
    /// string. The end of a string is delimited by the zero byte. If a string
    /// is more than 8 bytes or contains a `NUL` byte, then an error is
    /// returned.
    pub fn codepoint_to_string(
        &mut self,
        name: &str,
        map: &BTreeMap<u32, String>,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = MapBuilder::memory();
            for (&k, v) in map {
                let v = pack_str(v)?;
                builder.insert(u32_key(k), v)?;
            }
            let map = Map::from_bytes(builder.into_inner()?)?;
            self.fst(&name, map.as_fst(), true)?;
        } else {
            let table: Vec<(u32, &str)> =
                map.iter().map(|(&k, v)| (k, &**v)).collect();
            self.codepoint_to_string_slice(&name, &table)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    fn codepoint_to_string_slice(
        &mut self,
        name: &str,
        table: &[(u32, &str)],
    ) -> Result<()> {
        self.ensure_not_c("codepoint->string map")?;
        let ty = self.rust_codepoint_type();
        writeln!(
            self.wtr,
            "pub const {}: &'static [({}, &'static str)] = &[",
            name, ty)?;
        for &(cp, ref s) in table {
            if let Some(cp) = self.rust_codepoint(cp) {
                self.wtr.write_str(&format!("({}, {:?}), ", cp, s))?;
            }
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    /// Write a map that associates strings to codepoints.
    pub fn string_to_codepoint(
        &mut self,
        name: &str,
        map: &BTreeMap<String, u32>,
    ) -> Result<()> {
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = MapBuilder::memory();
            for (k, &v) in map {
                builder.insert(k.as_bytes(), v as u64)?;
            }
            let map = Map::from_bytes(builder.into_inner()?)?;
            self.fst(&name, map.as_fst(), true)?;
        } else {
            let table: Vec<(&str, u32)> =
                map.iter().map(|(k, &v)| (&**k, v)).collect();
            self.string_to_codepoint_slice(&name, &table)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    fn string_to_codepoint_slice(
        &mut self,
        name: &str,
        table: &[(&str, u32)],
    ) -> Result<()> {
        self.ensure_not_c("string->codepoint map")?;
        let ty = self.rust_codepoint_type();
        writeln!(
            self.wtr,
            "pub const {}: &'static [(&'static str, {})] = &[",
            name, ty)?;
        for &(ref s, cp) in table {
            if let Some(cp) = self.rust_codepoint(cp) {
                self.wtr.write_str(&format!("({:?}, {}), ", s, cp))?;
            }
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    /// Write a map that associates strings to `u64` values.
    pub fn string_to_u64(
        &mut self,
        name: &str,
        map: &BTreeMap<String, u64>,
    ) -> Result<()> {
        self.ensure_not_c("string->u64 map")?;
        self.header()?;
        self.separator()?;

        let name = rust_const_name(name);
        if self.opts.fst_dir.is_some() {
            let mut builder = MapBuilder::memory();
            for (k, &v) in map {
                builder.insert(k.as_bytes(), v)?;
            }
            let map = Map::from_bytes(builder.into_inner()?)?;
            self.fst(&name, map.as_fst(), true)?;
        } else {
            let table: Vec<(&str, u64)> =
                map.iter().map(|(k, &v)| (&**k, v)).collect();
            self.string_to_u64_slice(&name, &table)?;
        }
        self.wtr.flush()?;
        Ok(())
    }

    fn string_to_u64_slice(
        &mut self,
        name: &str,
        table: &[(&str, u64)],
    ) -> Result<()> {
        writeln!(
            self.wtr,
            "pub const {}: &'static [(&'static str, u64)] = &[",
            name)?;
        for &(ref s, n) in table {
            self.wtr.write_str(&format!("({:?}, {}), ", s, n))?;
        }
        writeln!(self.wtr, "];")?;
        Ok(())
    }

    fn fst(&mut self, const_name: &str, fst: &Fst, map: bool) -> Result<()> {
        self.ensure_not_c("fst")?;
        let fst_dir = self.opts.fst_dir.as_ref().unwrap();
        let fst_file_name = format!("{}.fst", rust_module_name(const_name));
        let fst_file_path = fst_dir.join(&fst_file_name);
        File::create(fst_file_path)?.write_all(&fst.to_vec())?;

        let ty = if map { "Map" } else { "Set" };
        writeln!(self.wtr, "lazy_static! {{")?;
        writeln!(
            self.wtr,
            "  pub static ref {}: ::fst::{} = ", const_name, ty)?;
        writeln!(
            self.wtr,
            "    ::fst::{}::from(::fst::raw::Fst::from_static_slice(", ty)?;
        writeln!(
            self.wtr,
            "      include_bytes!({:?})).unwrap());", fst_file_name)?;
        writeln!(self.wtr, "}}")?;
        Ok(())
    }

    pub fn dense_regex<T: AsRef<[S]>, S: StateID>(
        &mut self,
        const_name: &str,
        re: &Regex<DenseDFA<T, S>>,
    ) -> Result<()> {
        self.ensure_not_c("dense regex")?;
        self.header()?;
        self.separator()?;

        let rust_name = rust_module_name(const_name);
        let idty = rust_uint_type::<S>();
        let fname_fwd_be = format!("{}.fwd.bigendian.dfa", rust_name);
        let fname_rev_be = format!("{}.rev.bigendian.dfa", rust_name);
        let fname_fwd_le = format!("{}.fwd.littleendian.dfa", rust_name);
        let fname_rev_le = format!("{}.rev.littleendian.dfa", rust_name);
        let ty = format!(
            "Regex<::regex_automata::DenseDFA<&'static [{}], {}>>",
            idty, idty
        );
        {
            let dfa_dir = self.opts.dfa_dir.as_ref().unwrap();

            File::create(dfa_dir.join(&fname_fwd_be))?
                .write_all(&re.forward().to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_rev_be))?
                .write_all(&re.reverse().to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_fwd_le))?
                .write_all(&re.forward().to_bytes_little_endian()?)?;
            File::create(dfa_dir.join(&fname_rev_le))?
                .write_all(&re.reverse().to_bytes_little_endian()?)?;
        }
        writeln!(self.wtr, "#[cfg(target_endian = \"big\")]")?;
        self.write_regex_static(
            const_name, &ty, "DenseDFA", idty, &fname_fwd_be, &fname_rev_be,
        )?;

        self.separator()?;

        writeln!(self.wtr, "#[cfg(target_endian = \"little\")]")?;
        self.write_regex_static(
            const_name, &ty, "DenseDFA", idty, &fname_fwd_le, &fname_rev_le,
        )?;
        Ok(())
    }

    pub fn sparse_regex<T: AsRef<[u8]>, S: StateID>(
        &mut self,
        const_name: &str,
        re: &Regex<SparseDFA<T, S>>,
    ) -> Result<()> {
        self.ensure_not_c("sparse regex")?;
        self.header()?;
        self.separator()?;

        let rust_name = rust_module_name(const_name);
        let idty = rust_uint_type::<S>();
        let fname_fwd_be = format!("{}.fwd.bigendian.dfa", rust_name);
        let fname_rev_be = format!("{}.rev.bigendian.dfa", rust_name);
        let fname_fwd_le = format!("{}.fwd.littleendian.dfa", rust_name);
        let fname_rev_le = format!("{}.rev.littleendian.dfa", rust_name);
        let ty = format!(
            "Regex<::regex_automata::SparseDFA<&'static [u8], {}>>", idty
        );
        {
            let dfa_dir = self.opts.dfa_dir.as_ref().unwrap();

            File::create(dfa_dir.join(&fname_fwd_be))?
                .write_all(&re.forward().to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_rev_be))?
                .write_all(&re.reverse().to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_fwd_le))?
                .write_all(&re.forward().to_bytes_little_endian()?)?;
            File::create(dfa_dir.join(&fname_rev_le))?
                .write_all(&re.reverse().to_bytes_little_endian()?)?;
        }
        writeln!(self.wtr, "#[cfg(target_endian = \"big\")]")?;
        self.write_regex_static(
            const_name, &ty, "SparseDFA", "u8", &fname_fwd_be, &fname_rev_be,
        )?;

        self.separator()?;

        writeln!(self.wtr, "#[cfg(target_endian = \"little\")]")?;
        self.write_regex_static(
            const_name, &ty, "SparseDFA", "u8", &fname_fwd_le, &fname_rev_le,
        )?;
        Ok(())
    }

    pub fn dense_dfa<T: AsRef<[S]>, S: StateID>(
        &mut self,
        const_name: &str,
        dfa: &DenseDFA<T, S>,
    ) -> Result<()> {
        self.ensure_not_c("dense dfa")?;
        self.header()?;
        self.separator()?;

        let rust_name = rust_module_name(const_name);
        let fname_be = format!("{}.bigendian.dfa", rust_name);
        let fname_le = format!("{}.littleendian.dfa", rust_name);
        let idty = rust_uint_type::<S>();
        let ty = format!("DenseDFA<&'static [{}], {}>", idty, idty);
        {
            let dfa_dir = self.opts.dfa_dir.as_ref().unwrap();
            File::create(dfa_dir.join(&fname_be))?
                .write_all(&dfa.to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_le))?
                .write_all(&dfa.to_bytes_little_endian()?)?;
        }
        writeln!(self.wtr, "#[cfg(target_endian = \"big\")]")?;
        self.write_dfa_static(const_name, &ty, "DenseDFA", idty, &fname_be)?;

        self.separator()?;

        writeln!(self.wtr, "#[cfg(target_endian = \"little\")]")?;
        self.write_dfa_static(const_name, &ty, "DenseDFA", idty, &fname_le)?;
        Ok(())
    }

    pub fn sparse_dfa<T: AsRef<[u8]>, S: StateID>(
        &mut self,
        const_name: &str,
        dfa: &SparseDFA<T, S>,
    ) -> Result<()> {
        self.ensure_not_c("sparse dfa")?;
        self.header()?;
        self.separator()?;

        let rust_name = rust_module_name(const_name);
        let fname_be = format!("{}.bigendian.dfa", rust_name);
        let fname_le = format!("{}.littleendian.dfa", rust_name);
        let idty = rust_uint_type::<S>();
        let ty = format!("SparseDFA<&'static [u8], {}>", idty);
        {
            let dfa_dir = self.opts.dfa_dir.as_ref().unwrap();
            File::create(dfa_dir.join(&fname_be))?
                .write_all(&dfa.to_bytes_big_endian()?)?;
            File::create(dfa_dir.join(&fname_le))?
                .write_all(&dfa.to_bytes_little_endian()?)?;
        }
        writeln!(self.wtr, "#[cfg(target_endian = \"big\")]")?;
        self.write_dfa_static(const_name, &ty, "SparseDFA", "u8", &fname_be)?;

        self.separator()?;

        writeln!(self.wtr, "#[cfg(target_endian = \"little\")]")?;
        self.write_dfa_static(const_name, &ty, "SparseDFA", "u8", &fname_le)?;
        Ok(())
    }

    fn write_regex_static(
        &mut self,
        const_name: &str,
        full_regex_ty: &str,
        short_dfa_ty: &str,
        align_to: &str,
        file_name_fwd: &str,
        file_name_rev: &str,
    ) -> Result<()> {
        self.ensure_not_c("static regex")?;
        writeln!(self.wtr, "lazy_static! {{")?;
        writeln!(
            self.wtr,
            "  pub static ref {}: ::regex_automata::{} = {{",
            const_name,
            full_regex_ty)?;

        writeln!(self.wtr, "    let fwd =")?;
        self.write_dfa_deserialize(short_dfa_ty, align_to, file_name_fwd)?;
        writeln!(self.wtr, "    ;")?;

        writeln!(self.wtr, "    let rev =")?;
        self.write_dfa_deserialize(short_dfa_ty, align_to, file_name_rev)?;
        writeln!(self.wtr, "    ;")?;

        writeln!(
            self.wtr,
            "    ::regex_automata::Regex::from_dfas(fwd, rev)")?;
        writeln!(self.wtr, "  }};")?;
        writeln!(self.wtr, "}}")?;

        Ok(())
    }

    fn write_dfa_static(
        &mut self,
        const_name: &str,
        full_dfa_ty: &str,
        short_dfa_ty: &str,
        align_to: &str,
        file_name: &str,
    ) -> Result<()> {
        self.ensure_not_c("static dfa")?;
        writeln!(self.wtr, "lazy_static! {{")?;
        writeln!(
            self.wtr,
            "  pub static ref {}: ::regex_automata::{} = {{",
            const_name,
            full_dfa_ty)?;
        self.write_dfa_deserialize(short_dfa_ty, align_to, file_name)?;
        writeln!(self.wtr, "  }};")?;
        writeln!(self.wtr, "}}")?;

        Ok(())
    }

    fn write_dfa_deserialize(
        &mut self,
        short_dfa_ty: &str,
        align_to: &str,
        file_name: &str,
    ) -> Result<()> {
        writeln!(self.wtr, "    #[repr(C)]")?;
        writeln!(self.wtr, "    struct Aligned<B: ?Sized> {{")?;
        writeln!(self.wtr, "        _align: [{}; 0],", align_to)?;
        writeln!(self.wtr, "        bytes: B,")?;
        writeln!(self.wtr, "    }}")?;
        writeln!(self.wtr, "    ")?;

        writeln!(
            self.wtr,
            "    static ALIGNED: &'static Aligned<[u8]> = &Aligned {{")?;
        writeln!(self.wtr, "        _align: [],")?;
        writeln!(self.wtr, "        bytes: *include_bytes!({:?}),", file_name)?;
        writeln!(self.wtr, "    }};")?;
        writeln!(self.wtr, "    ")?;

        writeln!(self.wtr, "    unsafe {{")?;
        writeln!(
            self.wtr,
            "      ::regex_automata::{}::from_bytes(&ALIGNED.bytes)", short_dfa_ty)?;
        writeln!(self.wtr, "    }}")?;

        Ok(())
    }

    fn write_slice_u8(&mut self, xs: &[u8]) -> Result<()> {
        for &x in xs {
            self.wtr.write_str(&format!("{}, ", x))?;
        }
        Ok(())
    }

    fn write_slice_u64(&mut self, xs: &[u64]) -> Result<()> {
        for &x in xs {
            if x == 0 {
                self.wtr.write_str("0, ")?;
            } else {
                self.wtr.write_str(&format!("0x{:X}, ", x))?;
            }
        }
        Ok(())
    }

    fn header(&mut self) -> Result<()> {
        if self.wrote_header {
            return Ok(());
        }
        self.wrote_header = true;
        let mut argv = vec![];
        argv.push(
            env::current_exe()?
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned());
        for arg in env::args_os().skip(1) {
            let x = arg.to_string_lossy();
            if x.contains("\n") {
                argv.push("[snip (arg too long)]".to_string());
            } else {
                argv.push(x.into_owned());
            }
        }
        // Use /* and not //, since rust supports both, but we might be emitting
        // C, which does not.
        writeln!(self.wtr, "/* DO NOT EDIT THIS FILE. \
                               IT WAS AUTOMATICALLY GENERATED BY:")?;
        writeln!(self.wtr, " *")?;
        writeln!(self.wtr, " *  {}", argv.join(" "))?;
        writeln!(self.wtr, " *")?;
        writeln!(self.wtr, " * ucd-generate is available on crates.io.")?;
        if !self.opts.emit_c {
            writeln!(self.wtr, " */")?;
            return Ok(());
        }

        // Write this once in the header, instead of repeating it dozens of
        // times for files with many arrays.
        writeln!(self.wtr, " *")?;
        writeln!(self.wtr, " * Note: The final item of each array is padding, so you must")?;
        writeln!(self.wtr, " * use it's corresponding <arr>_SIZE constant, rather than")?;
        writeln!(self.wtr, " * using sizeof or similar to determine the size.")?;
        writeln!(self.wtr, " */")?;
        writeln!(self.wtr)?;

        writeln!(self.wtr, "#ifndef UCD_GENERATE_DATA_LINKAGE")?;
        writeln!(self.wtr, "#define UCD_GENERATE_DATA_LINKAGE static")?;
        writeln!(self.wtr, "#endif")?;
        writeln!(self.wtr)?;

        Ok(())
    }

    fn separator(&mut self) -> Result<()> {
        write!(self.wtr, "\n")?;
        Ok(())
    }

    /// Return valid Rust source code that represents the given codepoint.
    ///
    /// The source code returned is either a u32 literal or a char literal,
    /// depending on the configuration. If the configuration demands a char
    /// literal and the given codepoint is a surrogate, then return None.
    fn rust_codepoint(&self, cp: u32) -> Option<String> {
        if self.opts.char_literals {
            char::from_u32(cp).map(|c| format!("{:?}", c))
        } else {
            Some(cp.to_string())
        }
    }

    /// Return valid Rust source code indicating the type of the codepoint
    /// that we emit based on this writer's configuration.
    fn rust_codepoint_type(&self) -> &'static str {
        if self.opts.char_literals {
            "char"
        } else {
            "u32"
        }
    }
}

#[derive(Debug)]
struct LineWriter<W> {
    wtr: W,
    line: String,
    columns: usize,
    indent: String,
}

impl<W: io::Write> LineWriter<W> {
    fn new(wtr: W) -> LineWriter<W> {
        LineWriter {
            wtr: wtr,
            line: String::new(),
            columns: 79,
            indent: "  ".to_string(),
        }
    }

    fn write_str(&mut self, s: &str) -> io::Result<()> {
        if self.line.len() + s.len() > self.columns {
            self.flush_line()?;
        }
        if self.line.is_empty() {
            self.line.push_str(&self.indent);
        }
        self.line.push_str(s);
        Ok(())
    }

    fn indent(&mut self, s: &str) {
        self.indent = s.to_string();
    }

    fn flush_line(&mut self) -> io::Result<()> {
        if self.line.is_empty() {
            return Ok(());
        }
        self.wtr.write_all(self.line.trim_end().as_bytes())?;
        self.wtr.write_all(b"\n")?;
        self.line.clear();
        Ok(())
    }
}

impl<W: io::Write> io::Write for LineWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.flush_line()?;
        self.wtr.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_line()?;
        self.wtr.flush()
    }
}

/// Heuristically produce an appropriate constant Rust name.
fn rust_const_name(s: &str) -> String {
    // Property names/values seem pretty uniform, particularly the
    // "canonical" variants we use to produce variable names. So we
    // don't need to do much.
    //
    // N.B. Age names have a `.` in them, so get rid of that.
    let mut s = s.replace('.', "_").to_string();
    s.make_ascii_uppercase();
    s
}

/// Heuristically produce an appropriate module Rust name.
fn rust_module_name(s: &str) -> String {
    // Property names/values seem pretty uniform, particularly the
    // "canonical" variants we use to produce variable names. So we
    // don't need to do much.
    let mut s = s.to_string();
    s.make_ascii_lowercase();
    s
}

/// Return the unsigned integer type for the size of the given type, which must
/// have size 1, 2, 4 or 8.
fn rust_uint_type<S>() -> &'static str {
    match size_of::<S>() {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        8 => "u64",
        s => panic!("unsupported DFA state id size: {}", s),
    }
}

/// Return the given u32 encoded in big-endian.
pub fn u32_key(cp: u32) -> [u8; 4] {
    let mut key = [0; 4];
    BE::write_u32(&mut key, cp);
    key
}

/// Convert the given string into a u64, where the least significant byte of
/// the u64 is the first byte of the string.
///
/// If the string contains any `NUL` bytes or has more than 8 bytes, then an
/// error is returned.
fn pack_str(s: &str) -> Result<u64> {
    if s.len() > 8 {
        return err!("cannot encode string {:?} (too long)", s);
    }
    if s.contains('\x00') {
        return err!("cannot encode string {:?} (contains NUL byte)", s);
    }
    let mut value = 0;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        assert!(i <= 7);
        value |= (b as u64) << (8 * i as u64);
    }
    Ok(value)
}

/// Return a string representing the smallest unsigned integer type for the
/// given value.
fn smallest_unsigned_type(n: u64) -> &'static str {
    if n <= ::std::u8::MAX as u64 {
        "u8"
    } else if n <= ::std::u16::MAX as u64 {
        "u16"
    } else if n <= ::std::u32::MAX as u64 {
        "u32"
    } else {
        "u64"
    }
}

#[cfg(test)]
mod tests {
    use super::pack_str;

    fn unpack_str(mut encoded: u64) -> String {
        let mut value = String::new();
        while encoded != 0 {
            value.push((encoded & 0xFF) as u8 as char);
            encoded = encoded >> 8;
        }
        value
    }

    #[test]
    fn packed() {
        assert_eq!("G", unpack_str(pack_str("G").unwrap()));
        assert_eq!("GG", unpack_str(pack_str("GG").unwrap()));
        assert_eq!("YEO", unpack_str(pack_str("YEO").unwrap()));
        assert_eq!("ABCDEFGH", unpack_str(pack_str("ABCDEFGH").unwrap()));
        assert_eq!("", unpack_str(pack_str("").unwrap()));

        assert!(pack_str("ABCDEFGHI").is_err());
        assert!(pack_str("AB\x00CD").is_err());
    }
}
