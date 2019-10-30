use crate::{deserialize_int_opt, peripheral::Peripheral, Access};
use anyhow::Result;
use serde::{Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::Write,
};

/// The outermost frame of the description.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    /// The string identifies the device or device series.
    pub name: String,
    /// Default bit-width of any register contained in the device.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub size: Option<u32>,
    /// Default value for all registers at RESET.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub reset_value: Option<u32>,
    /// Default access rights for all registers.
    pub access: Option<Access>,
    pub(crate) peripherals: Peripherals,
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Peripherals {
    #[serde(default, deserialize_with = "deserialize_peripheral")]
    pub(crate) peripheral: BTreeMap<String, Peripheral>,
}

impl Device {
    /// Creates a new empty device definition.
    pub fn new(name: String) -> Self {
        Self {
            name,
            size: None,
            reset_value: None,
            access: None,
            peripherals: Peripherals::default(),
        }
    }

    /// Returns a mutable reference to the peripheral with name `name`.
    pub fn periph(&mut self, name: &str) -> &mut Peripheral {
        self.peripherals.peripheral.get_mut(name).unwrap()
    }

    /// Inserts a new peripheral `peripheral`.
    pub fn add_periph(&mut self, peripheral: Peripheral) {
        self.peripherals
            .peripheral
            .insert(peripheral.name.clone(), peripheral);
    }

    /// Inserts a new peripheral initialized by `f`.
    pub fn new_periph(&mut self, f: impl FnOnce(&mut Peripheral)) {
        let mut peripheral = Peripheral::default();
        f(&mut peripheral);
        self.add_periph(peripheral);
    }

    /// Removes the peripheral with name `name`
    pub fn remove_periph(&mut self, name: &str) -> Peripheral {
        self.peripherals.peripheral.remove(name).unwrap()
    }

    /// Writes register binding definitions to the file `regs`.
    pub fn generate_regs(
        self,
        regs: &mut File,
        except: &[&str],
        pool_number: usize,
        pool_size: usize,
    ) -> Result<()> {
        let mut counter = 0;
        for peripheral in self.peripherals.peripheral.values() {
            if except.iter().any(|&name| name == peripheral.name) {
                continue;
            }
            peripheral.generate_regs(&self, regs, pool_number, pool_size, &mut counter)?;
        }
        Ok(())
    }

    /// Writes interrupt binding definitions to the file `interrupts` and
    /// register bindings index to the file `reg_index`.
    pub fn generate_rest(
        self,
        reg_index: &mut File,
        interrupts: &mut File,
        except: &[&str],
        reg_index_macro: &str,
    ) -> Result<()> {
        let mut int_names = HashSet::new();
        writeln!(reg_index, "reg::tokens! {{")?;
        writeln!(
            reg_index,
            "    /// Defines an index of {} register tokens.",
            self.name
        )?;
        writeln!(reg_index, "    pub macro {};", reg_index_macro)?;
        writeln!(
            reg_index,
            "    use macro drone_cortex_m::map::cortex_m_reg_tokens;"
        )?;
        writeln!(reg_index, "    super::inner;")?;
        writeln!(reg_index, "    crate::reg;")?;
        for peripheral in self.peripherals.peripheral.values() {
            peripheral.generate_rest(&self, &mut int_names, reg_index, interrupts, except)?;
        }
        writeln!(reg_index, "}}")?;
        Ok(())
    }
}

fn deserialize_peripheral<'de, D>(deserializer: D) -> Result<BTreeMap<String, Peripheral>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = BTreeMap::new();
    for peripheral in Vec::<Peripheral>::deserialize(deserializer)? {
        map.insert(peripheral.name.clone(), peripheral);
    }
    Ok(map)
}
