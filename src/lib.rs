//! CMSIS-SVD parser for Drone, an Embedded Operating System.
//!
//! # Documentation
//!
//! - [Drone Book](https://book.drone-os.com/)
//! - [API documentation](https://api.drone-os.com/drone-svd/0.14/)
//!
//! # Usage
//!
//! Add the crate to your `Cargo.toml` dependencies:
//!
//! ```toml
//! [dependencies]
//! drone-svd = { version = "0.14.0" }
//! ```

#![feature(bool_to_option)]
#![feature(cell_update)]
#![feature(generator_trait)]
#![feature(generators)]
#![feature(or_patterns)]
#![feature(unsafe_block_in_unsafe_fn)]
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
mod register;
mod traverse;
mod variant;

pub use device::{Access, Device, Field, Peripheral, Register};

use self::{
    register::{generate_index, generate_registers},
    variant::trace_variants,
};
use anyhow::{anyhow, Result};
use std::{
    env,
    fs::File,
    io::{prelude::*, BufReader},
    mem,
    ops::Range,
    path::Path,
};

/// Options to configure how bindings are generated.
pub struct Config<'a> {
    macro_name: &'a str,
    bit_band: Option<Range<u32>>,
    exclude_peripherals: Vec<&'a str>,
}

impl<'a> Config<'a> {
    /// Creates a blank new set of options ready for configuration.
    pub fn new(macro_name: &'a str) -> Self {
        Self { macro_name, bit_band: None, exclude_peripherals: Vec::new() }
    }

    /// Extends the list of peripherals to exclude from generated bindings.
    pub fn exclude_peripherals(&mut self, exclude_peripherals: &[&'a str]) -> &mut Self {
        self.exclude_peripherals.extend(exclude_peripherals);
        self
    }

    /// Sets bit-band memory region.
    pub fn bit_band(&mut self, bit_band: Range<u32>) -> &mut Self {
        self.bit_band = Some(bit_band);
        self
    }

    /// Generates register bindings.
    pub fn generate_regs(
        self,
        output: &mut File,
        mut device: Device,
        pool_number: usize,
        pool_size: usize,
    ) -> Result<()> {
        normalize(&mut device);
        trace_variants(&mut device, &self)?;
        generate_registers(output, &device, pool_number, pool_size, &self)?;
        Ok(())
    }

    /// Generates registers index.
    pub fn generate_index(self, index_output: &mut File, mut device: Device) -> Result<()> {
        normalize(&mut device);
        trace_variants(&mut device, &self)?;
        generate_index(index_output, &device, &self)?;
        Ok(())
    }
}

/// Parse the SVD file at `path`.
pub fn parse<P: AsRef<Path>>(path: P) -> Result<Device> {
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

fn normalize(device: &mut Device) {
    device.peripherals.peripheral = mem::take(&mut device.peripherals.peripheral)
        .into_iter()
        .map(|(_, peripheral)| (peripheral.name.clone(), peripheral))
        .collect();
}
