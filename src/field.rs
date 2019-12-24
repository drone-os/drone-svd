use crate::{deserialize_int_opt, Access, DimGroup};
use serde::Deserialize;

/// Bit-field properties of a register.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Field {
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// Name string used to identify the field.
    pub name: String,
    /// String describing the details of the register.
    #[serde(default)]
    pub description: String,
    /// The position of the least significant bit of the field within the register.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub bit_offset: Option<u32>,
    /// The bit-width of the bitfield within the register.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub bit_width: Option<u32>,
    /// The bit position of the least significant bit within the register.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub lsb: Option<u32>,
    /// The bit position of the most significant bit within the register.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub msb: Option<u32>,
    /// The access type.
    pub access: Option<Access>,
}

impl Field {
    /// Returns the position of the least significant bit of the field within
    /// the register.
    pub fn bit_offset(&self) -> u32 {
        self.bit_offset.or(self.lsb).expect("bit-range is missing")
    }

    /// Returns the bit-width of the bitfield within the register.
    pub fn bit_width(&self) -> u32 {
        self.bit_width
            .or_else(|| self.lsb.and_then(|lsb| self.msb.map(|msb| msb - lsb + 1)))
            .expect("bit-range is missing")
    }
}

impl DimGroup for Field {
    fn dim(&self) -> Option<(u32, u32)> {
        self.dim.and_then(|dim| self.dim_increment.map(|dim_increment| (dim, dim_increment)))
    }

    fn name(&self) -> &String {
        &self.name
    }
}
