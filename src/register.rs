use crate::{deserialize_hex, field::Field, Access};
use failure::Error;
use serde::Deserialize;
use std::{fs::File, io::Write};

/// The description of a register.
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Register {
    /// String to identify the register.
    pub name: String,
    /// String describing the details of the register.
    pub description: String,
    /// The address offset relative to the enclosing element.
    #[serde(deserialize_with = "deserialize_hex")]
    pub address_offset: u32,
    /// The bit-width of the register.
    #[serde(deserialize_with = "deserialize_hex")]
    pub size: u32,
    /// The access rights for the register.
    pub access: Option<Access>,
    /// The default value for the register at RESET.
    #[serde(deserialize_with = "deserialize_hex")]
    pub reset_value: u32,
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
            let &Field {
                ref name,
                ref description,
                bit_offset,
                bit_width,
                access,
            } = field;
            for line in description.lines() {
                writeln!(regs, "  /// {}", line.trim())?;
            }
            write!(regs, "  {} {{ {} {}", name, bit_offset, bit_width)?;
            match access.as_ref().or_else(|| base_access.as_ref()) {
                Some(&Access::WriteOnly) => {
                    write!(regs, " WWRegField")?;
                    write!(regs, " WoWRegField")?;
                }
                Some(&Access::ReadOnly) => {
                    write!(regs, " RRRegField")?;
                    write!(regs, " RoRRegField")?;
                }
                Some(&Access::ReadWrite) | None => {
                    write!(regs, " RRRegField")?;
                    write!(regs, " WWRegField")?;
                }
            }
            writeln!(regs, " }}")?;
        }
        Ok(())
    }
}
