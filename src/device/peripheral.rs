use super::{
    access::Access,
    deserialize_int, deserialize_int_opt,
    register::{deserialize_register_tree, tree_reg, tree_remove_reg, Register, RegisterTree},
    Device,
};
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::Deserialize;

/// A peripheral of a device.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Peripheral {
    /// The peripheral name from which to inherit data.
    pub derived_from: Option<String>,
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// The string identifies the peripheral.
    pub name: String,
    /// The string provides an overview of the purpose and functionality of the peripheral.
    pub description: Option<String>,
    /// A peripheral redefining an address block needs to specify the name of
    /// the peripheral that is listed first in the description.
    pub alternate_peripheral: Option<String>,
    /// Lowest address reserved or used by the peripheral.
    #[serde(deserialize_with = "deserialize_int")]
    pub base_address: u32,
    /// Default bit-width of any register contained in the peripheral.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub size: Option<u32>,
    /// Default value for all registers in the peripheral at RESET.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub reset_value: Option<u32>,
    /// Default access rights for all registers in the peripheral.
    pub access: Option<Access>,
    /// Associated interrupts.
    #[serde(default)]
    pub interrupt: Vec<Interrupt>,
    pub(crate) registers: Option<Registers>,
    #[serde(skip)]
    pub(crate) variants: Vec<String>,
}

/// An interrupt associated with a peripheral.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Interrupt {
    /// The string represents the interrupt name.
    pub name: String,
    /// The string describes the interrupt.
    #[serde(default)]
    pub description: String,
    /// Represents the enumeration index value associated to the interrupt.
    #[serde(deserialize_with = "deserialize_int")]
    pub value: u32,
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Registers {
    #[serde(rename = "$value", deserialize_with = "deserialize_register_tree")]
    pub(crate) tree: IndexMap<String, RegisterTree>,
}

impl Peripheral {
    /// Returns a mutable reference to the register at the path `path`.
    pub fn reg(&mut self, path: &str) -> &mut Register {
        tree_reg(&mut self.registers.as_mut().unwrap().tree, path)
    }

    /// Adds a new register `register`.
    pub fn add_reg(&mut self, register: Register) {
        self.registers
            .get_or_insert_with(Default::default)
            .tree
            .insert(register.name.clone(), RegisterTree::Register(register));
    }

    /// Adds a new register initialized by `f`.
    pub fn new_reg(&mut self, f: impl FnOnce(&mut Register)) {
        let mut register = Register::default();
        f(&mut register);
        self.add_reg(register);
    }

    /// Removes the register at the path `path`.
    pub fn remove_reg(&mut self, path: &str) -> Register {
        tree_remove_reg(&mut self.registers.as_mut().unwrap().tree, path)
    }

    pub(crate) fn derived_from<'a>(&'a self, device: &'a Device) -> Result<Option<&'a Self>> {
        Ok(if let Some(derived_from) = &self.derived_from {
            Some(
                device
                    .peripherals
                    .peripheral
                    .get(derived_from)
                    .ok_or_else(|| anyhow!("peripheral referenced in `derivedFrom` not found"))?,
            )
        } else {
            None
        })
    }

    pub(crate) fn description<'a>(&'a self, parent: Option<&'a Peripheral>) -> Result<&'a str> {
        self.description
            .as_ref()
            .or_else(|| parent.and_then(|parent| parent.description.as_ref()))
            .map(String::as_str)
            .ok_or_else(|| anyhow!("peripheral description not found"))
    }
}
