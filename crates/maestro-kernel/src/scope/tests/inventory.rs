//! There is no read function without a `ScopeSet`: every public method of
//! the kernel's database that opens a reader takes one, but for the readers
//! of unscoped bookkeeping the scope module's docs list with their reason.
//! The test reads the crate's own sources, as rustfmt lays them out.
//!
//! The scan sees a reader only by the `self.reader()` in its own body. A
//! method that reads inside its write, as `ack` does, through another method
//! or from the artifact store escapes it, and so does a reader outside the
//! database's `impl` blocks: those are kept by hand in `BY_HAND`, and the
//! scope module's docs must name each.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// The readers of unscoped bookkeeping that open a reader: each takes no
/// `ScopeSet`, and the scope module's docs give its reason.
const UNSCOPED: [&str; 6] = [
    "artifact",
    "check_artifacts",
    "cursor",
    "garbage",
    "quick_check",
    "visible",
];

/// The unscoped readers the scan cannot see, as the scope module's docs name
/// them: `ack` reads inside its write, `collect_garbage` through `garbage`,
/// `get` from the artifact store, and the other three are not the
/// database's.
const BY_HAND: [&str; 6] = [
    "Database::ack",
    "Database::collect_garbage",
    "Database::get",
    "artifact::Store::get",
    "ModelCard::load",
    "store::pending_migrations",
];

/// A public method of the database, as the crate's sources declare it.
#[derive(Debug)]
struct Method {
    /// Its name.
    name: String,
    /// Its declaration, from `pub fn` to the brace that opens its body.
    signature: String,
    /// Its body.
    body: String,
}

impl Method {
    /// Whether its body opens a reader, however rustfmt splits the call.
    fn opens_reader(&self) -> bool {
        let compact: String = self.body.split_whitespace().collect();
        compact.contains("self.reader()")
    }

    /// Whether it takes the caller's `ScopeSet`.
    fn takes_scopes(&self) -> bool {
        self.signature.contains("scopes: &ScopeSet")
    }
}

/// Every Rust file under `directory` but the tests, in path order.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for path in fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
        {
            if path.ends_with("tests") || path.ends_with("tests.rs") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The public methods of the `impl Database` blocks of `text`.
fn methods_of(text: &str) -> Vec<Method> {
    let mut methods = Vec::new();
    let mut lines = text.lines().map(str::trim_end);
    while lines.by_ref().any(|line| line == "impl Database {") {
        let block: Vec<&str> = lines.by_ref().take_while(|line| *line != "}").collect();
        let mut rest = block.into_iter();
        while let Some(first) = rest.find(|line| line.starts_with("    pub fn ")) {
            let mut signature = vec![first];
            if !first.ends_with('{') {
                signature.extend(rest.by_ref().take_while(|line| !line.ends_with('{')));
            }
            let body: Vec<&str> = rest.by_ref().take_while(|line| *line != "    }").collect();
            let name = first["    pub fn ".len()..]
                .split(['(', '<'])
                .next()
                .unwrap()
                .to_owned();
            methods.push(Method {
                name,
                signature: signature.join("\n"),
                body: body.join("\n"),
            });
        }
    }
    methods
}

#[test]
fn there_is_no_read_function_without_a_scope_set() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let methods: Vec<Method> = sources(&root)
        .iter()
        .flat_map(|path| methods_of(&fs::read_to_string(path).unwrap()))
        .collect();
    let readers: Vec<&Method> = methods
        .iter()
        .filter(|method| method.opens_reader())
        .collect();
    let scoped: BTreeSet<&str> = readers
        .iter()
        .filter(|method| method.takes_scopes())
        .map(|method| method.name.as_str())
        .collect();
    assert!(scoped.contains("events"), "{methods:#?}");
    let unscoped: BTreeSet<&str> = readers
        .iter()
        .filter(|method| !method.takes_scopes())
        .map(|method| method.name.as_str())
        .collect();
    assert_eq!(
        unscoped,
        BTreeSet::from(UNSCOPED),
        "a reader of scoped data takes `scopes: &ScopeSet`; a reader of \
         bookkeeping is listed here and in the scope module's docs"
    );
    let docs = fs::read_to_string(root.join("scope").join("mod.rs")).unwrap();
    let listed = UNSCOPED
        .map(|name| format!("Database::{name}"))
        .into_iter()
        .chain(BY_HAND.map(str::to_owned));
    for name in listed {
        assert!(docs.contains(&format!("[`{name}`]")), "{name}");
    }
}

#[test]
fn the_inventory_reads_each_public_method_of_the_database_whole() {
    let text = "impl Other {\n    pub fn skipped(&self) {\n        self.reader();\n    }\n}\n\
                impl Database {\n    fn private(&self) {}\n\n    /// Doc.\n    \
                pub fn short(&self) -> u8 {\n        self\n            .reader()\n    }\n\n    \
                pub fn long<T>(\n        &self,\n        scopes: &ScopeSet,\n    ) -> T\n    \
                where\n        T: Default,\n    {\n        T::default()\n    }\n}\n";
    let methods = methods_of(text);
    let names: Vec<&str> = methods.iter().map(|method| method.name.as_str()).collect();
    assert_eq!(names, ["short", "long"]);
    let [short, long] = methods.as_slice() else {
        panic!("{methods:#?}");
    };
    assert_eq!(short.signature, "    pub fn short(&self) -> u8 {");
    assert_eq!(short.body, "        self\n            .reader()");
    assert!(short.opens_reader());
    assert!(!short.takes_scopes());
    assert!(long.signature.ends_with("where\n        T: Default,"));
    assert_eq!(long.body, "        T::default()");
    assert!(!long.opens_reader());
    assert!(long.takes_scopes());
}
