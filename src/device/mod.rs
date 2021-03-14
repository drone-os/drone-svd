mod access;
mod field;
mod peripheral;
mod register;

pub use self::{access::Access, field::Field, peripheral::Peripheral, register::Register};

pub(crate) use self::register::{Cluster, RegisterTree};

use anyhow::Result;
use indexmap::IndexMap;
use serde::{de, Deserialize, Deserializer};
use std::num::ParseIntError;

/// The outermost frame of the description.
#[non_exhaustive]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Peripherals {
    #[serde(default, deserialize_with = "deserialize_peripheral")]
    pub(crate) peripheral: IndexMap<String, Peripheral>,
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

    /// Returns an iterator over all peripheral names.
    pub fn periph_names(&self) -> impl Iterator<Item = &String> + '_ {
        self.peripherals.peripheral.keys()
    }

    /// Returns a mutable reference to the peripheral with name `name`.
    pub fn periph(&mut self, name: &str) -> &mut Peripheral {
        self.peripherals.peripheral.get_mut(name).unwrap()
    }

    /// Inserts a new peripheral `peripheral`.
    pub fn add_periph(&mut self, peripheral: Peripheral) {
        self.peripherals.peripheral.insert(peripheral.name.clone(), peripheral);
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
}

fn deserialize_peripheral<'de, D>(deserializer: D) -> Result<IndexMap<String, Peripheral>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = IndexMap::new();
    for peripheral in Vec::<Peripheral>::deserialize(deserializer)? {
        map.insert(peripheral.name.clone(), peripheral);
    }
    Ok(map)
}

fn deserialize_int<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    parse_int(&String::deserialize(deserializer)?).map_err(de::Error::custom)
}

fn deserialize_int_opt<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map_or(Ok(None), |s| parse_int(&s).map(Some).map_err(de::Error::custom))
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
