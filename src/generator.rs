use crate::traverse::{for_each_clusters_combination, traverse_peripheral_registers};
use crate::variant::{collect_variants, trace_variants};
use crate::{Access, Device, Field, Peripheral, Register};
use eyre::Result;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::mem;

pub trait RegisterTraitsCallback: Fn(String, Vec<String>, u32) -> Vec<String> {}

impl<T: Fn(String, Vec<String>, u32) -> Vec<String>> RegisterTraitsCallback for T {}

pub trait CoreRegPredicate: Fn(String, Vec<String>) -> bool {}

impl<T: Fn(String, Vec<String>) -> bool> CoreRegPredicate for T {}

/// Memory-mapped register bindings generator.
pub struct Generator<'a> {
    macro_name: &'a str,
    exclude_peripherals: Vec<&'a str>,
    register_traits_callback: Option<Box<dyn RegisterTraitsCallback>>,
    core_regs: Option<(&'a str, &'a str, Box<dyn CoreRegPredicate>)>,
}

impl<'a> Generator<'a> {
    /// Creates a blank new set of options ready for configuration.
    pub fn new(macro_name: &'a str) -> Self {
        Self {
            macro_name,
            exclude_peripherals: Vec::new(),
            register_traits_callback: None,
            core_regs: None,
        }
    }

    /// Extracts core-level registers into a separate set of tokens.
    pub fn core_regs(
        &mut self,
        macro_name: &'a str,
        prev_macro: &'a str,
        core_reg_predicate: impl CoreRegPredicate + 'static,
    ) -> &mut Self {
        self.core_regs = Some((macro_name, prev_macro, Box::new(core_reg_predicate)));
        self
    }

    /// Extends the list of peripherals to exclude from generated bindings.
    pub fn exclude_peripherals(&mut self, exclude_peripherals: &[&'a str]) -> &mut Self {
        self.exclude_peripherals.extend(exclude_peripherals);
        self
    }

    /// Sets a callback function to provide additional register traits based on
    /// its memory address.
    pub fn register_traits_callback(
        &mut self,
        register_traits_callback: impl RegisterTraitsCallback + 'static,
    ) -> &mut Self {
        self.register_traits_callback = Some(Box::new(register_traits_callback));
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
        trace_variants(&mut device, &self.exclude_peripherals)?;
        let mut counter = 0;
        let stagger = move || {
            counter += 1;
            counter % pool_size != pool_number - 1
        };
        let mut generated = HashSet::new();
        for peripheral in device.peripherals.values() {
            if self.exclude_peripherals.iter().any(|&name| name == peripheral.name) {
                continue;
            }
            generate_peripheral(
                output,
                &device,
                peripheral,
                &mut generated,
                stagger,
                &self.register_traits_callback,
            )?;
        }
        Ok(())
    }

    /// Generates registers index.
    pub fn generate_index(self, index_output: &mut File, mut device: Device) -> Result<()> {
        normalize(&mut device);
        trace_variants(&mut device, &self.exclude_peripherals)?;
        let output: &mut File = index_output;
        let mut index = IndexMap::new();
        for peripheral in device.peripherals.values() {
            if self.exclude_peripherals.iter().any(|&name| name == peripheral.name) {
                continue;
            }
            generate_peripheral_index(&device, peripheral, &mut index)?;
        }
        generate_reg_tokens(
            output,
            &device,
            &index,
            &format!("Defines an index of {} MCU-level register tokens.", device.name),
            self.macro_name,
            None,
            self.core_regs.as_ref().map(|(_, _, core_regs_predicate)| core_regs_predicate),
            false,
        )?;
        if let Some((macro_name, prev_macro, core_regs_predicate)) = &self.core_regs {
            writeln!(output)?;
            generate_reg_tokens(
                output,
                &device,
                &index,
                &format!("Defines an index of {} core-level register tokens.", device.name),
                macro_name,
                Some(prev_macro),
                Some(core_regs_predicate),
                true,
            )?;
        }
        Ok(())
    }
}

struct Instance {
    description: Vec<String>,
    peripheral_name: String,
    name: Vec<String>,
    address: u32,
    size: u32,
    reset_value: u32,
    access: Option<Access>,
}

fn generate_peripheral(
    output: &mut File,
    device: &Device,
    peripheral: &Peripheral,
    generated: &mut HashSet<(String, Vec<String>)>,
    mut stagger: impl FnMut() -> bool,
    register_traits_callback: &Option<Box<dyn RegisterTraitsCallback>>,
) -> Result<()> {
    let parent = peripheral.derived_from(device)?;
    traverse_peripheral_registers(peripheral, parent, |clusters, register| {
        let variants = collect_variants(device, peripheral, parent, &clusters, register)?;
        let register_data = variants
            .iter()
            .map(|variant| {
                let parent = variant.peripheral.derived_from(device)?;
                let mut description = Vec::new();
                for cluster in &variant.clusters {
                    description.push(cluster.description.clone());
                }
                description.push(variant.register.description.clone());
                Ok((
                    description,
                    variant.register.size(device, variant.peripheral, parent)?,
                    variant.register.reset_value(device, variant.peripheral, parent)?,
                    variant.register.access(device, variant.peripheral, parent),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        for peripheral_n in
            0..variants.iter().map(|v| v.peripheral.dim.unwrap_or(1)).max().unwrap_or(1)
        {
            let peripheral_data = variants
                .iter()
                .map(|variant| {
                    (
                        dim_name(peripheral_n, &variant.peripheral.name),
                        peripheral_n * variant.peripheral.dim_increment.unwrap_or(0),
                    )
                })
                .collect::<Vec<_>>();
            for_each_clusters_combination(
                &variants,
                (Vec::new(), 0),
                |(mut name, mut address), cluster, cluster_n| {
                    name.push(dim_name(cluster_n, &cluster.name));
                    address +=
                        cluster.address_offset + cluster_n * cluster.dim_increment.unwrap_or(0);
                    Ok((name, address))
                },
                |clusters_data| {
                    'outer: for register_n in
                        0..variants.iter().map(|v| v.register.dim.unwrap_or(1)).max().unwrap_or(1)
                    {
                        let mut instances = Vec::new();
                        for (i, variant) in variants.iter().enumerate() {
                            if peripheral_n < variant.peripheral.dim.unwrap_or(1)
                                && register_n < variant.register.dim.unwrap_or(1)
                            {
                                let (peripheral_name, peripheral_offset) = &peripheral_data[i];
                                let (clusters_name, clusters_address) = &clusters_data[i];
                                let (ref description, size, reset_value, access) = register_data[i];
                                let mut name = clusters_name.clone();
                                name.push(dim_name(register_n, &variant.register.name));
                                let address = variant.peripheral.base_address
                                    + peripheral_offset
                                    + clusters_address
                                    + variant.register.address_offset
                                    + register_n * variant.register.dim_increment.unwrap_or(0);
                                if !generated.insert((peripheral_name.to_string(), name.clone())) {
                                    continue 'outer;
                                }
                                instances.push((variant.register, Instance {
                                    description: description.clone(),
                                    peripheral_name: peripheral_name.clone(),
                                    name,
                                    address,
                                    size,
                                    reset_value,
                                    access,
                                }));
                            }
                        }
                        if !stagger() {
                            generate_variants(output, &instances, register_traits_callback)?;
                        }
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

fn generate_peripheral_index(
    device: &Device,
    peripheral: &Peripheral,
    index: &mut IndexMap<String, IndexMap<Vec<String>, bool>>,
) -> Result<()> {
    let parent = peripheral.derived_from(device)?;
    traverse_peripheral_registers(peripheral, parent, |clusters, register| {
        let variants = collect_variants(device, peripheral, parent, &clusters, register)?;
        for peripheral_n in
            0..variants.iter().map(|v| v.peripheral.dim.unwrap_or(1)).max().unwrap_or(1)
        {
            let peripheral_name = variants
                .iter()
                .map(|v| dim_name(peripheral_n, &v.peripheral.name))
                .collect::<Vec<_>>();
            for_each_clusters_combination(
                &variants,
                Vec::new(),
                |mut name, cluster, cluster_n| {
                    name.push(dim_name(cluster_n, &cluster.name));
                    Ok(name)
                },
                |clusters_name| {
                    for register_n in
                        0..variants.iter().map(|v| v.register.dim.unwrap_or(1)).max().unwrap_or(1)
                    {
                        for (i, variant) in variants.iter().enumerate() {
                            if peripheral_n < variant.peripheral.dim.unwrap_or(1)
                                && register_n < variant.register.dim.unwrap_or(1)
                            {
                                let mut name = clusters_name[i].clone();
                                name.push(dim_name(register_n, &variant.register.name));
                                let peripheral =
                                    index.entry(peripheral_name[i].clone()).or_default();
                                if i == 0 {
                                    peripheral.entry(name).or_insert(true);
                                } else {
                                    peripheral.insert(name, false);
                                }
                            }
                        }
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

fn generate_variants(
    output: &mut File,
    instances: &[(&Register, Instance)],
    register_traits_callback: &Option<Box<dyn RegisterTraitsCallback>>,
) -> Result<()> {
    writeln!(output, "reg! {{")?;
    for (register, instance) in instances {
        let Instance { description, peripheral_name, name, address, size, reset_value, access } =
            instance;
        for description in description {
            for line in description.lines() {
                writeln!(output, "    /// {}", line.trim())?;
            }
        }
        write!(output, "    pub {} ", peripheral_name)?;
        for (i, name) in name.iter().enumerate() {
            if i > 0 {
                write!(output, "_")?;
            }
            write!(output, "{}", name)?;
        }
        writeln!(output, " => {{")?;
        writeln!(output, "        address => 0x{:04X}_{:04X};", address >> 16, address & 0xFFFF)?;
        writeln!(output, "        size => {};", size)?;
        writeln!(
            output,
            "        reset => 0x{:04X}_{:04X};",
            reset_value >> 16,
            reset_value & 0xFFFF
        )?;
        write!(output, "        traits => {{")?;
        match access {
            Some(Access::WriteOnly) => {
                write!(output, " WReg")?;
                write!(output, " WoReg")?;
            }
            Some(Access::ReadOnly) => {
                write!(output, " RReg")?;
                write!(output, " RoReg")?;
            }
            Some(Access::ReadWrite | Access::ReadWriteonce) | None => {
                write!(output, " RReg")?;
                write!(output, " WReg")?;
            }
        }
        if let Some(register_traits_callback) = register_traits_callback {
            for name in register_traits_callback(peripheral_name.clone(), name.clone(), *address) {
                write!(output, " {name}")?;
            }
        }
        writeln!(output, " }};")?;
        writeln!(output, "        fields => {{")?;
        for field in &register.fields {
            generate_field(output, field, *access)?;
        }
        writeln!(output, "        }};")?;
        writeln!(output, "    }};")?;
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn generate_field(output: &mut File, field: &Field, base_access: Option<Access>) -> Result<()> {
    for number in 0..field.dim.unwrap_or(1) {
        let offset = number * field.dim_increment.unwrap_or(0);
        for line in field.description.lines() {
            writeln!(output, "            /// {}", line.trim())?;
        }
        writeln!(output, "            {} => {{", dim_name(number, &field.name))?;
        writeln!(output, "                offset => {};", field.bit_offset() + offset)?;
        writeln!(output, "                width => {};", field.bit_width())?;
        write!(output, "                traits => {{")?;
        match field.access.or(base_access) {
            Some(Access::WriteOnly) => {
                write!(output, " WWRegField")?;
                write!(output, " WoWRegField")?;
            }
            Some(Access::ReadOnly) => {
                write!(output, " RRRegField")?;
                write!(output, " RoRRegField")?;
            }
            Some(Access::ReadWrite | Access::ReadWriteonce) | None => {
                write!(output, " RRRegField")?;
                write!(output, " WWRegField")?;
            }
        }
        if field.force_bits {
            write!(output, " ForceBits")?;
        }
        writeln!(output, " }};")?;
        writeln!(output, "            }};")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::borrowed_box)]
fn generate_reg_tokens(
    output: &mut File,
    device: &Device,
    index: &IndexMap<String, IndexMap<Vec<String>, bool>>,
    macro_doc: &str,
    macro_name: &str,
    prev_macro: Option<&str>,
    core_reg_predicate: Option<&Box<dyn CoreRegPredicate>>,
    core_regs: bool,
) -> Result<()> {
    writeln!(output, "reg::tokens! {{")?;
    writeln!(output, "    /// {}", macro_doc)?;
    writeln!(output, "    pub macro {};", macro_name)?;
    if let Some(prev_macro) = prev_macro {
        writeln!(output, "    use macro {};", prev_macro)?;
    }
    writeln!(output, "    super::inner;")?;
    writeln!(output, "    crate::reg;")?;
    for (peripheral_name, registers) in index {
        let peripheral = &device.peripherals[peripheral_name];
        let parent = peripheral.derived_from(device)?;
        if let Some(description) = peripheral.description(parent) {
            for line in description.lines() {
                writeln!(output, "    /// {}", line.trim())?;
            }
        }
        writeln!(
            output,
            "    pub mod {}{} {{",
            core_regs.then_some("!").unwrap_or_default(),
            peripheral_name
        )?;
        for (name, primary) in registers {
            let core_reg = core_reg_predicate.map_or(false, |predicate| {
                let core_reg = !predicate(peripheral_name.clone(), name.clone());
                if core_regs { core_reg } else { !core_reg }
            });
            write!(output, "        ")?;
            if !primary || (!core_regs && core_reg) {
                write!(output, "!")?;
            }
            for (i, name) in name.iter().enumerate() {
                if i > 0 {
                    write!(output, "_")?;
                }
                write!(output, "{}", name)?;
            }
            writeln!(output, ";")?;
        }
        writeln!(output, "    }}")?;
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn dim_name(number: u32, name: &str) -> String {
    if let Some(name) = name.strip_suffix("[%s]") {
        format!("{}_{}", name, number)
    } else {
        name.to_owned()
    }
}

fn normalize(device: &mut Device) {
    device.peripherals = mem::take(&mut device.peripherals)
        .into_iter()
        .map(|(_, peripheral)| (peripheral.name.clone(), peripheral))
        .collect();
}
