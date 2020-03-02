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
mod interrupt;
mod register;
mod traverse;
mod variant;

pub use device::{Access, Device, Field, Interrupt, Peripheral, Register};

use self::{
    interrupt::generate_interrupts,
    register::{generate_index, generate_registers},
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
        trace_variants(&mut device, &self)?;
        generate_registers(output, &device, pool_number, pool_size, &self)?;
        Ok(())
    }

    /// Generates registers index and interrupt bindings.
    pub fn generate_rest(
        self,
        index_output: &mut File,
        interrupts_output: &mut File,
        mut device: Device,
    ) -> Result<()> {
        trace_variants(&mut device, &self)?;
        generate_index(index_output, &device, &self)?;
        generate_interrupts(interrupts_output, &device)?;
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
