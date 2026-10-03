//! The repository's policies, checked on every pull request by `cargo test`.
#![cfg(test)]

mod catalog_traceability;
mod s1_traceability;
mod unsafe_policy;

use unsafe_policy::{cfg_test_module, privilege_support_is_gated, unsafe_file_violation};

use maestro_conventions::{
    broken_links, counted_lines, names_a_personal_directory, repository_files, root,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// Every repository file that reads as UTF-8 text, with its contents.
fn text_files() -> Vec<(PathBuf, String)> {
    let root = root();
    repository_files(&root)
        .unwrap()
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(root.join(&file)).ok()?;
            Some((file, text))
        })
        .collect()
}

#[test]
fn every_conventions_test_file_is_registered() {
    let tests = root().join("crates/maestro-conventions/tests");
    for file in repository_files(&tests).unwrap() {
        if file.extension().is_none_or(|extension| extension != "rs")
            || file == Path::new("policies.rs")
        {
            continue;
        }
        let module_path = if file.file_name().unwrap() == "mod.rs" {
            file.parent().unwrap().to_path_buf()
        } else {
            file.with_extension("")
        };
        let parent = module_path.parent().unwrap();
        // Root modules belong to the one test binary; nested modules to their parent.
        let declaring = if parent.as_os_str().is_empty() {
            PathBuf::from("policies.rs")
        } else {
            parent.join("mod.rs")
        };
        let source = fs::read_to_string(tests.join(&declaring)).unwrap();
        let module = module_path.file_name().unwrap().to_str().unwrap();
        assert!(
            source
                .lines()
                .any(|line| line.trim() == format!("mod {module};")),
            "{} must be declared as a module in {}",
            file.display(),
            declaring.display()
        );
    }
}

#[test]
fn rust_files_stay_within_500_counted_lines() {
    let oversized: Vec<String> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .filter_map(|(file, text)| {
            let lines = counted_lines(&text);
            (lines > 500).then(|| format!("{}: {lines}", file.display()))
        })
        .collect();
    assert!(oversized.is_empty(), "split these files: {oversized:?}");
}

#[test]
fn no_personal_path_in_repository_text() {
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(_, text)| names_a_personal_directory(text))
        .map(|(file, _)| file)
        .collect();
    assert!(offenders.is_empty(), "personal paths in {offenders:?}");
}

#[test]
fn no_private_registry_or_quality_service_setting() {
    // Split so that this file does not name them. Documents may say what was
    // dropped; configuration and code may not carry it.
    let settings = [
        ["arti", "factory"].concat(),
        ["jf", "rog"].concat(),
        ["son", "ar"].concat(),
    ];
    let offenders: Vec<PathBuf> = text_files()
        .into_iter()
        .filter(|(file, _)| file.extension().is_none_or(|extension| extension != "md"))
        .filter(|(_, text)| {
            let text = text.to_lowercase();
            settings
                .iter()
                .any(|setting| text.contains(setting.as_str()))
        })
        .map(|(file, _)| file)
        .collect();
    assert!(
        offenders.is_empty(),
        "registry or quality-service settings in {offenders:?}"
    );
}

#[test]
fn every_member_inherits_the_workspace_lints() {
    for member in fs::read_dir(root().join("crates")).unwrap() {
        let manifest = member.unwrap().path().join("Cargo.toml");
        let text = fs::read_to_string(&manifest).unwrap();
        let name = manifest.display();
        assert!(
            text.contains("[lints]\nworkspace = true"),
            "{name} must inherit the lints"
        );
        assert!(
            !text.contains("[lints."),
            "{name} must not define its own lints"
        );
    }
}

#[test]
fn positional_mutant_exclusions_match_their_source_operator() {
    let root = root();
    let path = root.join(".cargo/mutants.toml");
    let config: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let entries = config["exclude_re"].as_array().unwrap();
    let mut failures = Vec::new();
    let mut positional_entries = 0;
    for entry in entries.iter().filter_map(toml::Value::as_str) {
        let Some((position, replacement)) = entry.split_once(": replace ") else {
            continue;
        };
        if !replacement.contains(" in ") {
            continue;
        }
        let position = position.strip_prefix('^').unwrap_or(position);
        let Some((position, column)) = position.rsplit_once(':') else {
            failures.push(format!("{entry}: missing column"));
            continue;
        };
        let Some((file, line)) = position.rsplit_once(':') else {
            failures.push(format!("{entry}: missing line"));
            continue;
        };
        let file = file.replace("\\.", ".");
        let operator = replacement
            .split_once(" with ")
            .map(|(operator, _)| operator.replace('\\', ""));
        let Some(operator) = operator else {
            failures.push(format!("{entry}: missing replaced operator"));
            continue;
        };
        let source = fs::read_to_string(root.join(&file));
        let (Ok(line), Ok(column)) = (line.parse::<usize>(), column.parse::<usize>()) else {
            failures.push(format!("{entry}: missing numeric line or column"));
            continue;
        };
        positional_entries += 1;
        let found = source.ok().and_then(|source| {
            source
                .lines()
                .nth(line.checked_sub(1)?)?
                .get(column.checked_sub(1)?..)
                .map(str::to_owned)
        });
        if !found.as_deref().is_some_and(|source| {
            source.starts_with(&operator)
                && source
                    .get(operator.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|next| !"|&<>=!^+-*/%".contains(next))
        }) {
            failures.push(format!(
                "{entry}: position holds {found:?}, expected operator {operator:?}"
            ));
        }
    }
    assert!(
        positional_entries > 0,
        "no positional mutant exclusions found"
    );
    assert!(
        failures.is_empty(),
        "dead mutant exclusions:\n{}",
        failures.join("\n")
    );
}

#[test]
fn function_body_mutant_exclusions_name_the_body_and_function() {
    let root = root();
    let path = root.join(".cargo/mutants.toml");
    let config: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let entries = config["exclude_re"].as_array().unwrap();
    let mut failures = Vec::new();
    let mut body_entries = 0;
    let mut match_arm_entries = 0;
    for entry in entries.iter().filter_map(toml::Value::as_str) {
        let Some((position, action)) = entry.trim_start_matches('^').split_once(": ") else {
            continue;
        };
        let function = if let Some(replacement) = action.strip_prefix("replace ") {
            replacement.split_once(" -> ").map(|(function, _)| function)
        } else if let Some(deletion) = action.strip_prefix("delete match arm ") {
            deletion.split_once(" in ").map(|(_, function)| function)
        } else {
            continue;
        };
        let Some(function) = function else {
            continue;
        };
        let function = function
            .trim_end_matches('$')
            .rsplit("::")
            .next()
            .unwrap_or(function);
        if action.starts_with("delete match arm ") {
            match_arm_entries += 1;
        } else {
            body_entries += 1;
        }
        let Some((position, column)) = position.rsplit_once(':') else {
            failures.push(format!("{entry}: missing column"));
            continue;
        };
        let Some((file, line)) = position.rsplit_once(':') else {
            failures.push(format!("{entry}: missing line"));
            continue;
        };
        let file = file.replace("\\.", ".");
        let (Ok(line), Ok(column)) = (line.parse::<usize>(), column.parse::<usize>()) else {
            failures.push(format!("{entry}: missing numeric line or column"));
            continue;
        };
        let source = fs::read_to_string(root.join(file));
        let location = line.checked_sub(1);
        let column = column.checked_sub(1);
        let body = source.ok().and_then(|source| {
            let lines: Vec<_> = source.lines().collect();
            let location = location?;
            let column = column?;
            let body_text = lines.get(location)?.get(column..)?;
            let declaration = lines
                .iter()
                .enumerate()
                .take(location + 1)
                .rev()
                .find(|(_, candidate)| candidate.contains(&format!("fn {function}")))
                .map(|(index, candidate)| (index, *candidate));
            let function_declaration = declaration.is_some_and(|(index, candidate)| {
                (index < location
                    && lines[index..=location]
                        .iter()
                        .any(|line| line.contains('{')))
                    || (index == location
                        && candidate.find('{').is_some_and(|opening| opening < column))
            });
            Some((!body_text.trim().is_empty(), function_declaration))
        });
        if !body.is_some_and(|(nonblank, declaration)| nonblank && declaration) {
            failures.push(format!(
                "{entry}: position must be nonblank inside fn {function}; found {body:?}"
            ));
        }
    }
    assert!(body_entries > 0, "no function-body mutant exclusions found");
    assert!(
        match_arm_entries > 0,
        "no deleted match-arm exclusions found"
    );
    assert!(
        failures.is_empty(),
        "invalid function-body mutant exclusions:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_relative_link_and_anchor_resolves() {
    let root = root();
    let broken: Vec<String> = repository_files(&root)
        .unwrap()
        .into_iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "md"))
        // Markdown under examples/ is test input, kept byte for byte.
        .filter(|file| !file.components().any(|part| part.as_os_str() == "examples"))
        .flat_map(|file| broken_links(&root, &file).unwrap())
        .collect();
    assert!(broken.is_empty(), "broken links: {broken:?}");
}

/// Serialized opaque fields whose serde attributes omit the canonical adapter.
fn noncanonical_fields(source: &str) -> Vec<&str> {
    let mut attributes = String::new();
    let mut serialized = false;
    let mut offenders = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("#[derive(") {
            serialized = line.contains("Serialize");
        }
        if line.starts_with("///") || line.is_empty() {
            continue;
        }
        if line.starts_with("#[") || line.starts_with("serialize_with") {
            attributes.push_str(line);
            continue;
        }
        if serialized
            && line.contains(':')
            && line.ends_with(',')
            && line.contains("Value")
            && !attributes.contains("maestro_kernel::json::serialize_canonical")
        {
            offenders.push(line);
        }
        attributes.clear();
    }
    offenders
}

#[test]
fn opaque_canonical_fields_require_canonical_serialization() {
    for module in ["model", "document", "content", "dedup"] {
        let path = root().join(format!("crates/maestro-canonicalization/src/{module}.rs"));
        let source = fs::read_to_string(path).unwrap();
        assert!(
            noncanonical_fields(&source).is_empty(),
            "{module}: opaque fields lack canonical serialization: {:?}",
            noncanonical_fields(&source)
        );
    }
}

/// Unsafe operations stay in held-handle production code or exact native PTY test boundaries.
#[test]
fn unsafe_is_confined_to_windows_handle_security() {
    let files = text_files();
    let declarations = files
        .iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        privilege_support_is_gated(&declarations),
        "native privilege support must remain test-only and Windows-only"
    );
    let init = fs::read_to_string(root().join("crates/maestro/src/cli/init/mod.rs")).unwrap();
    assert!(
        cfg_test_module(&init, "tests"),
        "native PTY support must remain test-only"
    );
    let offenders: Vec<_> = files
        .into_iter()
        .filter(|(file, _)| file.extension().is_some_and(|extension| extension == "rs"))
        .filter(|(file, text)| unsafe_file_violation(file, text))
        .map(|(file, _)| file)
        .collect();
    assert!(
        offenders.is_empty(),
        "unsafe boundary violations: {offenders:?}"
    );
}

#[test]
fn quality_dependency_exception_keys_are_unique() {
    let source = fs::read_to_string(root().join("maestro-quality.toml")).unwrap();
    let quality: toml::Value = toml::from_str(&source).unwrap();
    let mut keys = BTreeSet::new();
    for entry in quality["exception"].as_array().unwrap() {
        let rule = entry["rule"].as_str().unwrap();
        if rule == "DEP-001" {
            let path = entry["path"].as_str().unwrap();
            assert!(keys.insert((rule, path)), "duplicate {rule}: {path}");
        }
    }
}
