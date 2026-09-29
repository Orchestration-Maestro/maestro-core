//! Canonical JSON serialization: typed field order, exact bytes, no extra keys.

use super::{
    error::{Error, require},
    mapping::MappingLedger,
    types::DeliveryGraph,
};
use serde::{Serialize, de::DeserializeOwned};

/// Reads a typed payload and refuses any byte-level serialization ambiguity.
pub(crate) fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, Error> {
    let value = serde_json::from_slice::<T>(bytes)?;
    require(serde_json::to_vec(&value)? == bytes, "noncanonical JSON")?;
    Ok(value)
}

impl DeliveryGraph {
    /// Reads canonical graph bytes, refusing duplicate or unknown fields.
    ///
    /// # Errors
    /// Invalid JSON, digest syntax or noncanonical byte serialization.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        decode(bytes)
    }

    /// Serializes the graph; its CAS digest belongs outside these bytes.
    ///
    /// # Errors
    /// JSON serialization failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}

impl MappingLedger {
    /// Reads canonical, separately versioned mapping bytes.
    ///
    /// # Errors
    /// Invalid JSON or noncanonical serialization.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        decode(bytes)
    }

    /// Serializes the mapping ledger in field order.
    ///
    /// # Errors
    /// JSON serialization failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(self)?)
    }
}
