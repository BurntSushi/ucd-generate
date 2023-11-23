pub(crate) mod app;
pub(crate) mod args;
pub(crate) mod error;
pub(crate) mod util;
pub(crate) mod writer;

pub(crate) mod age;
pub(crate) mod bidi_class;
pub(crate) mod bidi_mirroring_glyph;
pub(crate) mod brk;
pub(crate) mod canonical_combining_class;
pub(crate) mod case_folding;
pub(crate) mod case_mapping;
pub(crate) mod general_category;
pub(crate) mod jamo_short_name;
pub(crate) mod joining_type;
pub(crate) mod names;
pub(crate) mod property_bool;
pub(crate) mod script;

macro_rules! err {
    ($($tt:tt)*) => {
        Err(crate::error::Error::Other(format!($($tt)*)))
    }
}

pub(crate) use err;
