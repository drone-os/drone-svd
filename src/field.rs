use crate::Access;
use serde::Deserialize;

/// Bit-field properties of a register.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Field {
    /// Name string used to identify the field.
    pub name: String,
    /// String describing the details of the register.
    pub description: String,
    /// The position of the least significant bit of the field within the register.
    pub bit_offset: usize,
    /// The bit-width of the bitfield within the register.
    pub bit_width: usize,
    /// The access type.
    pub access: Option<Access>,
}
