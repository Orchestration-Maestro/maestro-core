//! The closed graph vocabulary, shared by claims and schema migration.

/// The closed list of entity kinds (architecture 02 §8.2, FR-S2-024): every
/// one may be a claim's subject or object, and no other exists without an
/// ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// `Component`.
    Component,
    /// `Command`.
    Command,
    /// `Parameter`.
    Parameter,
    /// `ConfigFile`.
    ConfigFile,
    /// `ErrorCode`.
    ErrorCode,
    /// `Message`.
    Message,
    /// `Version`.
    Version,
    /// `Platform`.
    Platform,
    /// `Port`.
    Port,
    /// `Feature`.
    Feature,
    /// `Concept`.
    Concept,
    /// `API`.
    Api,
}

/// The closed list of relation types (architecture 02 §8.2, FR-S2-024).
/// Every one but `ALIAS_OF` is a claim predicate: an alias is a reviewed
/// identity record, never an extracted claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Predicate {
    /// The subject requires the object.
    Requires,
    /// The subject configures the object.
    Configures,
    /// The subject is part of the object.
    PartOf,
    /// The subject depends on the object.
    DependsOn,
    /// The subject replaces the object.
    Replaces,
    /// The subject is deprecated in the object, a version.
    DeprecatedIn,
    /// The subject is introduced in the object, a version.
    IntroducedIn,
    /// The subject applies to the object.
    AppliesTo,
    /// The subject causes the object.
    Causes,
    /// The subject resolves the object.
    Resolves,
    /// The subject's default is the object, a literal.
    DefaultsTo,
    /// The subject is another name of the object: never a claim.
    AliasOf,
}

impl EntityKind {
    /// Every kind, in the order architecture 02 §8.2 lists them.
    pub const ALL: [Self; 12] = [
        Self::Component,
        Self::Command,
        Self::Parameter,
        Self::ConfigFile,
        Self::ErrorCode,
        Self::Message,
        Self::Version,
        Self::Platform,
        Self::Port,
        Self::Feature,
        Self::Concept,
        Self::Api,
    ];

    /// Its name, as architecture 02 §8.2 and the `subject_kind` and
    /// `object_kind` columns spell it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Component => "Component",
            Self::Command => "Command",
            Self::Parameter => "Parameter",
            Self::ConfigFile => "ConfigFile",
            Self::ErrorCode => "ErrorCode",
            Self::Message => "Message",
            Self::Version => "Version",
            Self::Platform => "Platform",
            Self::Port => "Port",
            Self::Feature => "Feature",
            Self::Concept => "Concept",
            Self::Api => "API",
        }
    }

    /// The kind spelled exactly `name`; none outside the closed list.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

impl Predicate {
    /// Every relation type, in the order architecture 02 §8.2 lists them.
    pub const ALL: [Self; 12] = [
        Self::Requires,
        Self::Configures,
        Self::PartOf,
        Self::DependsOn,
        Self::Replaces,
        Self::DeprecatedIn,
        Self::IntroducedIn,
        Self::AppliesTo,
        Self::Causes,
        Self::Resolves,
        Self::DefaultsTo,
        Self::AliasOf,
    ];

    /// Its name, as architecture 02 §8.2 and the `predicate` column spell
    /// it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requires => "REQUIRES",
            Self::Configures => "CONFIGURES",
            Self::PartOf => "PART_OF",
            Self::DependsOn => "DEPENDS_ON",
            Self::Replaces => "REPLACES",
            Self::DeprecatedIn => "DEPRECATED_IN",
            Self::IntroducedIn => "INTRODUCED_IN",
            Self::AppliesTo => "APPLIES_TO",
            Self::Causes => "CAUSES",
            Self::Resolves => "RESOLVES",
            Self::DefaultsTo => "DEFAULTS_TO",
            Self::AliasOf => "ALIAS_OF",
        }
    }

    /// The relation type spelled exactly `name`; none outside the closed
    /// list.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|predicate| predicate.as_str() == name)
    }

    /// Whether a claim may state it: every relation type but `ALIAS_OF`.
    #[must_use]
    pub fn is_claimable(self) -> bool {
        self != Self::AliasOf
    }

    /// Whether its object is a literal, as `DEFAULTS_TO`'s alone is, rather
    /// than an entity.
    #[must_use]
    pub fn takes_literal(self) -> bool {
        self == Self::DefaultsTo
    }
}
