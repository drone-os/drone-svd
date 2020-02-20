//! CMSIS-SVD parser for Drone, an Embedded Operating System.
//!
//! # Documentation
//!
//! - [Drone Book](https://book.drone-os.com/)
//! - [API documentation](https://api.drone-os.com/drone-svd/0.11/)
//!
//! # Usage
//!
//! Place the following to the Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! drone-svd = { version = "0.11.1" }
//! ```

#![feature(generator_trait)]
#![feature(generators)]
#![deny(elided_lifetimes_in_paths)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]

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

use anyhow::{anyhow, Error};
use serde::{de, Deserialize, Deserializer};
use std::{
    env,
    fs::File,
    io::{prelude::*, BufReader},
    num::ParseIntError,
    ops::Range,
    path::Path,
};

/// Bit-band memory region.
pub const BIT_BAND: Range<u32> = 0x4000_0000..0x4010_0000;

/// Predefined access rights.
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Access {
    /// Read operations have an undefined result. Write access is permitted.
    WriteOnly,
    /// Read access is permitted. Write operations have an undefined result.
    ReadOnly,
    /// Read and write accesses are permitted. Writes affect the state of the
    /// register and reads return the register value.
    ReadWrite,
    /// Read access is always permitted. Only the first write access after a
    /// reset will have an effect on the content. Other write operations have an
    /// undefined result.
    ReadWriteonce,
}

/// Parse the SVD file at `path`.
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Device, Error> {
    let mut input = BufReader::new(File::open(path)?);
    let mut xml = String::new();
    input.read_to_string(&mut xml)?;
    serde_xml_rs::from_reader(xml.as_bytes()).map_err(|err| anyhow!("{}", err))
}

/// Instructs cargo to rerun the build script when RUSTFLAGS environment
/// variables changed.
pub fn rerun_if_env_changed() {
    for (var, _) in env::vars_os() {
        if let Some(var) = var.to_str() {
            if var.ends_with("RUSTFLAGS") {
                println!("cargo:rerun-if-env-changed={}", var);
            }
        }
    }
}

pub(crate) trait DimGroup {
    fn dim(&self) -> Option<(u32, u32)>;

    fn dim_index(&self) -> &Option<String>;

    fn name(&self) -> &String;

    fn dim_group(&self) -> Vec<(String, u32)> {
        if let Some((count, step)) = self.dim() {
            if count > 1 {
                let indices = self
                    .dim_index()
                    .as_ref()
                    .map(|idx| idx.split(',').into_iter().map(|s| s.to_owned()).collect::<Vec<_>>())
                    .unwrap_or_else(|| (0..count).map(|i| format!("{}", i)).collect::<Vec<_>>());
                return indices
                    .into_iter()
                    .enumerate()
                    .map(|(i, idx)| {
                        (
                            self.name().replace("[%s]", &format!("_{}", idx)).replace("%s", &idx),
                            i as u32 * step,
                        )
                    })
                    .collect();
            }
        }
        vec![(self.name().clone(), 0)]
    }
}

pub(crate) fn deserialize_int<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    parse_int(&String::deserialize(deserializer)?).map_err(de::Error::custom)
}

pub(crate) fn deserialize_int_opt<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    if let Some(s) = s { parse_int(&s).map(Some).map_err(de::Error::custom) } else { Ok(None) }
}

fn parse_int(src: &str) -> Result<u32, ParseIntError> {
    let mut range = 0..src.len();
    let radix = if src.starts_with("0x") || src.starts_with("0X") {
        range.start += 2;
        16
    } else if src.starts_with('0') && src.len() > 1 {
        range.start += 1;
        8
    } else {
        10
    };
    u32::from_str_radix(&src[range], radix)
}
