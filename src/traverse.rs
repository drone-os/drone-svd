use crate::device::{Cluster, RegisterTree};
use crate::variant::Variant;
use crate::{Peripheral, Register};
use eyre::Result;
use std::collections::HashSet;

pub(crate) fn traverse_peripheral_registers<'a>(
    peripheral: &'a Peripheral,
    parent: Option<&'a Peripheral>,
    f: impl FnMut(Vec<&'a Cluster>, &'a Register) -> Result<()>,
) -> Result<()> {
    let mut paths = Vec::<(Vec<&Cluster>, _)>::new();
    if let Some(peripheral) = parent {
        paths.push((Vec::new(), peripheral.registers.values()));
    }
    paths.push((Vec::new(), peripheral.registers.values()));
    traverse_registers(paths, f)
}

pub(crate) fn traverse_registers<'a>(
    mut paths: Vec<(Vec<&'a Cluster>, indexmap::map::Values<'a, String, RegisterTree>)>,
    mut f: impl FnMut(Vec<&'a Cluster>, &'a Register) -> Result<()>,
) -> Result<()> {
    let mut visited = HashSet::new();
    while let Some((path, tree)) = paths.pop() {
        for node in tree {
            match node {
                RegisterTree::Register(register) => {
                    let key = (
                        path.iter().map(|cluster| &cluster.name).collect::<Vec<_>>(),
                        &register.name,
                    );
                    if !visited.contains(&key) {
                        f(path.clone(), register)?;
                        visited.insert(key);
                    }
                }
                RegisterTree::Cluster(cluster) => {
                    let mut path = path.clone();
                    path.push(cluster);
                    paths.push((path, cluster.register.values()));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn for_each_clusters_combination<T: Clone>(
    variants: &[Variant<'_>],
    init: T,
    mut f: impl FnMut(T, &Cluster, u32) -> Result<T>,
    mut g: impl FnMut(&[T]) -> Result<()>,
) -> Result<()> {
    let mut matrix =
        vec![vec![0; variants.len()]; variants.iter().map(|v| v.clusters.len()).max().unwrap_or(0)];
    for (i, variant) in variants.iter().enumerate() {
        for (j, cluster) in variant.clusters.iter().enumerate() {
            matrix[j][i] = cluster.dim.unwrap_or(1);
        }
    }
    let dim = matrix.iter().map(|row| row.iter().copied().max().unwrap_or(1)).collect::<Vec<_>>();
    'outer: for n in 0..dim.iter().product() {
        let mut combination = Vec::new();
        for (i, variant) in variants.iter().enumerate() {
            let mut init = init.clone();
            let mut k = n;
            for (j, (cluster, dim)) in variant.clusters.iter().zip(dim.iter()).enumerate() {
                let cluster_n = (k % dim) as u32;
                if cluster_n >= matrix[j][i] {
                    continue 'outer;
                }
                k /= dim;
                init = f(init, cluster, cluster_n)?;
            }
            combination.push(init);
        }
        g(&combination)?;
    }
    Ok(())
}
