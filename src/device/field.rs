use super::access::{Access, AccessWrapper};
use super::{deserialize_int_opt, parse_int};
use serde::de::Deserializer;
use serde::{de, Deserialize};
use std::num::ParseIntError;
use std::ops::RangeInclusive;

/// Bit-field properties of a register.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array
    /// members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// Name string used to identify the field.
    pub name: String,
    /// String describing the details of the register.
    #[serde(default)]
    pub description: String,
    /// The position of the least significant bit of the field within the
    /// register.
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
    /// A string in the format: `[<msb>:<lsb>]`.
    #[serde(default, deserialize_with = "deserialize_bit_range")]
    pub bit_range: Option<RangeInclusive<u32>>,
    /// The access type.
    #[serde(default, with = "AccessWrapper")]
    pub access: Option<Access>,
    /// Force the field to have multiple bits interface.
    #[serde(skip)]
    pub force_bits: bool,
}

impl Field {
    /// Returns the position of the least significant bit of the field within
    /// the register.
    pub fn bit_offset(&self) -> u32 {
        self.bit_offset
            .or(self.lsb)
            .or_else(|| self.bit_range.as_ref().map(|r| *r.start()))
            .expect("bit-range is missing")
    }

    /// Returns the bit-width of the bitfield within the register.
    pub fn bit_width(&self) -> u32 {
        self.bit_width
            .or_else(|| self.lsb.and_then(|lsb| self.msb.map(|msb| msb - lsb + 1)))
            .or_else(|| self.bit_range.as_ref().map(|r| *r.end() - *r.start() + 1))
            .expect("bit-range is missing")
    }
}

fn deserialize_bit_range<'de, D>(deserializer: D) -> Result<Option<RangeInclusive<u32>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?.map_or(Ok(None), |s| {
        s.strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .and_then(|s| s.split_once(':'))
            .map(|(msb, lsb)| Ok(parse_int(lsb)?..=parse_int(msb)?))
            .transpose()
            .map_err(de::Error::custom::<ParseIntError>)
    })
}
