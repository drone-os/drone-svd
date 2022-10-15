use super::access::Access;
use super::register::{tree_reg, tree_remove_reg, Register, RegisterTree};
use super::{deserialize_int, deserialize_int_opt, Device};
use eyre::{eyre, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

/// A peripheral of a device.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peripheral {
    /// The peripheral name from which to inherit data.
    pub derived_from: Option<String>,
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array
    /// members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// The string identifies the peripheral.
    pub name: String,
    /// The string provides an overview of the purpose and functionality of the
    /// peripheral.
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
    pub(crate) registers: Option<Registers>,
    #[serde(skip)]
    pub(crate) variants: Vec<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
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
                    .ok_or_else(|| eyre!("peripheral referenced in `derivedFrom` not found"))?,
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
            .ok_or_else(|| eyre!("peripheral description not found"))
    }
}

fn deserialize_register_tree<'de, D>(
    deserializer: D,
) -> Result<IndexMap<String, RegisterTree>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = IndexMap::new();
    for tree in Vec::<RegisterTree>::deserialize(deserializer)? {
        let name = match &tree {
            RegisterTree::Register(register) => register.name.clone(),
            RegisterTree::Cluster(cluster) => cluster.name.clone(),
        };
        map.insert(name, tree);
    }
    Ok(map)
}
