/*!
The `ucd-util` crate contains a smattering of utility functions that implement
various algorithms specified by Unicode. There is no specific goal for
exhaustiveness. Instead, implementations should be added on an as-needed basis.

A *current* design constraint of this crate is that it should not bring in any
large Unicode tables. For example, to use the various property name and value
canonicalization functions, you'll need to supply your own table, which can
be generated using `ucd-generate`.
*/

#![deny(missing_docs)]
#![allow(unknown_lints)]
#![allow(ellipsis_inclusive_range_patterns)]

mod hangul;
mod ideograph;
mod name;
mod property;
mod unicode_tables;

pub use crate::hangul::{
    hangul_full_canonical_decomposition, hangul_name, RANGE_HANGUL_SYLLABLE,
};
pub use crate::ideograph::{ideograph_name, RANGE_IDEOGRAPH};
pub use crate::name::{character_name_normalize, symbolic_name_normalize};
pub use crate::property::{
    canonical_property_name, canonical_property_value, property_values,
    PropertyTable, PropertyValueTable, PropertyValues,
};
