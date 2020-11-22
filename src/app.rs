use clap::{App, AppSettings, Arg, SubCommand};

const TEMPLATE: &'static str = "\
{bin} {version}
{author}
{about}

USAGE:
    {usage}

SUBCOMMANDS:
{subcommands}

OPTIONS:
{unified}";

const TEMPLATE_SUB: &'static str = "\
{before-help}
USAGE:
    {usage}

ARGS:
{positionals}

OPTIONS:
{unified}";

const ABOUT: &'static str = "
ucd-generate is a tool that generates Rust source files containing various
Unicode tables.

Unicode tables are typically represented by a sorted sequence of character
ranges, which can be searched quickly via binary search. There is also support
for generating FSTs (slower access but better compression for things like
character names), and tries (faster access, but sometimes bigger than sorted
ranges).

Project home page: https://github.com/BurntSushi/ucd-generate";

const ABOUT_BIDI_CLASS: &'static str = "\
bidi-class produces one table of Unicode codepoint ranges for each
possible Bidi_Class value.
";

const ABOUT_GENERAL_CATEGORY: &'static str = "\
general-category produces one table of Unicode codepoint ranges for each
possible General_Category value.
";

const ABOUT_SCRIPT: &'static str = "\
script produces one table of Unicode codepoint ranges for each possible Script
value.
";

const ABOUT_SCRIPT_EXTENSION: &'static str = "\
script-extension produces one table of Unicode codepoint ranges for each
possible Script_Extension value.
";

const ABOUT_JOINING_TYPE: &'static str = "\
joining-type produces one table of Unicode codepoint ranges for each
possible Joining_Type value.
";

const ABOUT_AGE: &'static str = "\
age produces a table for each discrete Unicode age. Each table includes the
codepoints that were added for that age. Tables can be emitted as a sorted
sequence of ranges, an FST or a trie.
";

const ABOUT_BIDI_MIRRORING_GLYPH: &'static str = "\
bidi-mirroring-glyph produces a table that maps codepoints that have the
Bidi_Mirrored=Yes property to another codepoint that typically has a glyph that
is the mirror image of the original codepoint's glyph.
";

const ABOUT_PROP_BOOL: &'static str = "\
property-bool produces possibly many tables for boolean properties. Tables can
be emitted as a sorted sequence of ranges, an FST or a trie.
";

const ABOUT_PERL_WORD: &'static str = "\
perl-word emits a table of codepoints in Unicode's definition of the \\w
character class, according to Annex C in UTS#18. In particular, this includes
the Alphabetic and Join_Control properties, in addition to the Decimal_Number,
Mark and Connector_Punctuation general categories.

Commands for \\s and \\d are not provided, since they directly correspond
to the property Whitespace and the general category Decimal_Number,
respectively.

The flags for this command are similar as the flags for property-bool.
";

const ABOUT_JAMO_SHORT_NAME: &'static str = "\
jamo-short-name parses the UCD's Jamo.txt file and emits its contents as a
slice table. The slice consists of a sorted sequences of pairs, where each
pair corresponds to the codepoint and the Jamo_Short_Name property value.

When emitted as an FST table, the FST corresponds to a map from a Unicode
codepoint (encoded as a big-endian u32) to a u64, where the u64 contains the
Jamo_Short_Name property value. The value is encoded in the least significant
bytes (up to 3).

Since the table is so small, the slice table is faster to search.
";

const ABOUT_NAMES: &'static str = "\
names emits a table of all character names in the UCD, including aliases and
names that are algorithmically generated such as Hangul syllables and
ideographs. Flags can be provided to tweak this behavior.

This table maps character names to codepoints.
";

const ABOUT_TEST_UNICODE_DATA: &'static str = "\
test-unicode-data parses the UCD's UnicodeData.txt file and emits its contents
on stdout. The purpose of this command is to diff the output with the input and
confirm that they are identical. This is a sanity test on the UnicodeData.txt
parser.
";

const ABOUT_PROPERTY_NAMES: &'static str = "\
property-names emits a table of all property aliases that map to a canonical
property name.
";

const ABOUT_PROPERTY_VALUES: &'static str = "\
property-values emits a table of all property values and their aliases that map
to a canonical property value.
";

const ABOUT_CASE_FOLDING_SIMPLE: &'static str = "\
case-folding emits a table of Simple case folding mappings from codepoint
to codepoint. When codepoints are mapped according to this table, then case
differences (according to Unicode) are eliminated.
";
const ABOUT_CASE_MAPPING: &'static str = "\
case-mapping emits case mapping tables, which map from a codepoint to a
list of codepoints (currently up to three), and are used to convert
text between lower, upper, and title cases.

This command currently has no support for emitting the conditional case
mapping data, and can only produce the unconditional mapping tables.
";
const ABOUT_GRAPHEME_CLUSTER_BREAK: &'static str = "\
grapheme-cluster-break emits the table of property values and their
corresponding codepoints for the Grapheme_Cluster_Break property.
";

const ABOUT_WORD_BREAK: &'static str = "\
word-break emits the table of property values and their corresponding
codepoints for the Word_Break property.
";

const ABOUT_SENTENCE_BREAK: &'static str = "\
sentence-break emits the table of property values and their corresponding
codepoints for the Sentence_Break property.
";

const ABOUT_DFA: &'static str = "\
dfa emits a single serialized DFAs from an arbitrary regular expression. If
you want a regular expression for finding the start and end of a match, then
use the 'regex' sub-command. Otherwise, if you only care about the end of a
match (forward DFA, the default) or the start of a match (reverse DFA), then
only a single DFA is necessary.
";

const ABOUT_REGEX: &'static str = "\
regex emits serialized DFAs from arbitrary regular expressions.
";

/// Build a clap application.
pub fn app() -> App<'static, 'static> {
    // Various common flags and arguments.
    let flag_name = |default| {
        Arg::with_name("name")
            .long("name")
            .help("Set the name of the table in the emitted code.")
            .takes_value(true)
            .default_value(default)
    };
    let flag_chars = Arg::with_name("chars").long("chars").help(
        "Write codepoints as character literals. If a codepoint \
         cannot be written as a character literal, then it is \
         silently dropped.",
    );
    let flag_short_names = Arg::with_name("short-names")
        .long("short-names")
        .help("Use the abbreviated property names in generated files.");
    let flag_trie_set = Arg::with_name("trie-set").long("trie-set").help(
        "Write codepoint sets as a compressed trie. \
         Code using this trie depends on the ucd_trie crate.",
    );
    let flag_fst_dir = Arg::with_name("fst-dir")
        .long("fst-dir")
        .help("Emit the table as a FST in Rust source code.")
        .takes_value(true);
    let ucd_dir = Arg::with_name("ucd-dir")
        .required(true)
        .help("Directory containing the Unicode character database files.");
    let flag_combined = Arg::with_name("combined").long("combined").help(
        "Emit a single table with all included codepoint ranges. You might \
        want to use this option when checking if characters belong to a \
        subset of categories, since only one table will need to be checked. \
        Searching the combined table should be simpler and more efficient.",
    );

    // Subcommands.
    let cmd_bidi_class = SubCommand::with_name("bidi-class")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the Bidi_Class property tables.")
        .before_help(ABOUT_BIDI_CLASS)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_name("BIDI_CLASS"))
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(flag_short_names.clone())
        .arg(flag_combined.clone())
        .arg(
            Arg::with_name("enum").long("enum").help(
                "Emit a single table that maps codepoints to bidi class.",
            ),
        )
        .arg(Arg::with_name("rust-enum").long("rust-enum").help(
            "Emit a Rust enum and a table that maps codepoints to bidi class.",
        ))
        .arg(
            Arg::with_name("list-classes")
                .long("list-classes")
                .help("List all of the bidi class names with abbreviations."),
        );
    let cmd_bidi_mirroring_glyph =
        SubCommand::with_name("bidi-mirroring-glyph")
            .author(clap::crate_authors!())
            .version(clap::crate_version!())
            .template(TEMPLATE_SUB)
            .about("Create Unicode Bidi Mirroring Glyph table.")
            .before_help(ABOUT_BIDI_MIRRORING_GLYPH)
            .arg(ucd_dir.clone())
            .arg(flag_fst_dir.clone())
            .arg(flag_name("BIDI_MIRRORING_GLYPH"))
            .arg(flag_chars.clone())
            .arg(flag_trie_set.clone())
            .arg(Arg::with_name("rust-match").long("rust-match").help(
                "Emit a function that uses a match to map between codepoints.",
            ));
    let cmd_general_category = SubCommand::with_name("general-category")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the General_Category property tables.")
        .before_help(ABOUT_GENERAL_CATEGORY)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_name("GENERAL_CATEGORY"))
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(flag_combined.clone())
        .arg(
            Arg::with_name("enum").long("enum").help(
                "Emit a single table that maps codepoints to categories.",
            ),
        )
        .arg(Arg::with_name("rust-enum").long("rust-enum").help(
            "Emit a Rust enum and a table that maps codepoints to categories.",
        ))
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of categories to include. \
             When absent, all categories are included.",
        ))
        .arg(Arg::with_name("exclude").long("exclude").takes_value(true).help(
            "A comma separated list of categories to exclude. \
             When absent, no categories are excluded. This overrides \
             categories specified with the --include flag.",
        ))
        .arg(
            Arg::with_name("list-categories")
                .long("list-categories")
                .help("List all of the category names with abbreviations."),
        );
    let cmd_script = SubCommand::with_name("script")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the Script property tables.")
        .before_help(ABOUT_SCRIPT)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_name("SCRIPT"))
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(flag_combined.clone())
        .arg(
            Arg::with_name("enum")
                .long("enum")
                .help("Emit a single table that maps codepoints to scripts."),
        )
        .arg(Arg::with_name("rust-enum").long("rust-enum").help(
            "Emit a Rust enum and a table that maps codepoints to scripts.",
        ))
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of scripts to include. \
             When absent, all scripts are included.",
        ))
        .arg(Arg::with_name("exclude").long("exclude").takes_value(true).help(
            "A comma separated list of scripts to exclude. \
             When absent, no scripts are excluded. This overrides \
             scripts specified with the --include flag.",
        ))
        .arg(
            Arg::with_name("list-scripts")
                .long("list-scripts")
                .help("List all of the script names with abbreviations."),
        );
    let cmd_script_extension = SubCommand::with_name("script-extension")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the Script_Extension property tables.")
        .before_help(ABOUT_SCRIPT_EXTENSION)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_name("SCRIPT_EXTENSION"))
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of script extensions to include. \
             When absent, all scripts extensions are included.",
        ))
        .arg(Arg::with_name("exclude").long("exclude").takes_value(true).help(
            "A comma separated list of script extensions to exclude. \
             When absent, no script extensions are excluded. This \
             overrides script extensions specified with the --include \
             flag.",
        ))
        .arg(
            Arg::with_name("list-script-extensions")
                .long("list-script-extensions")
                .help(
                    "List all of the script extension names with \
                     abbreviations.",
                ),
        );
    let cmd_age = SubCommand::with_name("age")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create Unicode Age tables.")
        .before_help(ABOUT_AGE)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(Arg::with_name("list-properties").long("list-properties").help(
            "List the properties that can be generated with this \
             command.",
        ));
    let cmd_joining_type =
        SubCommand::with_name("joining-type")
            .author(clap::crate_authors!())
            .version(clap::crate_version!())
            .template(TEMPLATE_SUB)
            .about("Create the Joining_Type property tables.")
            .before_help(ABOUT_JOINING_TYPE)
            .arg(ucd_dir.clone())
            .arg(flag_fst_dir.clone())
            .arg(flag_name("JOINING_TYPE"))
            .arg(flag_chars.clone())
            .arg(flag_trie_set.clone())
            .arg(flag_combined.clone())
            .arg(Arg::with_name("enum").long("enum").help(
                "Emit a single table that maps codepoints to joining type.",
            ))
            .arg(Arg::with_name("rust-enum").long("rust-enum").help(
                "Emit a Rust enum and a table that maps codepoints to \
                 joining type.",
            ));
    let cmd_prop_bool = SubCommand::with_name("property-bool")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create boolean property tables.")
        .before_help(ABOUT_PROP_BOOL)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of properties to include. \
             When absent, all available properties are included.",
        ))
        .arg(Arg::with_name("exclude").long("exclude").takes_value(true).help(
            "A comma separated list of properties to exclude. \
             When absent, no properties are excluded. This overrides \
             properties specified with the --include flag.",
        ))
        .arg(Arg::with_name("list-properties").long("list-properties").help(
            "List the properties that can be generated with this \
             command.",
        ));
    let cmd_perl_word = SubCommand::with_name("perl-word")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create a boolean property table for the \\w character class.")
        .before_help(ABOUT_PERL_WORD)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(flag_name("PERL_WORD"));
    let cmd_jamo_short_name = SubCommand::with_name("jamo-short-name")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the Jamo_Short_Name property table.")
        .before_help(ABOUT_JAMO_SHORT_NAME)
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_name("JAMO_SHORT_NAME"));
    let cmd_names =
        SubCommand::with_name("names")
            .author(clap::crate_authors!())
            .version(clap::crate_version!())
            .template(TEMPLATE_SUB)
            .about("Create a mapping from character name to codepoint.")
            .before_help(ABOUT_NAMES)
            .arg(ucd_dir.clone())
            .arg(flag_fst_dir.clone())
            .arg(flag_chars.clone().conflicts_with("tagged"))
            .arg(flag_name("NAMES"))
            .arg(Arg::with_name("no-aliases").long("no-aliases").help(
                "Ignore all character name aliases. When used, every name \
                 maps to exactly one codepoint.",
            ))
            .arg(Arg::with_name("no-ideograph").long("no-ideograph").help(
                "Do not include algorithmically generated ideograph names.",
            ))
            .arg(Arg::with_name("no-hangul").long("no-hangul").help(
                "Do not include algorithmically generated Hangul syllable \
                 names.",
            ))
            .arg(Arg::with_name("tagged").long("tagged").help(
                "Tag each codepoint with how the name was derived. \
                 The lower 32 bits corresponds to the codepoint. Bit 33 \
                 indicates the name was explicitly provided in \
                 UnicodeData.txt. Bit 34 indicates the name is from \
                 NameAliases.txt. \
                 Bit 35 indicates the name is a Hangul syllable. Bit 36 \
                 indicates the name is an ideograph.",
            ))
            .arg(Arg::with_name("normalize").long("normalize").help(
                "Normalize all character names according to UAX44-LM2.",
            ));
    let cmd_property_names = SubCommand::with_name("property-names")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the canonical property name table.")
        .before_help(ABOUT_PROPERTY_NAMES)
        .arg(ucd_dir.clone())
        .arg(flag_name("PROPERTY_NAMES"))
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of property names to include. \
             When absent, all property names are included.",
        ))
        .arg(
            Arg::with_name("exclude").long("exclude").takes_value(true).help(
                "A comma separated list of property names to exclude. \
                 When absent, no property names are excluded. This \
                 overrides property names specified with the --include \
                 flag.",
            ),
        );
    let cmd_property_values = SubCommand::with_name("property-values")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create the canonical property value table.")
        .before_help(ABOUT_PROPERTY_VALUES)
        .arg(ucd_dir.clone())
        .arg(flag_name("PROPERTY_VALUES"))
        .arg(Arg::with_name("include").long("include").takes_value(true).help(
            "A comma separated list of property names to include. \
             When absent, all property values for all properties are \
             included.",
        ))
        .arg(
            Arg::with_name("exclude").long("exclude").takes_value(true).help(
                "A comma separated list of property names to exclude. \
                 When absent, no property values are excluded. This \
                 overrides property names specified with the --include \
                 flag.",
            ),
        );
    let cmd_case_folding_simple = SubCommand::with_name("case-folding-simple")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create a case folding table using the simple mapping.")
        .before_help(ABOUT_CASE_FOLDING_SIMPLE)
        .arg(flag_name("CASE_FOLDING_SIMPLE"))
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(
            Arg::with_name("circular")
                .long("circular")
                .help("Emit a table where mappings are circular."),
        )
        .arg(Arg::with_name("all-pairs").long("all-pairs").help(
            "Emit a table where each codepoint includes all possible \
             Simple mappings.",
        ));
    let cmd_case_mapping = SubCommand::with_name("case-mapping")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create unconditional case mapping tables for upper, lower and title case.")
        .before_help(ABOUT_CASE_MAPPING)
        .arg(flag_name("CASE_MAPPING"))
        .arg(ucd_dir.clone())
        .arg(flag_chars.clone())
        .arg(Arg::with_name("simple").long("simple").help(
            "Only emit the simple case mapping tables \
             (emit maps of codepoint to codepoint, \
             ignoring rules from SpecialCasing.txt)",
        ));

    let cmd_grapheme_cluster_break =
        SubCommand::with_name("grapheme-cluster-break")
            .author(clap::crate_authors!())
            .version(clap::crate_version!())
            .template(TEMPLATE_SUB)
            .about("Create a table for each Grapheme_Cluster_Break value.")
            .before_help(ABOUT_GRAPHEME_CLUSTER_BREAK)
            .arg(flag_name("GRAPHEME_CLUSTER_BREAK"))
            .arg(ucd_dir.clone())
            .arg(flag_fst_dir.clone())
            .arg(flag_chars.clone())
            .arg(flag_trie_set.clone())
            .arg(
                Arg::with_name("enum").long("enum").help(
                    "Emit a single table that maps codepoints to values.",
                ),
            );
    let cmd_word_break = SubCommand::with_name("word-break")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create a table for each Word_Break value.")
        .before_help(ABOUT_WORD_BREAK)
        .arg(flag_name("WORD_BREAK"))
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(
            Arg::with_name("enum")
                .long("enum")
                .help("Emit a single table that maps codepoints to values."),
        );
    let cmd_sentence_break = SubCommand::with_name("sentence-break")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Create a table for each Sentence_Break value.")
        .before_help(ABOUT_SENTENCE_BREAK)
        .arg(flag_name("SENTENCE_BREAK"))
        .arg(ucd_dir.clone())
        .arg(flag_fst_dir.clone())
        .arg(flag_chars.clone())
        .arg(flag_trie_set.clone())
        .arg(
            Arg::with_name("enum")
                .long("enum")
                .help("Emit a single table that maps codepoints to values."),
        );
    let cmd_dfa = SubCommand::with_name("dfa")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Serialize a single DFAs")
        .before_help(ABOUT_DFA)
        .arg(Arg::with_name("dfa-dir").help("Emit DFAs to this directory"))
        .arg(Arg::with_name("pattern"))
        .arg(flag_name("DFA"))
        .arg(Arg::with_name("sparse").long("sparse"))
        .arg(Arg::with_name("anchored").long("anchored"))
        .arg(Arg::with_name("minimize").long("minimize"))
        .arg(Arg::with_name("classes").long("classes"))
        .arg(Arg::with_name("premultiply").long("premultiply"))
        .arg(Arg::with_name("no-utf8").long("no-utf8"))
        .arg(Arg::with_name("longest").long("longest"))
        .arg(Arg::with_name("reverse").long("reverse"))
        .arg(
            Arg::with_name("state-size")
                .long("state-size")
                .possible_values(&["1", "2", "4", "8"])
                .default_value("4"),
        );
    let cmd_regex = SubCommand::with_name("regex")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Serialize regular expression DFAs.")
        .before_help(ABOUT_REGEX)
        .arg(Arg::with_name("dfa-dir").help("Emit DFAs to this directory"))
        .arg(Arg::with_name("pattern"))
        .arg(flag_name("REGEX"))
        .arg(Arg::with_name("sparse").long("sparse"))
        .arg(Arg::with_name("anchored").long("anchored"))
        .arg(Arg::with_name("minimize").long("minimize"))
        .arg(Arg::with_name("classes").long("classes"))
        .arg(Arg::with_name("premultiply").long("premultiply"))
        .arg(Arg::with_name("no-utf8").long("no-utf8"))
        .arg(
            Arg::with_name("state-size")
                .long("state-size")
                .possible_values(&["1", "2", "4", "8"])
                .default_value("4"),
        );

    let cmd_test_unicode_data = SubCommand::with_name("test-unicode-data")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .template(TEMPLATE_SUB)
        .about("Test the UnicodeData.txt parser.")
        .before_help(ABOUT_TEST_UNICODE_DATA)
        .arg(ucd_dir.clone());

    // The actual App.
    App::new("ucd-generate")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .about(ABOUT)
        .template(TEMPLATE)
        .max_term_width(100)
        .setting(AppSettings::UnifiedHelpMessage)
        .subcommand(cmd_bidi_class)
        .subcommand(cmd_general_category)
        .subcommand(cmd_script)
        .subcommand(cmd_script_extension)
        .subcommand(cmd_joining_type)
        .subcommand(cmd_age)
        .subcommand(cmd_bidi_mirroring_glyph)
        .subcommand(cmd_prop_bool)
        .subcommand(cmd_perl_word)
        .subcommand(cmd_jamo_short_name)
        .subcommand(cmd_names)
        .subcommand(cmd_property_names)
        .subcommand(cmd_property_values)
        .subcommand(cmd_case_folding_simple)
        .subcommand(cmd_case_mapping)
        .subcommand(cmd_grapheme_cluster_break)
        .subcommand(cmd_word_break)
        .subcommand(cmd_sentence_break)
        .subcommand(cmd_dfa)
        .subcommand(cmd_regex)
        .subcommand(cmd_test_unicode_data)
}
