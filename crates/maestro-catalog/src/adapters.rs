//! Shared reviewed adapter naming metadata. Products are typed values, never
//! functional path/ID names. Schema and checker consumers share these records.

/// One registered adapter's product names and exact naming exceptions.
#[derive(Debug)]
pub struct AdapterMetadata {
    /// Its typed product name and reviewed aliases; matching is token-based.
    pub product_names: &'static [&'static str],
    /// Its exact approved host directory, if any.
    pub host_directory: Option<&'static str>,
    /// Tool-required native filenames with registered relative placements.
    pub native_files: &'static [NativeFile],
}

/// One exact native filename; wildcards name inventories/resources, never files.
#[derive(Debug)]
pub struct NativeFile {
    /// The tool-required filename.
    pub filename: &'static str,
    /// Its area-local registered placement patterns.
    pub placements: &'static [&'static str],
}

/// Registered adapters. A new adapter contributes names to the forbidden union;
/// it does not gain an exception unless this reviewed metadata gives one.
pub const REGISTERED: &[AdapterMetadata] = &[
    AdapterMetadata {
        product_names: &[],
        host_directory: None,
        native_files: &[NativeFile {
            filename: "SKILL.md",
            placements: &["skills/*/SKILL.md"],
        }],
    },
    AdapterMetadata {
        product_names: &["ladybug"],
        host_directory: None,
        native_files: &[],
    },
    AdapterMetadata {
        product_names: &["qdrant"],
        host_directory: None,
        native_files: &[],
    },
    AdapterMetadata {
        product_names: &["gerrit"],
        host_directory: None,
        native_files: &[],
    },
    AdapterMetadata {
        product_names: &["pi"],
        host_directory: Some("core/hosts/pi"),
        native_files: &[NativeFile {
            filename: "AGENTS.md",
            placements: &["bootstrap/*/files/AGENTS.md"],
        }],
    },
    AdapterMetadata {
        product_names: &["claude-code"],
        host_directory: Some("core/hosts/claude-code"),
        native_files: &[NativeFile {
            filename: "CLAUDE.md",
            placements: &["bootstrap/*/files/CLAUDE.md"],
        }],
    },
    AdapterMetadata {
        product_names: &["copilot"],
        host_directory: Some("core/hosts/copilot"),
        native_files: &[NativeFile {
            filename: "copilot-instructions.md",
            placements: &["bootstrap/*/files/.github/copilot-instructions.md"],
        }],
    },
];
