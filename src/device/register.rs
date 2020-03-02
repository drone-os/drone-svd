use super::{
    access::Access, deserialize_int, deserialize_int_opt, field::Field, peripheral::Peripheral,
    Device,
};
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Deserialize)]
pub(crate) enum RegisterTree {
    Register(Register),
    Cluster(Cluster),
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Cluster {
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// String to identify the cluster.
    pub name: String,
    /// Specify the name of the original cluster if this cluster provides an
    /// alternative description.
    pub alternate_cluster: Option<String>,
    /// String describing the details of the register cluster.
    #[serde(default)]
    pub description: String,
    /// Cluster address relative to the <baseAddress> of the peripheral.
    #[serde(deserialize_with = "deserialize_int")]
    pub address_offset: u32,
    // See https://github.com/RReverser/serde-xml-rs/issues/55#issuecomment-473679067
    #[serde(default, deserialize_with = "deserialize_register_tree")]
    pub(crate) register: IndexMap<String, RegisterTree>,
    #[serde(skip)]
    pub(crate) variants: Vec<String>,
}

/// The description of a register.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Register {
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// String to identify the register.
    pub name: String,
    /// String describing the details of the register.
    #[serde(default)]
    pub description: String,
    /// This tag can reference a register that has been defined above to current
    /// location in the description and that describes the memory location
    /// already.
    pub alternate_register: Option<String>,
    /// The address offset relative to the enclosing element.
    #[serde(deserialize_with = "deserialize_int")]
    pub address_offset: u32,
    /// The bit-width of the register.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub size: Option<u32>,
    /// The access rights for the register.
    pub access: Option<Access>,
    /// The default value for the register at RESET.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub reset_value: Option<u32>,
    pub(crate) fields: Option<Fields>,
    #[serde(skip)]
    pub(crate) variants: Vec<String>,
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Fields {
    pub(crate) field: Vec<Field>,
}

impl Register {
    /// Returns a mutable reference to the field with name `name`.
    pub fn field(&mut self, name: &str) -> &mut Field {
        self.fields.as_mut().unwrap().field.iter_mut().find(|field| field.name == name).unwrap()
    }

    /// Adds a new field `field`.
    pub fn add_field(&mut self, field: Field) {
        self.fields.get_or_insert_with(Default::default).field.push(field);
    }

    /// Adds a new field initialized by `f`.
    pub fn new_field(&mut self, f: impl FnOnce(&mut Field)) {
        let mut field = Field::default();
        f(&mut field);
        self.add_field(field);
    }

    /// Removes the field with name `name`.
    pub fn remove_field(&mut self, name: &str) -> Field {
        let index = self
            .fields
            .as_ref()
            .unwrap()
            .field
            .iter()
            .position(|field| field.name == name)
            .unwrap();
        self.fields.as_mut().unwrap().field.remove(index)
    }

    pub(crate) fn size(
        &self,
        device: &Device,
        peripheral: &Peripheral,
        parent: Option<&Peripheral>,
    ) -> Result<u32> {
        self.size
            .or(peripheral.size)
            .or_else(|| parent.and_then(|peripheral| peripheral.size))
            .or(device.size)
            .ok_or_else(|| anyhow!("missing register size"))
    }

    pub(crate) fn reset_value(
        &self,
        device: &Device,
        peripheral: &Peripheral,
        parent: Option<&Peripheral>,
    ) -> Result<u32> {
        self.reset_value
            .or(peripheral.reset_value)
            .or_else(|| parent.and_then(|peripheral| peripheral.reset_value))
            .or(device.reset_value)
            .ok_or_else(|| anyhow!("missing reset value"))
    }

    pub(crate) fn access(
        &self,
        device: &Device,
        peripheral: &Peripheral,
        parent: Option<&Peripheral>,
    ) -> Option<Access> {
        self.access
            .or(peripheral.access)
            .or_else(|| parent.and_then(|peripheral| peripheral.access))
            .or(device.access)
    }
}

impl RegisterTree {
    #[track_caller]
    pub(crate) fn unwrap_register_ref(&self) -> &Register {
        match self {
            RegisterTree::Register(register) => register,
            RegisterTree::Cluster(_) => panic!(
                "called `RegisterTree::unwrap_register_ref()` on a `&RegisterTree::Cluster` value"
            ),
        }
    }

    pub(crate) fn unwrap_register_mut(&mut self) -> &mut Register {
        match self {
            RegisterTree::Register(register) => register,
            RegisterTree::Cluster(_) => panic!(
                "called `RegisterTree::unwrap_register_mut()` on a `&mut RegisterTree::Cluster` \
                 value"
            ),
        }
    }

    #[track_caller]
    pub(crate) fn unwrap_register(self) -> Register {
        match self {
            RegisterTree::Register(register) => register,
            RegisterTree::Cluster(_) => panic!(
                "called `RegisterTree::unwrap_register()` on a `RegisterTree::Cluster` value"
            ),
        }
    }

    #[track_caller]
    pub(crate) fn unwrap_cluster_ref(&self) -> &Cluster {
        match self {
            RegisterTree::Cluster(cluster) => cluster,
            RegisterTree::Register(_) => panic!(
                "called `RegisterTree::unwrap_cluster_ref()` on a `&RegisterTree::Register` value"
            ),
        }
    }

    #[track_caller]
    pub(crate) fn unwrap_cluster_mut(&mut self) -> &mut Cluster {
        match self {
            RegisterTree::Cluster(cluster) => cluster,
            RegisterTree::Register(_) => panic!(
                "called `RegisterTree::unwrap_cluster_mut()` on a `&mut RegisterTree::Register` \
                 value"
            ),
        }
    }
}

pub(crate) fn tree_reg<'a, 'b>(
    tree: &'a mut IndexMap<String, RegisterTree>,
    path: &'b str,
) -> &'a mut Register {
    let mut path = path.splitn(2, '/');
    let name = path.next().unwrap();
    for node in tree.values_mut() {
        match node {
            RegisterTree::Register(register) => {
                if register.name == name {
                    if path.next().is_none() {
                        return register;
                    } else {
                        panic!("extra segments at the tail")
                    }
                }
            }
            RegisterTree::Cluster(cluster) => {
                if cluster.name == name {
                    return tree_reg(&mut cluster.register, path.next().unwrap());
                }
            }
        }
    }
    panic!("register not found")
}

pub(crate) fn tree_remove_reg(tree: &mut IndexMap<String, RegisterTree>, path: &str) -> Register {
    let mut path = path.splitn(2, '/');
    let name = path.next().unwrap();
    for key in tree.keys().cloned().collect::<Vec<_>>() {
        match tree.get_mut(&key).unwrap() {
            RegisterTree::Register(register) => {
                if register.name == name {
                    if path.next().is_none() {
                        return tree.remove(&key).unwrap().unwrap_register();
                    } else {
                        panic!("extra segments at the tail")
                    }
                }
            }
            RegisterTree::Cluster(cluster) => {
                if cluster.name == name {
                    return tree_remove_reg(&mut cluster.register, path.next().unwrap());
                }
            }
        }
    }
    panic!("register not found");
}

fn deserialize_register_tree<'de, D>(
    deserializer: D,
) -> Result<IndexMap<String, RegisterTree>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = IndexMap::new();
    for register in Vec::<Register>::deserialize(deserializer)? {
        map.insert(register.name.clone(), RegisterTree::Register(register));
    }
    Ok(map)
}
