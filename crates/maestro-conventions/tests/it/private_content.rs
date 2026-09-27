//! The privacy CLI refuses invented private content without disclosing it.
use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::{self, Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const CANARY: &str = "quartz zephyr lantern cobalt apricot meadow orbit ember nebula";
const REMOTE_CANARY: &str = "violet geode compass willow tinsel delta crater mango";

pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = env::temp_dir().join(format!("maestro-privacy-{}-{nonce}", process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub(super) fn path(&self, child: &str) -> PathBuf {
        self.0.join(child)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[derive(Clone)]
pub(super) struct TestBank {
    path: PathBuf,
    digest_path: PathBuf,
    key: PathBuf,
    inventory_path: PathBuf,
    input_list: PathBuf,
    shingle_text_list: PathBuf,
    short_unit_list: PathBuf,
    allowlist: PathBuf,
    inventory: String,
}

impl TestBank {
    pub(super) fn build(
        binary: &str,
        scratch: &Scratch,
        shingle_texts: &[&str],
        short_units: &[&str],
        allowlist_entries: &[(&str, &str)],
    ) -> Result<Self, Box<dyn Error>> {
        let source = scratch.path("source.txt");
        fs::write(&source, "synthetic private source used only by tests")?;
        let input_list = scratch.path("inputs");
        fs::write(
            &input_list,
            [source.as_os_str().as_encoded_bytes(), b"\0"].concat(),
        )?;
        set_private_mode(&input_list)?;
        let shingle_text_list = scratch.path("shingle-texts");
        let mut shingle_text_bytes = Vec::new();
        for text in shingle_texts {
            shingle_text_bytes.extend_from_slice(text.as_bytes());
            shingle_text_bytes.push(0);
        }
        fs::write(&shingle_text_list, shingle_text_bytes)?;
        set_private_mode(&shingle_text_list)?;
        let short_unit_list = scratch.path("short-units");
        let mut short_unit_bytes = Vec::new();
        for unit in short_units {
            short_unit_bytes.extend_from_slice(unit.as_bytes());
            short_unit_bytes.push(0);
        }
        fs::write(&short_unit_list, short_unit_bytes)?;
        set_private_mode(&short_unit_list)?;
        let allowlist = scratch.path("allowlist.nul");
        let mut allowlist_bytes = Vec::new();
        for (unit, reason) in allowlist_entries {
            allowlist_bytes.extend_from_slice(unit.as_bytes());
            allowlist_bytes.push(0);
            allowlist_bytes.extend_from_slice(reason.as_bytes());
            allowlist_bytes.push(0);
        }
        fs::write(&allowlist, allowlist_bytes)?;
        set_private_mode(&allowlist)?;
        let key = scratch.path("key");
        fs::write(&key, [0x27; 32])?;
        set_private_mode(&key)?;
        let mut bank = Self {
            path: scratch.path("bank.sqlite"),
            digest_path: scratch.path("bank.hmac"),
            key,
            inventory_path: scratch.path("inventory-id"),
            input_list,
            shingle_text_list,
            short_unit_list,
            allowlist,
            inventory: String::new(),
        };
        let built = bank.rebuild(binary)?;
        require_success(&built);
        fs::read_to_string(&bank.inventory_path)?
            .trim()
            .clone_into(&mut bank.inventory);
        Ok(bank)
    }

    fn rebuild(&self, binary: &str) -> Result<Output, Box<dyn Error>> {
        Ok(Command::new(binary)
            .args(["build-bank", "--key-file"])
            .arg(&self.key)
            .arg("--input-list")
            .arg(&self.input_list)
            .arg("--shingle-text-list")
            .arg(&self.shingle_text_list)
            .arg("--short-unit-list")
            .arg(&self.short_unit_list)
            .arg("--allowlist")
            .arg(&self.allowlist)
            .arg("--bank")
            .arg(&self.path)
            .arg("--digest-out")
            .arg(&self.digest_path)
            .arg("--inventory-out")
            .arg(&self.inventory_path)
            .output()?)
    }

    fn verify(&self, binary: &str) -> Result<Output, Box<dyn Error>> {
        Ok(Command::new(binary)
            .args(["verify-bank", "--bank"])
            .arg(&self.path)
            .arg("--bank-digest-file")
            .arg(&self.digest_path)
            .arg("--key-file")
            .arg(&self.key)
            .arg("--inventory-id")
            .arg(&self.inventory)
            .output()?)
    }

    fn scan(&self, binary: &str, repo: &Path, refspec: &str) -> Result<Output, Box<dyn Error>> {
        self.scan_with_remote(binary, repo, refspec, "origin")
    }

    pub(super) fn scan_with_remote(
        &self,
        binary: &str,
        repo: &Path,
        refspec: &str,
        remote: &str,
    ) -> Result<Output, Box<dyn Error>> {
        Ok(Command::new(binary)
            .args(["scan-git", "--repo"])
            .arg(repo)
            .arg("--remote")
            .arg(remote)
            .arg("--bank")
            .arg(&self.path)
            .arg("--bank-digest-file")
            .arg(&self.digest_path)
            .arg("--key-file")
            .arg(&self.key)
            .arg("--inventory-id")
            .arg(&self.inventory)
            .arg("--ref")
            .arg(refspec)
            .output()?)
    }
}

pub(super) fn privacy_binary() -> Result<&'static str, io::Error> {
    option_env!("CARGO_BIN_EXE_maestro-privacy")
        .ok_or_else(|| io::Error::other("the maestro-privacy binary is missing"))
}

pub(super) fn git(repo: &Path, arguments: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()?)
}

pub(super) fn init_repo(repo: &Path, branch: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(repo)?;
    let initial_branch = format!("--initial-branch={branch}");
    require_success(&git(repo, &["init", "--quiet", &initial_branch])?);
    require_success(&git(repo, &["config", "user.name", "Synthetic Test"])?);
    require_success(&git(
        repo,
        &["config", "user.email", "synthetic@example.invalid"],
    )?);
    require_success(&git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/maestro-core.git",
        ],
    )?);
    Ok(())
}

pub(super) fn commit(repo: &Path, message: &str) -> Result<String, Box<dyn Error>> {
    require_success(&git(repo, &["add", "--all"])?);
    require_success(&git(repo, &["commit", "--quiet", "-m", message])?);
    let output = git(repo, &["rev-parse", "HEAD"])?;
    require_success(&output);
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub(super) fn require_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed without a useful diagnostic"
    );
}

fn set_private_mode(path: &Path) -> Result<(), Box<dyn Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    fs::metadata(path)?;
    Ok(())
}

pub(super) fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn git_scan_detects_deleted_renamed_content_and_never_prints_the_match()
-> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[CANARY], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("base.txt"), "Control-M BMC job scheduler")?;
    let base = commit(&repo, "synthetic base")?;

    fs::write(
        repo.join("copy.md"),
        "ＱＵＡＲＴＺ ZEPHYR lantern cobalt apricot meadow orbit ember nebula\r\n",
    )?;
    commit(&repo, "synthetic copy")?;
    require_success(&git(&repo, &["mv", "copy.md", "renamed.data"])?);
    commit(&repo, "synthetic rename")?;
    require_success(&git(&repo, &["rm", "--quiet", "renamed.data"])?);
    commit(&repo, "synthetic removal")?;

    let encoded = CANARY.replace(' ', r"\u0020");
    fs::write(
        repo.join("record.json"),
        format!(r#"{{"value":"{encoded}"}}"#),
    )?;
    commit(&repo, "synthetic encoded record")?;
    require_success(&git(&repo, &["rm", "--quiet", "record.json"])?);
    let candidate = commit(&repo, "synthetic encoded removal")?;
    let refused = bank.scan(binary, &repo, &format!("{candidate}:{base}"))?;
    assert_eq!(refused.status.code(), Some(1), "output: {}", text(&refused));
    assert!(text(&refused).contains("hits=2"));
    assert!(!text(&refused).contains("quartz"));
    assert!(!text(&refused).contains(CANARY));

    require_success(&git(&repo, &["checkout", "--quiet", "-b", "clean", &base])?);
    fs::write(repo.join("clean.txt"), "Control-M BMC job scheduler")?;
    let clean_tip = commit(&repo, "synthetic clean case")?;
    let clean = bank.scan(binary, &repo, &format!("{clean_tip}:{base}"))?;
    require_success(&clean);
    assert!(!text(&clean).contains(CANARY));
    Ok(())
}

#[test]
fn git_scan_skips_content_already_reachable_from_remote_refs() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[REMOTE_CANARY], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("baseline.md"), REMOTE_CANARY)?;
    let baseline = commit(&repo, "synthetic remote baseline")?;
    require_success(&git(
        &repo,
        &["update-ref", "refs/remotes/origin/main", &baseline],
    )?);
    fs::write(repo.join("outgoing.md"), "new unrelated synthetic text")?;
    let candidate = commit(&repo, "synthetic outgoing commit")?;
    let result = bank.scan_with_remote(binary, &repo, &format!("{candidate}:-"), "origin")?;
    require_success(&result);
    assert!(text(&result).contains("hits=0"));
    assert!(!text(&result).contains(REMOTE_CANARY));
    Ok(())
}

#[test]
fn git_scan_ignores_tree_paths_when_the_push_adds_no_objects() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[REMOTE_CANARY], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(
        repo.join(format!("{REMOTE_CANARY}.txt")),
        "synthetic remote file",
    )?;
    let baseline = commit(&repo, "synthetic remote baseline")?;
    require_success(&git(
        &repo,
        &["update-ref", "refs/remotes/origin/main", &baseline],
    )?);

    let result =
        bank.scan_with_remote(binary, &repo, &format!("{baseline}:{baseline}"), "origin")?;
    require_success(&result);
    assert!(text(&result).contains("objects=0 hits=0"));
    Ok(())
}

#[test]
fn short_units_match_exact_windows_and_allowlisted_units_pass() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(
        binary,
        &scratch,
        &[],
        &[
            "how do i restart the agent",
            "which port does zorvex beacon use",
            "which port zorvex uses",
            "where does the zorvex beacon store config",
        ],
        &[(
            "how do i restart the agent",
            "generic operator documentation phrase",
        )],
    )?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("base.txt"), "clean invented content")?;
    let base = commit(&repo, "synthetic base")?;
    fs::write(
        repo.join("question.txt"),
        "HOW DO I RESTART THE AGENT\nWhich port does Zorvex Beacon use?\n\
         which port zorvex uses\nwhere does the zorvex beacon store config?\n\
         which port zorvex\n",
    )?;
    let tip = commit(&repo, "synthetic short unit copy")?;
    let result = bank.scan(binary, &repo, &format!("{tip}:{base}"))?;
    assert_eq!(result.status.code(), Some(1), "output: {}", text(&result));
    assert!(text(&result).contains("hits=3"));
    assert!(!text(&result).contains("restart the agent"));
    assert!(!text(&result).contains("Zorvex"));
    Ok(())
}

#[test]
fn malformed_short_unit_allowlist_refuses_closed() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[], &[], &[])?;
    for malformed in [
        b"which port zorvex\0synthetic one-line reason\0".as_slice(),
        b"which port does Zorvex\0synthetic first line\nsecond line\0".as_slice(),
    ] {
        fs::write(&bank.allowlist, malformed)?;
        set_private_mode(&bank.allowlist)?;
        let result = bank.rebuild(binary)?;
        assert_eq!(result.status.code(), Some(2));
        assert!(!text(&result).contains("Zorvex"));
    }
    Ok(())
}

#[test]
fn one_byte_change_to_an_unused_bank_page_refuses_with_empty_scan_range()
-> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let bank = TestBank::build(binary, &scratch, &[], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("clean.txt"), "clean synthetic data")?;
    let tip = commit(&repo, "synthetic base")?;
    require_success(&bank.verify(binary)?);
    let mut bytes = fs::read(&bank.path)?;
    let final_byte = bytes
        .last_mut()
        .ok_or_else(|| io::Error::other("empty bank"))?;
    *final_byte ^= 1;
    fs::write(&bank.path, bytes)?;
    set_private_mode(&bank.path)?;
    let result = bank.scan(binary, &repo, &format!("{tip}:{tip}"))?;
    assert_eq!(result.status.code(), Some(2), "output: {}", text(&result));
    assert_eq!(bank.verify(binary)?.status.code(), Some(2));
    Ok(())
}

#[test]
fn missing_stale_wrong_key_and_corrupt_banks_refuse_closed() -> Result<(), Box<dyn Error>> {
    let binary = privacy_binary()?;
    let scratch = Scratch::new()?;
    let mut bank = TestBank::build(binary, &scratch, &[], &[], &[])?;
    let repo = scratch.path("candidate");
    init_repo(&repo, "candidate")?;
    fs::write(repo.join("clean.txt"), "clean invented content")?;
    let tip = commit(&repo, "synthetic base")?;
    let refspec = format!("{tip}:{tip}");

    let mut absent = bank.clone();
    absent.path = scratch.path("missing.sqlite");
    absent.inventory = "fresh-inventory".to_owned();
    assert_eq!(absent.scan(binary, &repo, &refspec)?.status.code(), Some(2));

    bank.inventory = "stale-inventory".to_owned();
    let stale = bank.scan(binary, &repo, &refspec)?;
    assert_eq!(stale.status.code(), Some(2));
    assert!(!text(&stale).contains("synthetic private source"));

    let wrong_key_path = scratch.path("wrong-key");
    fs::write(&wrong_key_path, [0x11; 32])?;
    set_private_mode(&wrong_key_path)?;
    let wrong_key = TestBank {
        key: wrong_key_path,
        inventory: fs::read_to_string(scratch.path("inventory-id"))?
            .trim()
            .to_owned(),
        ..bank.clone()
    };
    let wrong = wrong_key.scan(binary, &repo, &refspec)?;
    assert_eq!(wrong.status.code(), Some(2));

    fs::write(&bank.path, b"corrupt synthetic bank")?;
    set_private_mode(&bank.path)?;
    let corrupt = bank.scan(binary, &repo, &refspec)?;
    assert_eq!(corrupt.status.code(), Some(2));
    assert!(!text(&corrupt).contains("synthetic private source"));
    Ok(())
}
