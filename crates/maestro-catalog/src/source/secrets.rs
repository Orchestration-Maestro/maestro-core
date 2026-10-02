//! Typed secret bindings for registered settings, backend and extension fields.
//! These are references only: validation and serialization never read a binding.

use serde::{Deserialize, Deserializer, Serialize, de};

/// Exactly one environment-variable or keychain-service/account reference.
/// Unknown keys, literal values and mixed forms are refused by the strict decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SecretReference {
    /// A runtime environment-variable binding, never its value.
    Environment {
        /// The nonblank variable name.
        #[serde(deserialize_with = "nonblank")]
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
#[serde(deny_unknown_fields)]
pub struct KeychainReference {
    /// The nonblank service name.
    #[serde(deserialize_with = "nonblank")]
    pub service: String,
    /// The nonblank account name.
    #[serde(deserialize_with = "nonblank")]
    pub account: String,
}

/// Require a nonblank lookup identity without interpolating or resolving it.
fn nonblank<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let name = String::deserialize(deserializer)?;
    if name.trim().is_empty() {
        return Err(de::Error::custom(
            "secret reference identity must not be blank",
        ));
    }
    Ok(name)
}
