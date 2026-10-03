//! Frozen Copilot v1 authoring projection, not host discovery evidence.
use super::{preferences::ClientPreferencesDelivery, shared_json};
use crate::{
    bootstrap::{PresetPort, SourceFile},
    files::{self, FileInput, FilePlan, apply as apply_files, digest},
    frontmatter::split_frontmatter,
    limits::Limits,
    policy::workspace::{Access, CheckedTrust},
    source::{Value as SourceValue, parse::yaml_table},
};
use maestro_filesystem::Directory;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    ffi::OsStr,
    io::{self, Read as _},
    iter,
    path::{Component, Path, PathBuf, is_separator},
    str,
};

/// C01's one canonical v1 declaration, shared by admission and rendering.
const TOOLS: [&str; 2] = ["maestro/knowledge_search", "view"];

/// Checked, inert source closure; C14 can supply admitted installed sources at this boundary.
#[derive(Debug)]
pub struct SourceSnapshot {
    /// Canonical native body, extracted from the already checked source bytes.
    pub(super) body: String,
    /// Exact captured closure and its read roots, rechecked before effects.
    pub(super) sources: Vec<CapturedSource>,
}

/// One source's immutable provenance; absolute read roots never enter projected files.
#[derive(Debug, Serialize)]
pub(super) struct CapturedSource {
    /// Catalog-relative checked source name, including sidecars.
    path: String,
    /// Qualified owning resource identity.
    id: String,
    /// Declared version, if this resource kind has one.
    revision: Option<String>,
    /// Digest of the single validated snapshot.
    digest: String,
    /// Root is read authority only, not projection write authority.
    #[serde(skip)]
    root: Option<PathBuf>,
}

impl SourceSnapshot {
    /// Capture the checked closure without rereading sources; qualify captured native fields.
    ///
    /// # Errors
    /// Refuses missing/unknown presets, conflicting sources and a missing canonical Maestro agent.
    pub fn resolve(port: &dyn PresetPort, presets: &[String]) -> Result<Self, String> {
        if presets.is_empty() {
            return Err("select at least one preset".to_owned());
        }
        let mut sources: BTreeMap<String, (SourceFile, Option<PathBuf>)> = BTreeMap::new();
        for preset in port.resolve(presets)? {
            for (path, source) in preset.source_files {
                insert_source(&mut sources, path, source, preset.source_root.clone())?;
            }
        }
        let agents: Vec<_> = sources
            .iter()
            .filter(|(path, (source, _))| {
                source.id == "agent:core/maestro" && path.ends_with(".agent.md")
            })
            .collect();
        let [(path, (agent, _))] = agents.as_slice() else {
            return Err(
                "source closure requires exactly one agent:core/maestro profile".to_owned(),
            );
        };
        let text = str::from_utf8(&agent.bytes).map_err(|error| format!("{path}: {error}"))?;
        let body = qualified_body(path, text)?;
        Ok(Self {
            body: body.to_owned(),
            sources: sources
                .into_iter()
                .map(|(path, (source, root))| CapturedSource {
                    path,
                    id: source.id,
                    revision: source.revision,
                    digest: digest(&source.bytes),
                    root,
                })
                .collect(),
        })
    }

    /// Recheck the exact closure before writing, never silently rebuild a changed preview.
    fn check(&self) -> io::Result<()> {
        for source in &self.sources {
            let Some(root) = &source.root else {
                continue;
            };
            let path = Path::new(&source.path);
            let parent = path.parent().unwrap_or(Path::new(""));
            let leaf = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| io::Error::other("invalid captured source path"))?;
            let bytes = Directory::open(root, parent, false)?
                .read_regular_bounded(leaf, Limits::PRODUCTION.source_file_bytes)?;
            if digest(&bytes) != source.digest {
                return Err(io::Error::other(format!(
                    "source {} changed since preview; run preview again",
                    source.path
                )));
            }
        }
        Ok(())
    }
}

/// Admit only C01's native mapping; unsupported declarations never disappear in rendering.
fn qualified_body<'a>(path: &str, text: &'a str) -> Result<&'a str, String> {
    let (front, body) =
        split_frontmatter(text).ok_or_else(|| format!("{path}: missing qualified frontmatter"))?;
    let fields = yaml_table(front, &Limits::PRODUCTION)
        .map_err(|(field, message)| format!("{path}: {field}: {message}"))?;
    for key in ["model", "mcp-servers"] {
        if fields.contains_key(key) {
            return Err(format!(
                "{path}: {key}: model mapping not qualified for Copilot (C01); \
                 remove the unqualified declaration and preview again"
            ));
        }
    }
    if let Some(value) = fields.get("tools") {
        let tools = value
            .texts()
            .ok_or_else(|| format!("{path}: tools must be a text list"))?;
        for tool in tools {
            if !TOOLS.contains(&tool) {
                return Err(format!(
                    "{path}: tool `{tool}` not qualified for Copilot; \
                     remove the unqualified tool declaration and preview again"
                ));
            }
        }
    }
    Ok(body)
}

/// Insert an already validated source once, refusing conflicting snapshots.
fn insert_source(
    sources: &mut BTreeMap<String, (SourceFile, Option<PathBuf>)>,
    path: String,
    source: SourceFile,
    root: Option<PathBuf>,
) -> Result<(), String> {
    if let Some((existing, _)) = sources.get(&path) {
        if existing != &source {
            return Err(format!("conflicting source: {path}"));
        }
        return Ok(());
    }
    sources.insert(path, (source, root));
    Ok(())
}

/// The native delivery adapter for the frozen C01 Copilot v1 shape.
#[derive(Debug)]
pub struct Copilot {
    /// Explicit read-only user profile directory for C01 declared-name shadow checks.
    user_agents: Option<PathBuf>,
}

impl ClientPreferencesDelivery for Copilot {
    fn initialization_instructions(&self, session_preferences: &str) -> String {
        format!(
            "{session_preferences}\nUse Maestro MCP knowledge tools for source-backed evidence. \
             Restart the host to discover projected files; in-session reload is unqualified.\n"
        )
    }
}

/// Native projection boundary; adding a host changes only the adapter registry below.
pub trait NativeProjection: ClientPreferencesDelivery {
    /// Preview a checked source snapshot without effects or host-obedience claims.
    /// The authorized target root and supplied read root must be canonical.
    ///
    /// # Errors
    /// Refuses collisions, drift, malformed host data and read-policy failures.
    fn preview<'a>(
        &self,
        root: &Path,
        source: &'a SourceSnapshot,
        remove: bool,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<ProjectionPreview<'a>>;
}

impl NativeProjection for Copilot {
    fn preview<'a>(
        &self,
        root: &Path,
        source: &'a SourceSnapshot,
        remove: bool,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<ProjectionPreview<'a>> {
        self.plan(root, source, remove, trust)
    }
}

/// Resolve one exact host through the native adapter registry, never a caller-side host branch.
///
/// # Errors
/// Refuses unsupported or ambiguous host names without fallback.
pub fn adapter(host: &str) -> Result<Box<dyn NativeProjection>, String> {
    match host {
        "copilot" => {
            let home = env::var_os("COPILOT_HOME")
                .map(PathBuf::from)
                .or_else(|| env::home_dir().map(|home| home.join(".copilot")));
            Ok(Box::new(
                Copilot::from_home(home).map_err(|error| error.to_string())?,
            ))
        }
        _ => Err(format!(
            "unsupported or ambiguous host {host:?}; use --host copilot"
        )),
    }
}

/// One immutable projection preview; private authority snapshots never enter its document.
#[derive(Debug, Serialize)]
pub struct ProjectionPreview<'a> {
    /// Honest projection format, not installation or live-host authority.
    pub(super) schema: &'static str,
    /// A native registration is present and owned; not proof of host discovery.
    pub registered: bool,
    /// No observation is fabricated by this fixture-backed adapter.
    pub observed: bool,
    /// Separate drift state, never treated as a successful observation.
    pub stale: Vec<String>,
    /// Separate failed state, never treated as a registration.
    pub failed: Vec<String>,
    /// C01 qualification limits and unresolved executable/model availability.
    pub diagnosis: &'static str,
    /// C04 immutable whole-file plan for inserted private native files.
    pub(super) files: FilePlan,
    /// Entry-owned shared JSON update; preserves all other values.
    pub(super) shared: FilePlan,
    /// Exact checked source closure behind the previewed bytes.
    #[serde(skip)]
    pub(super) snapshot: &'a SourceSnapshot,
    /// Explicit removal, never permission to remove a user entry.
    pub(super) remove: bool,
    /// Exact preview target; even another approved folder cannot reuse the proposal.
    pub(super) root: PathBuf,
    /// Adapter-owned read validation, never selected by file content.
    #[serde(skip)]
    pub(super) check_discovery: ShadowCheck,
    /// Read-only native discovery input, rechecked before any effects.
    #[serde(skip)]
    pub(super) user_agents: Option<PathBuf>,
}

/// Trusted native discovery callback, independent of the controlled writer.
pub(super) type ShadowCheck = fn(&Path, Option<&Path>, bool, &CheckedTrust<'_>) -> io::Result<()>;

/// Immutable entry ownership and source provenance, committed through C04 with the native files.
#[derive(Serialize)]
struct Receipt<'a> {
    /// Versioned receipt shape.
    schema: &'static str,
    /// Qualified native host shape, not an observation.
    host: &'static str,
    /// Only the named inserted entry is owned, not the shared file.
    entries: BTreeMap<&'static str, Value>,
    /// Exact input closure, including metadata sidecars and digests.
    sources: &'a [CapturedSource],
}

impl Copilot {
    /// Bind read-only discovery, retaining absence as a path to recheck before applying.
    pub(super) fn from_home(home: Option<PathBuf>) -> io::Result<Self> {
        let user_agents = home
            .map(|home| {
                discovery_home(&home)
                    .map(|home| home.join("agents"))
                    .map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!("cannot resolve Copilot home {}: {error}", home.display()),
                        )
                    })
            })
            .transpose()?;
        Ok(Self::new(user_agents))
    }

    /// Select only read-only discovery inputs; target write authority is separate.
    /// Supplied read roots must be canonical; preview refuses aliases without effects.
    #[must_use]
    pub fn new(user_agents: Option<PathBuf>) -> Self {
        Self { user_agents }
    }

    /// Preview native files and one shared registration; no file or trust record is created.
    ///
    /// # Errors
    /// Refuses source/owned drift, unowned collisions, malformed JSON and policy/read failures.
    fn plan<'a>(
        &self,
        root: &Path,
        snapshot: &'a SourceSnapshot,
        remove: bool,
        trust: &CheckedTrust<'_>,
    ) -> io::Result<ProjectionPreview<'a>> {
        for named in iter::once(root).chain(self.user_agents.as_deref()) {
            if named != discovery_home(named)? {
                return Err(io::Error::other(format!(
                    "root is not canonical: {}",
                    named.display()
                )));
            }
        }
        let registration = json!({
            "type":"stdio", "command":"maestro",
            "args":["mcp", "--workspace", "."], "tools":["*"]
        });
        let rules = self.native_instructions();
        let receipt = Receipt {
            schema: "maestro-copilot-ownership/1",
            host: "copilot-v1",
            entries: BTreeMap::from([("/mcpServers/maestro", registration.clone())]),
            sources: &snapshot.sources,
        };
        let tools = TOOLS.join("\", \"");
        let agent = format!(
            "---\nname: maestro\ndescription: Maestro source-backed knowledge agent.\n\
             tools: [\"{tools}\"]\n---\n{}\n{rules}",
            snapshot.body
        );
        let files = FilePlan::preview(
            root,
            [
                FileInput::new(".github/agents/maestro.agent.md", agent.into_bytes()),
                FileInput::new(".github/copilot-instructions.md", rules.into_bytes()),
                FileInput::new(
                    ".maestro/copilot-ownership.json",
                    serde_json::to_vec_pretty(&receipt).map_err(io::Error::other)?,
                ),
            ],
            trust,
        )?;
        let owned = files.is_applied();
        check_shadows(root, self.user_agents.as_deref(), owned, trust)?;
        let shared = FilePlan::preview_edit(
            root,
            ".mcp.json",
            |bytes| shared_json::edit(bytes, &registration, owned, remove),
            trust,
        )?;
        let registered = owned;
        Ok(ProjectionPreview {
            schema: "maestro-copilot-preview/1",
            registered,
            observed: false,
            stale: Vec::new(),
            failed: Vec::new(),
            diagnosis: "authoring convenience; not a verified install; \
                live discovery: C08/C28 gate; reload inside a running session: not run; \
                Maestro must be on PATH; tool/model availability is not observed; \
                host trust and enterprise policy remain independent; \
                replacement is not compare-and-swap; \
                avoid concurrent edits of shared configuration",
            files,
            shared,
            snapshot,
            remove,
            user_agents: self.user_agents.clone(),
            root: root.to_path_buf(),
            check_discovery: check_shadows,
        })
    }
}

impl ProjectionPreview<'_> {
    /// Apply exactly the previewed plans through C04, never silently replan or grant trust.
    ///
    /// # Errors
    /// Refuses changed sources/targets, trust/secret denials and C04 write or removal failures.
    pub fn apply(&self, root: &Path, trust: &CheckedTrust<'_>) -> io::Result<()> {
        if root != self.root {
            return Err(io::Error::other(
                "projection target changed; preview the requested target again",
            ));
        }
        self.snapshot.check()?;
        (self.check_discovery)(
            root,
            self.user_agents.as_deref(),
            self.files.is_applied(),
            trust,
        )?;
        self.shared.check_replacement(root, trust)?;
        if self.remove {
            if self.files.is_applied() {
                apply_files(root, &self.shared, trust)?;
                files::remove(root, self.files.id(), trust)?;
            }
            return Ok(());
        }
        apply_files(root, &self.files, trust)?;
        if let Err(error) = apply_files(root, &self.shared, trust) {
            if !self.files.is_applied() {
                files::remove(root, self.files.id(), trust)?;
            }
            return Err(error);
        }
        Ok(())
    }
}

/// Inspect declared names, never trust a filename stem or the host's silent precedence.
fn check_shadows(
    root: &Path,
    user: Option<&Path>,
    owned: bool,
    trust: &CheckedTrust<'_>,
) -> io::Result<()> {
    check_names(root, Path::new(".github/agents"), owned, trust)?;
    if let Some(user) = user {
        check_names(user, Path::new(""), false, trust)?;
    }
    Ok(())
}

/// A missing native folder has no profiles; malformed or linked profiles refuse.
fn check_names(root: &Path, below: &Path, owned: bool, trust: &CheckedTrust<'_>) -> io::Result<()> {
    let directory = match Directory::open_canonical(&root.join(below)) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in directory.list_bounded(Limits::PRODUCTION.catalog_resources)? {
        let Some(name) = profile_filename(&entry.name)? else {
            continue;
        };
        if owned && name == "maestro.agent.md" {
            continue;
        }
        let relative = below.join(name);
        let mut file = trust
            .authorize(root, &relative, Access::Read)?
            .open_read()?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(Limits::PRODUCTION.source_file_bytes + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > Limits::PRODUCTION.source_file_bytes {
            return Err(io::Error::other(
                "native profile exceeds source file byte limit",
            ));
        }
        let text = str::from_utf8(&bytes).map_err(io::Error::other)?;
        let (front, _) = split_frontmatter(text)
            .ok_or_else(|| io::Error::other("native profile has no frontmatter"))?;
        let fields = yaml_table(front, &Limits::PRODUCTION)
            .map_err(|(key, reason)| io::Error::other(format!("native profile {key}: {reason}")))?;
        let Some(SourceValue::Text(declared)) = fields.get("name") else {
            return Err(io::Error::other("native profile name must be a string"));
        };
        if declared == "maestro" {
            return Err(io::Error::other(format!(
                "{}: declared name maestro shadows the projection; \
                 move or rename the user profile's declared name",
                root.join(&relative).display()
            )));
        }
    }
    Ok(())
}

/// Bind the trusted discovery root once, retaining missing normal names for staged rechecks.
fn discovery_home(home: &Path) -> io::Result<PathBuf> {
    for ancestor in home.ancestors() {
        // Windows normalizes missing/name/.. before lookup; it cannot anchor the missing tail.
        if matches!(
            ancestor.components().next_back(),
            Some(Component::ParentDir)
        ) {
            continue;
        }
        let named = if ancestor.as_os_str().is_empty() {
            Path::new(".")
        } else {
            ancestor
        };
        match named.canonicalize() {
            Ok(resolved) => {
                let tail = home.strip_prefix(ancestor).map_err(io::Error::other)?;
                // Components normalize dots; verbatim Windows dots are Normal.
                // Check both literal spellings before appending the missing tail.
                let dot = tail
                    .as_os_str()
                    .as_encoded_bytes()
                    .split(|byte| is_separator(char::from(*byte)))
                    .any(|part| matches!(part, b"." | b".."));
                if dot
                    || !tail
                        .components()
                        .all(|part| matches!(part, Component::Normal(_)))
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Copilot home's missing tail must contain normal names only",
                    ));
                }
                return Ok(resolved.join(tail));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("Copilot home has no resolvable ancestor"))
}

/// Classify native directory names without asking the filesystem to create invalid names.
pub(super) fn profile_filename(name: &OsStr) -> io::Result<Option<&str>> {
    let name = name
        .to_str()
        .ok_or_else(|| io::Error::other("native profile name is not UTF-8"))?;
    Ok(name.ends_with(".agent.md").then_some(name))
}
