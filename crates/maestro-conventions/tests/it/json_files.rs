//! Integration tests for path-selected JSON decoding.
use std::{error::Error, fs};

use super::private_content::{
    CANARY, Scratch, TestBank, commit, git, init_repo, privacy_binary, require_success, text,
};

#[test]
fn git_scan_selects_json_decoding_by_path() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[CANARY], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("baseline.md"), "clean synthetic baseline")?;
    let baseline = commit(&repo, "synthetic baseline")?;
    require_success(&git(
        &repo,
        &["update-ref", "refs/remotes/origin/main", &baseline],
    )?);
    fs::write(repo.join("Cargo.toml"), "[workspace]\nresolver = \"3\"\n")?;
    fs::write(
        repo.join("README.md"),
        "[documentation](https://example.invalid)\n",
    )?;
    let encoded = CANARY.replace(' ', r"\u0020");
    fs::write(
        repo.join("records.jsonl"),
        format!("{{\"question\":\"{encoded}\"}}\n{{\"question\":\"clean synthetic\"}}\n"),
    )?;
    let candidate = commit(&repo, "synthetic text files")?;

    let result =
        bank.scan_with_remote(binary, &repo, &format!("{candidate}:{baseline}"), "origin")?;
    assert_eq!(result.status.code(), Some(1), "output: {}", text(&result));
    assert!(text(&result).contains("hits=1"));
    assert!(!text(&result).contains(CANARY));
    Ok(())
}

#[test]
fn git_scan_refuses_malformed_json_files() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("baseline.md"), "clean synthetic baseline")?;
    let baseline = commit(&repo, "synthetic baseline")?;
    require_success(&git(
        &repo,
        &["update-ref", "refs/remotes/origin/main", &baseline],
    )?);
    fs::write(repo.join("record.json"), r#"{"value":"synthetic marker""#)?;
    let candidate = commit(&repo, "synthetic malformed JSON")?;

    let refused =
        bank.scan_with_remote(binary, &repo, &format!("{candidate}:{baseline}"), "origin")?;
    assert_eq!(refused.status.code(), Some(2));
    assert!(!text(&refused).contains("synthetic marker"));
    Ok(())
}
