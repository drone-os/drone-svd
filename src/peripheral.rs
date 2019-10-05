use crate::{
    deserialize_dec, deserialize_hex, device::Peripherals, register::Register, Access, BIT_BAND,
};
use failure::{err_msg, Error};
use serde::Deserialize;
use std::{
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
    /// The string identifies the peripheral.
    pub name: String,
    /// The string provides an overview of the purpose and functionality of the peripheral.
    pub description: Option<String>,
    /// Lowest address reserved or used by the peripheral.
    #[serde(deserialize_with = "deserialize_hex")]
    pub base_address: u32,
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
    pub description: String,
    /// Represents the enumeration index value associated to the interrupt.
    #[serde(deserialize_with = "deserialize_dec")]
    pub value: u32,
}

#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Registers {
    pub(crate) register: Vec<Register>,
}

struct GenIter<T, G: Generator<Yield = T, Return = ()>>(G);

impl Peripheral {
    /// Returns a mutable reference to the peripheral with name `name`.
    pub fn reg(&mut self, name: &str) -> &mut Register {
        self.registers
            .as_mut()
            .unwrap()
            .register
            .iter_mut()
            .find(|register| register.name == name)
            .unwrap()
    }

    /// Adds a new register `register`.
    pub fn add_reg(&mut self, register: Register) {
        self.registers
            .get_or_insert_with(Default::default)
            .register
            .push(register);
    }

    /// Adds a new register initialized by `f`.
    pub fn new_reg(&mut self, f: impl FnOnce(&mut Register)) {
        let mut register = Register::default();
        f(&mut register);
        self.add_reg(register);
    }

    /// Removes the register with name `name`.
    pub fn remove_reg(&mut self, name: &str) -> Register {
        let index = self
            .registers
            .as_ref()
            .unwrap()
            .register
            .iter()
            .position(|register| register.name == name)
            .unwrap();
        self.registers.as_mut().unwrap().register.remove(index)
    }

    pub(crate) fn generate_regs(
        &self,
        peripherals: &Peripherals,
        regs: &mut File,
        pool_number: usize,
        pool_size: usize,
        counter: &mut usize,
    ) -> Result<(), Error> {
        let parent = self.derived_from(peripherals)?;
        for register in self.registers(parent) {
            *counter += 1;
            if *counter % pool_size != pool_number - 1 {
                continue;
            }
            let &Register {
                ref name,
                ref description,
                address_offset,
                size,
                access,
                reset_value,
                ref fields,
            } = register;
            let address = self.base_address + address_offset;
            writeln!(regs, "reg! {{")?;
            for line in description.lines() {
                writeln!(regs, "  /// {}", line.trim())?;
            }
            writeln!(regs, "  pub mod {} {};", self.name, name)?;
            writeln!(
                regs,
                "  0x{:04X}_{:04X} {} 0x{:04X}_{:04X}",
                address >> 16,
                address & 0xFFFF,
                size,
                reset_value >> 16,
                reset_value & 0xFFFF,
            )?;
            write!(regs, " ")?;
            match access {
                Some(Access::WriteOnly) => {
                    write!(regs, " WReg")?;
                    write!(regs, " WoReg")?;
                }
                Some(Access::ReadOnly) => {
                    write!(regs, " RReg")?;
                    write!(regs, " RoReg")?;
                }
                Some(Access::ReadWrite) | None => {
                    write!(regs, " RReg")?;
                    write!(regs, " WReg")?;
                }
            }
            if BIT_BAND.contains(&address) {
                write!(regs, " RegBitBand")?;
            }
            writeln!(regs, ";")?;
            if let Some(fields) = fields {
                fields.generate_regs(access, regs)?;
            }
            writeln!(regs, "}}")?;
        }
        Ok(())
    }

    pub(crate) fn generate_rest(
        &self,
        peripherals: &Peripherals,
        int_names: &mut HashSet<String>,
        reg_index: &mut File,
        interrupts: &mut File,
        except: &[&str],
    ) -> Result<(), Error> {
        let parent = self.derived_from(peripherals)?;
        if except.iter().all(|&except| except != self.name) {
            let description = self
                .description
                .as_ref()
                .or_else(|| parent.and_then(|x| x.description.as_ref()))
                .ok_or_else(|| err_msg("Peripheral description not found"))?;
            for line in description.lines() {
                writeln!(reg_index, "  /// {}", line.trim())?;
            }
            writeln!(reg_index, "  pub mod {} {{", self.name)?;
            for register in self.registers(parent) {
                let Register { name, .. } = register;
                writeln!(reg_index, "    {};", name)?;
            }
            writeln!(reg_index, "  }}")?;
        }
        for interrupt in &self.interrupt {
            if int_names.insert(interrupt.name.to_owned()) {
                let &Interrupt {
                    ref name,
                    ref description,
                    value,
                } = interrupt;
                writeln!(interrupts, "thr::int! {{")?;
                for line in description.lines() {
                    writeln!(interrupts, "  /// {}", line.trim())?;
                }
                writeln!(interrupts, "  pub trait {}: {};", name, value)?;
                writeln!(interrupts, "}}")?;
            }
        }
        Ok(())
    }

    fn derived_from<'a>(&'a self, peripherals: &'a Peripherals) -> Result<Option<&'a Self>, Error> {
        Ok(if let Some(derived_from) = &self.derived_from {
            Some(
                peripherals
                    .peripheral
                    .get(derived_from)
                    .ok_or_else(|| err_msg("Peripheral `derivedFrom` not found"))?,
            )
        } else {
            None
        })
    }

    fn registers<'a>(&'a self, parent: Option<&'a Self>) -> impl Iterator<Item = &'a Register> {
        GenIter::new(static move || {
            let mut visited = HashSet::new();
            let direct = self.registers.iter().flat_map(|x| x.register.iter());
            for register in direct {
                visited.insert(&register.name);
                yield register;
            }
            let inherited = parent
                .iter()
                .flat_map(|x| x.registers.iter().flat_map(|x| x.register.iter()));
            for register in inherited {
                if !visited.contains(&register.name) {
                    yield register;
                }
            }
        })
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
