use crate::{
    deserialize_int, deserialize_int_opt,
    device::Device,
    register::{Cluster, Register, RegisterTree, RegisterTreeVec},
    Access, DimGroup, BIT_BAND,
};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{
    cmp::max,
    collections::HashSet,
    fs::File,
    io::Write,
    ops::{Generator, GeneratorState},
    pin::Pin,
};

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
    /// The string to prepend to every register name of this peripheral.
    #[serde(default)]
    pub prepend_to_name: Option<String>,
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
    #[serde(rename = "$value")]
    pub(crate) tree: Vec<RegisterTree>,
}

struct GenIter<T, G: Generator<Yield = T, Return = ()>>(G);

impl Peripheral {
    /// Returns a mutable reference to the register at the path `path`.
    pub fn reg(&mut self, path: &str) -> &mut Register {
        self.registers.as_mut().unwrap().tree.reg(path)
    }

    /// Adds a new register `register`.
    pub fn add_reg(&mut self, register: Register) {
        self.registers
            .get_or_insert_with(Default::default)
            .tree
            .push(RegisterTree::Register(register));
    }

    /// Adds a new register initialized by `f`.
    pub fn new_reg(&mut self, f: impl FnOnce(&mut Register)) {
        let mut register = Register::default();
        f(&mut register);
        self.add_reg(register);
    }

    /// Removes the register at the path `path`.
    pub fn remove_reg(&mut self, path: &str) -> Register {
        self.registers.as_mut().unwrap().tree.remove_reg(path)
    }

    pub(crate) fn generate_regs(
        &self,
        device: &Device,
        regs: &mut File,
        pool_number: usize,
        pool_size: usize,
        counter: &mut usize,
    ) -> Result<()> {
        let parent = self.derived_from(device)?;
        for (clusters, register) in self.registers(parent) {
            *counter += 1;
            if *counter % pool_size != pool_number - 1 {
                continue;
            }
            let mut description = register.description.clone();
            let size = register
                .size
                .or(self.size)
                .or_else(|| parent.and_then(|p| p.size))
                .or(device.size)
                .expect("missing register size");
            let reset_value = register
                .reset_value
                .or(self.reset_value)
                .or_else(|| parent.and_then(|p| p.reset_value))
                .or(device.reset_value)
                .expect("missing reset value");
            let access = register
                .access
                .or(self.access)
                .or_else(|| parent.and_then(|p| p.access))
                .or(device.access);
            for cluster in &clusters {
                description = format!("{}\n{}", cluster.description, description);
            }
            let clusters = clusters
                .into_iter()
                .map(|cluster| (cluster, cluster.dim_group()))
                .collect::<Vec<_>>();
            for (peripheral_name, peripheral_offset) in self.dim_group() {
                for i in 0..max(clusters.iter().map(|(_, group)| group.len()).sum(), 1) {
                    for (mut name, offset) in register.dim_group() {
                        let mut address = self.base_address
                            + peripheral_offset
                            + register.address_offset
                            + offset;
                        let mut j = i;
                        for (cluster, group) in &clusters {
                            let (cluster_name, cluster_offset) = &group[j % group.len()];
                            j /= group.len();
                            name = format!("{}_{}", cluster_name, name);
                            address += cluster.address_offset + cluster_offset;
                        }
                        writeln!(regs, "reg! {{")?;
                        for line in description.lines() {
                            writeln!(regs, "    /// {}", line.trim())?;
                        }
                        writeln!(regs, "    pub mod {} {};", peripheral_name, name)?;
                        writeln!(
                            regs,
                            "    0x{:04X}_{:04X} {} 0x{:04X}_{:04X}",
                            address >> 16,
                            address & 0xFFFF,
                            size,
                            reset_value >> 16,
                            reset_value & 0xFFFF,
                        )?;
                        write!(regs, "   ")?;
                        match access {
                            Some(Access::WriteOnly) => {
                                write!(regs, " WReg")?;
                                write!(regs, " WoReg")?;
                            }
                            Some(Access::ReadOnly) => {
                                write!(regs, " RReg")?;
                                write!(regs, " RoReg")?;
                            }
                            Some(Access::ReadWrite) | Some(Access::ReadWriteonce) | None => {
                                write!(regs, " RReg")?;
                                write!(regs, " WReg")?;
                            }
                        }
                        if BIT_BAND.contains(&address) {
                            write!(regs, " RegBitBand")?;
                        }
                        writeln!(regs, ";")?;
                        if let Some(fields) = &register.fields {
                            fields.generate_regs(
                                access,
                                self.prepend_to_name.as_deref().unwrap_or(""),
                                regs,
                            )?;
                        }
                        writeln!(regs, "}}")?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn generate_rest(
        &self,
        device: &Device,
        int_names: &mut HashSet<String>,
        reg_index: &mut File,
        interrupts: &mut File,
        except: &[&str],
    ) -> Result<()> {
        let parent = self.derived_from(device)?;
        if except.iter().all(|&except| except != self.name) {
            let description = self
                .description
                .as_ref()
                .or_else(|| parent.and_then(|x| x.description.as_ref()))
                .ok_or_else(|| anyhow!("Peripheral description not found"))?;
            for (peripheral_name, _) in self.dim_group() {
                for line in description.lines() {
                    writeln!(reg_index, "    /// {}", line.trim())?;
                }
                writeln!(reg_index, "    pub mod {} {{", peripheral_name)?;
                for (clusters, register) in self.registers(parent) {
                    let clusters = clusters
                        .into_iter()
                        .map(|cluster| (cluster, cluster.dim_group()))
                        .collect::<Vec<_>>();
                    for i in 0..max(clusters.iter().map(|(_, group)| group.len()).sum(), 1) {
                        for (mut name, _) in register.dim_group() {
                            let mut j = i;
                            for (_, group) in &clusters {
                                let (cluster_name, _) = &group[j % group.len()];
                                j /= group.len();
                                name = format!("{}_{}", cluster_name, name);
                            }
                            writeln!(reg_index, "        {};", name)?;
                        }
                    }
                }
                writeln!(reg_index, "    }}")?;
            }
        }
        for interrupt in &self.interrupt {
            if int_names.insert(interrupt.name.to_owned()) {
                let &Interrupt { ref name, ref description, value } = interrupt;
                writeln!(interrupts, "thr::int! {{")?;
                for line in description.lines() {
                    writeln!(interrupts, "    /// {}", line.trim())?;
                }
                writeln!(interrupts, "    pub trait {}: {};", name, value)?;
                writeln!(interrupts, "}}")?;
            }
        }
        Ok(())
    }

    fn derived_from<'a>(&'a self, device: &'a Device) -> Result<Option<&'a Self>> {
        Ok(if let Some(derived_from) = &self.derived_from {
            Some(
                device
                    .peripherals
                    .peripheral
                    .get(derived_from)
                    .ok_or_else(|| anyhow!("Peripheral `derivedFrom` not found"))?,
            )
        } else {
            None
        })
    }

    fn registers<'a>(
        &'a self,
        parent: Option<&'a Self>,
    ) -> impl Iterator<Item = (Vec<&'a Cluster>, &'a Register)> {
        GenIter::new(static move || {
            let mut visited = HashSet::new();
            let mut paths = Vec::<(Vec<&Cluster>, _)>::new();
            if let Some(parent) = parent {
                if let Some(registers) = &parent.registers {
                    paths.push((Vec::new(), registers.tree.iter()));
                }
            }
            if let Some(registers) = &self.registers {
                paths.push((Vec::new(), registers.tree.iter()));
            }
            while let Some((path, iter)) = paths.pop() {
                for node in iter {
                    match node {
                        RegisterTree::Register(register) => {
                            let key = (
                                path.iter().map(|cluster| &cluster.name).collect::<Vec<_>>(),
                                &register.name,
                            );
                            if !visited.contains(&key) {
                                yield (path.clone(), register);
                                visited.insert(key);
                            }
                        }
                        RegisterTree::Cluster(cluster) => {
                            let mut path = path.clone();
                            path.push(cluster);
                            paths.push((path, cluster.register.iter()));
                        }
                    }
                }
            }
        })
    }
}

impl DimGroup for Peripheral {
    fn dim(&self) -> Option<(u32, u32)> {
        self.dim.and_then(|dim| self.dim_increment.map(|dim_increment| (dim, dim_increment)))
    }

    fn name(&self) -> &String {
        &self.name
    }
}

impl<T, G> GenIter<T, G>
where
    G: Generator<Yield = T, Return = ()>,
{
    pub fn new(gen: G) -> Pin<Box<Self>> {
        Box::pin(Self(gen))
    }
}

impl<T, G> Iterator for Pin<Box<GenIter<T, G>>>
where
    G: Generator<Yield = T, Return = ()>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let gen = unsafe { self.as_mut().map_unchecked_mut(|x| &mut x.0) };
        match gen.resume() {
            GeneratorState::Yielded(item) => Some(item),
            GeneratorState::Complete(()) => None,
        }
    }
}
