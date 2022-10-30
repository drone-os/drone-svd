//! CMSIS-SVD parser for Drone, an Embedded Operating System.
//!
//! # Documentation
//!
//! - [Drone Book](https://book.drone-os.com/)
//! - [API documentation](https://api.drone-os.com/drone-svd/0.15/)
//!
//! # Usage
//!
//! Add the crate to your `Cargo.toml` dependencies:
//!
//! ```toml
//! [dependencies]
//! drone-svd = { version = "0.15.0" }
//! ```

#![warn(missing_docs, unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

mod device;
mod generator;
mod traverse;
mod variant;

pub use self::generator::Generator;
pub use device::{Access, Device, Field, Peripheral, Register};
use eyre::Result;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

/// Parse the SVD file at `path`.
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Device> {
    let mut input = BufReader::new(File::open(path)?);
    let mut xml = String::new();
    input.read_to_string(&mut xml)?;
    Ok(quick_xml::de::from_reader(xml.as_bytes())?)
}
