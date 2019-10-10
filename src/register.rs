use crate::{deserialize_int, deserialize_int_opt, field::Field, Access, DimGroup};
use failure::Error;
use serde::{Deserialize, Deserializer};
use std::{fs::File, io::Write};

#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Deserialize)]
pub enum RegisterTree {
    Register(Register),
    Cluster(Cluster),
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Cluster {
    /// Define the number of elements in an array.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim: Option<u32>,
    /// Specify the address increment, in Bytes, between two neighboring array members in the address map.
    #[serde(default, deserialize_with = "deserialize_int_opt")]
    pub dim_increment: Option<u32>,
    /// String to identify the cluster.
    pub name: String,
    /// String describing the details of the register cluster.
    #[serde(default)]
    pub description: String,
    /// Cluster address relative to the <baseAddress> of the peripheral.
    #[serde(deserialize_with = "deserialize_int")]
    pub address_offset: u32,
    // See https://github.com/RReverser/serde-xml-rs/issues/55#issuecomment-473679067
    #[serde(default, deserialize_with = "deserialize_cluster_register")]
    pub(crate) register: Vec<RegisterTree>,
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
        self.fields
            .as_mut()
            .unwrap()
            .field
            .iter_mut()
            .find(|field| field.name == name)
            .unwrap()
    }

    /// Adds a new field `field`.
    pub fn add_field(&mut self, field: Field) {
        self.fields
            .get_or_insert_with(Default::default)
            .field
            .push(field);
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
}

impl Fields {
    pub(crate) fn generate_regs(
        &self,
        base_access: Option<Access>,
        regs: &mut File,
    ) -> Result<(), Error> {
        for field in &self.field {
            for (name, offset) in field.dim_group() {
                for line in field.description.lines() {
                    writeln!(regs, "    /// {}", line.trim())?;
                }
                write!(
                    regs,
                    "    {} {{ {} {}",
                    name,
                    field.bit_offset() + offset,
                    field.bit_width()
                )?;
                match field.access.or(base_access) {
                    Some(Access::WriteOnly) => {
                        write!(regs, " WWRegField")?;
                        write!(regs, " WoWRegField")?;
                    }
                    Some(Access::ReadOnly) => {
                        write!(regs, " RRRegField")?;
                        write!(regs, " RoRRegField")?;
                    }
                    Some(Access::ReadWrite) | Some(Access::ReadWriteonce) | None => {
                        write!(regs, " RRRegField")?;
                        write!(regs, " WWRegField")?;
                    }
                }
                writeln!(regs, "   }}")?;
            }
        }
        Ok(())
    }
}

pub(crate) trait RegisterTreeVec {
    fn reg(&mut self, path: &str) -> &mut Register;

    fn remove_reg(&mut self, path: &str) -> Register;
}

impl RegisterTreeVec for Vec<RegisterTree> {
    fn reg(&mut self, path: &str) -> &mut Register {
        let mut path = path.splitn(2, '/');
        let name = path.next().unwrap();
        for node in self {
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
                        return cluster.register.reg(path.next().unwrap());
                    }
                }
            }
        }
        panic!("register not found")
    }

    fn remove_reg(&mut self, path: &str) -> Register {
        let mut path = path.splitn(2, '/');
        let name = path.next().unwrap();
        for (i, node) in self.iter_mut().enumerate() {
            match node {
                RegisterTree::Register(register) => {
                    if register.name == name {
                        if path.next().is_none() {
                            if let RegisterTree::Register(register) = self.remove(i) {
                                return register;
                            } else {
                                unreachable!()
                            }
                        } else {
                            panic!("extra segments at the tail")
                        }
                    }
                }
                RegisterTree::Cluster(cluster) => {
                    if cluster.name == name {
                        return cluster.register.remove_reg(path.next().unwrap());
                    }
                }
            }
        }
        panic!("register not found");
    }
}

impl DimGroup for Cluster {
    fn dim(&self) -> Option<(u32, u32)> {
        self.dim
            .and_then(|dim| self.dim_increment.map(|dim_increment| (dim, dim_increment)))
    }

    fn name(&self) -> &String {
        &self.name
    }
}

impl DimGroup for Register {
    fn dim(&self) -> Option<(u32, u32)> {
        self.dim
            .and_then(|dim| self.dim_increment.map(|dim_increment| (dim, dim_increment)))
    }

    fn name(&self) -> &String {
        &self.name
    }
}

fn deserialize_cluster_register<'de, D>(deserializer: D) -> Result<Vec<RegisterTree>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut vec = Vec::new();
    for register in Vec::<Register>::deserialize(deserializer)? {
        vec.push(RegisterTree::Register(register));
    }
    Ok(vec)
}
