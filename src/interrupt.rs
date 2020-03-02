use crate::{Device, Interrupt, Peripheral};
use anyhow::Result;
use std::{collections::HashSet, fs::File, io::Write};

pub(crate) fn generate_interrupts(output: &mut File, device: &Device) -> Result<()> {
    let mut int_names = HashSet::new();
    for peripheral in device.peripherals.peripheral.values() {
        generate_peripheral(output, peripheral, &mut int_names)?;
    }
    Ok(())
}

fn generate_peripheral(
    output: &mut File,
    peripheral: &Peripheral,
    int_names: &mut HashSet<String>,
) -> Result<()> {
    for interrupt in &peripheral.interrupt {
        if int_names.insert(interrupt.name.to_owned()) {
            let Interrupt { name, description, value } = interrupt;
            writeln!(output, "thr::int! {{")?;
            for line in description.lines() {
                writeln!(output, "    /// {}", line.trim())?;
            }
            writeln!(output, "    pub trait {}: {};", name, value)?;
            writeln!(output, "}}")?;
        }
    }
    Ok(())
}
