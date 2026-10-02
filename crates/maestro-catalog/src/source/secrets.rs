//! Typed secret bindings for registered settings, backend and extension fields.
//! These are references only: validation and serialization never read a binding.

use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;

/// Exactly one environment-variable or keychain-service/account reference.
/// Unknown keys, literal values and mixed forms are refused by the strict decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SecretReference {
    /// A runtime environment-variable binding, never its value.
    Environment {
        /// The nonblank variable name.
        #[serde(deserialize_with = "deserialize_nonblank")]
        env: String,
    },
    /// A runtime keychain binding, never its credential.
    Keychain {
        /// The service and account identifying the binding.
        keychain: KeychainReference,
    },
}

/// A keychain lookup identity, without any keychain access capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, String>")]
pub struct KeychainReference {
    /// The nonblank service name.
    pub service: String,
    /// The nonblank account name.
    pub account: String,
}

impl TryFrom<BTreeMap<String, String>> for KeychainReference {
    type Error = &'static str;

    fn try_from(mut fields: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        let service = fields.remove("service").ok_or("missing service")?;
        let account = fields.remove("account").ok_or("missing account")?;
        if !fields.is_empty() {
            return Err("unknown keychain reference key");
        }
        Ok(Self {
            service: nonblank(service)?,
            account: nonblank(account)?,
        })
    }
}

/// Decode a nonblank identity, without resolving it.
fn deserialize_nonblank<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    nonblank(String::deserialize(deserializer)?).map_err(de::Error::custom)
}

/// Require a nonblank lookup identity without interpolating or resolving it.
fn nonblank(name: String) -> Result<String, &'static str> {
    if name.trim().is_empty() {
        return Err("secret reference identity must not be blank");
    }
    Ok(name)
}
