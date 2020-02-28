//! CMSIS-SVD parser for Drone, an Embedded Operating System.
//!
//! # Documentation
//!
//! - [Drone Book](https://book.drone-os.com/)
//! - [API documentation](https://api.drone-os.com/drone-svd/0.12/)
//!
//! # Usage
//!
//! Place the following to the Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! drone-svd = { version = "0.12.0" }
//! ```

#![feature(bool_to_option)]
#![feature(cell_update)]
#![feature(generator_trait)]
#![feature(generators)]
#![feature(str_strip)]
#![feature(track_caller)]
#![deny(elided_lifetimes_in_paths)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate
)]

mod device;
mod generate_interrupts;
mod generate_registers;
mod traverse;
mod variant;

pub use device::{Access, Device, Field, Interrupt, Peripheral, Register};
pub use generate_registers::generate_registers;

use self::{
    generate_interrupts::generate_interrupts, generate_registers::generate_index,
    variant::trace_variants,
};
use anyhow::{anyhow, Result};
use std::{
    env,
    fs::File,
    io::{prelude::*, BufReader},
    ops::Range,
    path::Path,
};

/// Bit-band memory region.
pub const BIT_BAND: Range<u32> = 0x4000_0000..0x4010_0000;

/// Parse the SVD file at `path`.
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Device> {
    let mut input = BufReader::new(File::open(path)?);
    let mut xml = String::new();
    input.read_to_string(&mut xml)?;
    serde_xml_rs::from_reader(xml.as_bytes()).map_err(|err| anyhow!("{}", err))
}

/// Writes registers index and interrupt bindings.
pub fn generate_rest(
    index_output: &mut File,
    interrupts_output: &mut File,
    mut device: Device,
    macro_name: &str,
    except: &[&str],
) -> Result<()> {
    trace_variants(&mut device, except)?;
    generate_index(index_output, &device, macro_name, except)?;
    generate_interrupts(interrupts_output, &device)?;
    Ok(())
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
