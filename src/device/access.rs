use serde::{Deserialize, Deserializer};

/// Predefined access rights.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Access {
    /// Read operations have an undefined result. Write access is permitted.
    WriteOnly,
    /// Read access is permitted. Write operations have an undefined result.
    ReadOnly,
    /// Read and write accesses are permitted. Writes affect the state of the
    /// register and reads return the register value.
    ReadWrite,
    /// Read access is always permitted. Only the first write access after a
    /// reset will have an effect on the content. Other write operations have an
    /// undefined result.
    ReadWriteonce,
}

#[derive(Deserialize)]
pub(crate) struct AccessWrapper {
    #[serde(rename = "$value")]
    value: Option<Access>,
}

impl AccessWrapper {
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Access>, D::Error> {
        Ok(<Self as Deserialize>::deserialize(deserializer)?.value)
    }
}
