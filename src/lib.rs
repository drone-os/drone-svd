//! CMSIS-SVD parser for Drone, an Embedded Operating System.
//!
//! # Documentation
//!
//! - [Drone Book](https://book.drone-os.com/)
//! - [API documentation](https://api.drone-os.com/drone-cortex-m-svd/0.10/)
//!
//! # Usage
//!
//! Place the following to the Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! drone-cortex-m-svd = { version = "0.10.0" }
//! ```

#![feature(generator_trait)]
#![feature(generators)]
#![feature(non_exhaustive)]
#![deny(elided_lifetimes_in_paths)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

mod device;
mod field;
mod peripheral;
mod register;

pub use self::{
    device::Device,
    field::Field,
    peripheral::{Interrupt, Peripheral},
    register::Register,
};

use failure::Error;
use serde::{de, Deserialize, Deserializer};
use std::{
    fs::File,
    io::{prelude::*, BufReader},
    ops::Range,
    path::Path,
};

/// Bit-band memory region.
pub const BIT_BAND: Range<u32> = 0x4000_0000..0x4010_0000;

/// Predefined access rights.
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Access {
    /// Read operations have an undefined result.
    WriteOnly,
    /// Read access is permitted.
    ReadOnly,
    /// Read and write accesses are permitted.
    ReadWrite,
}

/// Parse the SVD file at `path`.
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Device, Error> {
    let mut input = BufReader::new(File::open(path)?);
    let mut xml = String::new();
    input.read_to_string(&mut xml)?;
    let device = serde_xml_rs::deserialize(xml.as_bytes())?;
    Ok(device)
}

fn deserialize_hex<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(&s, 16).map_err(de::Error::custom)
}

fn deserialize_dec<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u32::from_str_radix(&s, 10).map_err(de::Error::custom)
}
