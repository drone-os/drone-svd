use crate::device::{Cluster, RegisterTree};
use crate::traverse::{traverse_peripheral_registers, traverse_registers};
use crate::{Device, Peripheral, Register};
use eyre::{eyre, Result};
use indexmap::IndexMap;

#[derive(Debug)]
pub(crate) struct Variant<'a> {
    pub(crate) peripheral: &'a Peripheral,
    pub(crate) clusters: Vec<&'a Cluster>,
    pub(crate) register: &'a Register,
}

impl<'a> Variant<'a> {
    fn new(peripheral: &'a Peripheral, clusters: Vec<&'a Cluster>, register: &'a Register) -> Self {
        Self { peripheral, clusters, register }
    }
}

pub(crate) fn trace_variants(device: &mut Device, exclude_peripherals: &[&str]) -> Result<()> {
    fn peripheral_variants<'a, 'b>(
        device: &'a mut Device,
        periheral_name: &'b str,
    ) -> Option<&'a mut Vec<String>> {
        device.peripherals.get_mut(periheral_name).map(|p| &mut p.variants)
    }
    for key in device.peripherals.keys().cloned().collect::<Vec<_>>() {
        if exclude_peripherals.iter().any(|&name| name == key) {
            continue;
        }
        let peripheral = device.peripherals.get_mut(&key).unwrap();
        trace_tree(&mut peripheral.registers)?;
        if let Some(alternate_peripheral) = peripheral.alternate_peripheral.as_ref().cloned() {
            let variants = peripheral_variants(device, &alternate_peripheral)
                .ok_or_else(|| eyre!("peripheral referenced in `alternatePeripheral` not found"))?
                .clone();
            for variant in variants {
                peripheral_variants(device, &variant).unwrap().push(key.clone());
            }
            peripheral_variants(device, &alternate_peripheral).unwrap().push(key);
        }
    }
    Ok(())
}

pub(crate) fn collect_variants<'a>(
    device: &'a Device,
    peripheral: &'a Peripheral,
    parent: Option<&'a Peripheral>,
    clusters: &'a [&'a Cluster],
    register: &'a Register,
) -> Result<Vec<Variant<'a>>> {
    fn is_paths_equal(
        clusters_a: &[&Cluster],
        register_a: &Register,
        clusters_b: &[&Cluster],
        register_b: &Register,
    ) -> bool {
        clusters_a.iter().map(|c| c.address_offset).sum::<u32>() + register_a.address_offset
            == clusters_b.iter().map(|c| c.address_offset).sum::<u32>() + register_b.address_offset
    }

    fn peripheral_get<'a, 'b>(
        peripheral: &'a Peripheral,
        parent: Option<&'a Peripheral>,
        name: &'b str,
    ) -> Option<&'a RegisterTree> {
        peripheral.registers.get(name).or_else(|| parent.and_then(|p| p.registers.get(name)))
    }

    let mut variants = vec![Variant::new(peripheral, clusters.to_vec(), register)];

    for o_register in &register.variants {
        let o_register = clusters.last().map_or_else(
            || peripheral_get(peripheral, parent, o_register),
            |cluster| cluster.register.get(o_register),
        );
        let o_register = o_register.unwrap().unwrap_register_ref();
        variants.push(Variant::new(peripheral, clusters.to_vec(), o_register));
    }

    for i in (0..clusters.len()).rev() {
        let cluster = clusters[i];
        let ancestor_clusters = &clusters[..i];
        let descendant_clusters = &clusters[i..];
        for o_cluster in &cluster.variants {
            let o_cluster = if i > 0 {
                clusters[i - 1].register.get(o_cluster)
            } else {
                peripheral_get(peripheral, parent, o_cluster)
            };
            let o_cluster = o_cluster.unwrap().unwrap_cluster_ref();
            let paths = vec![(Vec::new(), o_cluster.register.values())];
            traverse_registers(paths, |o_clusters, o_register| {
                if is_paths_equal(&o_clusters, o_register, descendant_clusters, register) {
                    let mut clusters =
                        Vec::with_capacity(ancestor_clusters.len() + o_clusters.len());
                    clusters.extend(ancestor_clusters);
                    clusters.extend(o_clusters);
                    variants.push(Variant::new(peripheral, clusters, o_register));
                }
                Ok(())
            })?;
        }
    }

    for o_peripheral in &peripheral.variants {
        let o_peripheral = device.peripherals.get(o_peripheral).unwrap();
        let o_parent = o_peripheral.derived_from(device)?;
        traverse_peripheral_registers(o_peripheral, o_parent, |o_clusters, o_register| {
            if is_paths_equal(&o_clusters, o_register, clusters, register) {
                variants.push(Variant::new(o_peripheral, o_clusters, o_register));
            }
            Ok(())
        })?;
    }

    Ok(variants)
}

fn trace_tree(tree: &mut IndexMap<String, RegisterTree>) -> Result<()> {
    fn cluster_variants<'a, 'b>(
        tree: &'a mut IndexMap<String, RegisterTree>,
        cluster_name: &'b str,
    ) -> Option<&'a mut Vec<String>> {
        tree.get_mut(cluster_name).map(|c| &mut c.unwrap_cluster_mut().variants)
    }
    for key in tree.keys().cloned().collect::<Vec<_>>() {
        match tree.get_mut(&key).unwrap() {
            RegisterTree::Register(register) => {
                if let Some(alternate_register) = register.alternate_register.as_ref().cloned() {
                    tree.get_mut(&alternate_register)
                        .ok_or_else(|| {
                            eyre!("register referenced in `alternateRegister` not found")
                        })?
                        .unwrap_register_mut()
                        .variants
                        .push(key);
                }
            }
            RegisterTree::Cluster(cluster) => {
                trace_tree(&mut cluster.register)?;
                if let Some(alternate_cluster) = cluster.alternate_cluster.as_ref().cloned() {
                    let variants = cluster_variants(tree, &alternate_cluster)
                        .ok_or_else(|| eyre!("cluster referenced in `alternateCluster` not found"))?
                        .clone();
                    for variant in variants {
                        cluster_variants(tree, &variant).unwrap().push(key.clone());
                    }
                    cluster_variants(tree, &alternate_cluster).unwrap().push(key);
                }
            }
        }
    }
    Ok(())
}
