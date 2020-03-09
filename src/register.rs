use crate::{
    traverse::{for_each_clusters_combination, traverse_peripheral_registers},
    variant::collect_variants,
    Access, Config, Device, Field, Peripheral, Register,
};
use anyhow::Result;
use indexmap::IndexMap;
use std::{collections::HashSet, fs::File, io::Write};

struct Instance {
    description: Vec<String>,
    peripheral_name: String,
    name: Vec<String>,
    address: u32,
    size: u32,
    reset_value: u32,
    access: Option<Access>,
}

pub(crate) fn generate_registers(
    output: &mut File,
    device: &Device,
    pool_number: usize,
    pool_size: usize,
    config: &Config<'_>,
) -> Result<()> {
    let mut counter = 0;
    let stagger = move || {
        counter += 1;
        counter % pool_size != pool_number - 1
    };
    let mut generated = HashSet::new();
    for peripheral in device.peripherals.peripheral.values() {
        if config.exclude_peripherals.iter().any(|&name| name == peripheral.name) {
            continue;
        }
        generate_peripheral(output, &device, peripheral, &mut generated, stagger, config)?;
    }
    Ok(())
}

pub(crate) fn generate_index(
    output: &mut File,
    device: &Device,
    config: &Config<'_>,
) -> Result<()> {
    let mut index = IndexMap::new();
    for peripheral in device.peripherals.peripheral.values() {
        if config.exclude_peripherals.iter().any(|&name| name == peripheral.name) {
            continue;
        }
        generate_peripheral_index(device, peripheral, &mut index)?;
    }
    writeln!(output, "reg::tokens! {{")?;
    writeln!(output, "    /// Defines an index of {} register tokens.", device.name)?;
    writeln!(output, "    pub macro {};", config.macro_name)?;
    writeln!(output, "    use macro drone_cortex_m::map::cortex_m_reg_tokens;")?;
    writeln!(output, "    super::inner;")?;
    writeln!(output, "    crate::reg;")?;
    for (peripheral_name, registers) in index {
        let peripheral = &device.peripherals.peripheral[&peripheral_name];
        let parent = peripheral.derived_from(device)?;
        let description = peripheral.description(parent)?;
        for line in description.lines() {
            writeln!(output, "    /// {}", line.trim())?;
        }
        writeln!(output, "    pub mod {} {{", peripheral_name)?;
        for (name, primary) in registers {
            write!(output, "        ")?;
            if !primary {
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

fn generate_peripheral(
    output: &mut File,
    device: &Device,
    peripheral: &Peripheral,
    generated: &mut HashSet<(String, Vec<String>)>,
    mut stagger: impl FnMut() -> bool,
    config: &Config<'_>,
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
                                if !generated.insert((peripheral_name.to_string(), name.to_vec())) {
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
                            generate_variants(output, &instances, config)?;
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
    config: &Config<'_>,
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
        write!(output, "    pub mod {} ", peripheral_name)?;
        for (i, name) in name.iter().enumerate() {
            if i > 0 {
                write!(output, "_")?;
            }
            write!(output, "{}", name)?;
        }
        writeln!(output, ";")?;
        writeln!(
            output,
            "    0x{:04X}_{:04X} {} 0x{:04X}_{:04X}",
            address >> 16,
            address & 0xFFFF,
            size,
            reset_value >> 16,
            reset_value & 0xFFFF,
        )?;
        write!(output, "   ")?;
        match access {
            Some(Access::WriteOnly) => {
                write!(output, " WReg")?;
                write!(output, " WoReg")?;
            }
            Some(Access::ReadOnly) => {
                write!(output, " RReg")?;
                write!(output, " RoReg")?;
            }
            Some(Access::ReadWrite) | Some(Access::ReadWriteonce) | None => {
                write!(output, " RReg")?;
                write!(output, " WReg")?;
            }
        }
        if let Some(bit_band) = &config.bit_band {
            if bit_band.contains(&address) {
                write!(output, " RegBitBand")?;
            }
        }
        writeln!(output, ";")?;
        if let Some(fields) = &register.fields {
            for field in &fields.field {
                generate_field(output, field, *access)?;
            }
        }
    }
    writeln!(output, "}}")?;
    Ok(())
}

fn generate_field(output: &mut File, field: &Field, base_access: Option<Access>) -> Result<()> {
    for number in 0..field.dim.unwrap_or(1) {
        let offset = number * field.dim_increment.unwrap_or(0);
        for line in field.description.lines() {
            writeln!(output, "    /// {}", line.trim())?;
        }
        write!(
            output,
            "    {} {{ {} {}",
            dim_name(number, &field.name),
            field.bit_offset() + offset,
            field.bit_width()
        )?;
        match field.access.or(base_access) {
            Some(Access::WriteOnly) => {
                write!(output, " WWRegField")?;
                write!(output, " WoWRegField")?;
            }
            Some(Access::ReadOnly) => {
                write!(output, " RRRegField")?;
                write!(output, " RoRRegField")?;
            }
            Some(Access::ReadWrite) | Some(Access::ReadWriteonce) | None => {
                write!(output, " RRRegField")?;
                write!(output, " WWRegField")?;
            }
        }
        writeln!(output, " }}")?;
    }
    Ok(())
}

fn dim_name(number: u32, name: &str) -> String {
    if let Some(name) = name.strip_suffix("[%s]") {
        format!("{}_{}", name, number)
    } else {
        name.to_owned()
    }
}
