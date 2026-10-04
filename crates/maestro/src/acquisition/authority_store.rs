//! Authority-only SQLite writer; grant changes and audit commit atomically.
use super::authority_host::{Host, unqualified};
use crate::failure::Failure;
use maestro_acquisition::{
    Refusal,
    policy::authority::{Authority, AuthorityRefusal, Grant, Operation, Permit, Target},
};
use rusqlite::{Connection, OptionalExtension as _, params};
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    time::{SystemTime, UNIX_EPOCH},
};

/// Only the qualified authority process opens the writer database.
#[derive(Debug)]
pub(super) struct Store(Connection);
impl Store {
    /// Open a private regular file, refusing unsafe existing metadata.
    pub(super) fn open(host: &Host) -> Result<Self, Failure> {
        let path = host.store.join("authority.sqlite3");
        if !path.exists() {
            drop(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)
                    .map_err(|_| unqualified())?,
            );
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| unqualified())?;
        if !metadata.is_file()
            || metadata.uid() != host.owner_uid
            || metadata.mode() & 0o7777 != 0o600
        {
            return Err(unqualified());
        }
        let database = Connection::open(path).map_err(|_| unqualified())?;
        database
            .execute_batch(
                "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; \
            CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY, record TEXT NOT NULL, \
            revoked INTEGER NOT NULL); \
            CREATE TABLE IF NOT EXISTS audit(sequence INTEGER PRIMARY KEY, \
            peer_uid INTEGER NOT NULL, at INTEGER NOT NULL, action TEXT NOT NULL, \
            record TEXT NOT NULL);",
            )
            .map_err(|_| unqualified())?;
        Ok(Self(database))
    }
    /// Authenticated exact changes, including edit and revoke, with immutable audit history.
    pub(super) fn change(&mut self, grant: &Grant, revoke: bool, peer: u32) -> Result<(), Refusal> {
        let now = SystemTime::now();
        let grant = grant.canonical()?;
        let expiry = grant.expiry()?;
        let record = serde_json::to_string(&grant).map_err(|_| Refusal::Invalid)?;
        if !revoke && expiry <= now {
            let seconds = now
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Refusal::Invalid)?
                .as_secs();
            self.0
                .execute(
                    "INSERT INTO audit(peer_uid,at,action,record) \
                VALUES (?1,?2,'expired_refusal',?3)",
                    params![
                        peer,
                        i64::try_from(seconds).map_err(|_| Refusal::Invalid)?,
                        record
                    ],
                )
                .map_err(|_| Refusal::Unqualified)?;
            return Err(Refusal::Access);
        }
        let transaction = self.0.transaction().map_err(|_| Refusal::Unqualified)?;
        if revoke {
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT record FROM grants WHERE id=?1 AND revoked=0",
                    [&grant.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|_| Refusal::Unqualified)?;
            if existing.as_deref() != Some(&record) {
                return Err(Refusal::Access);
            }
            transaction
                .execute("UPDATE grants SET revoked=1 WHERE id=?1", [&grant.id])
                .map_err(|_| Refusal::Unqualified)?;
        } else {
            transaction
                .execute(
                    "INSERT INTO grants(id,record,revoked) VALUES (?1,?2,0) \
                ON CONFLICT(id) DO UPDATE SET record=excluded.record,revoked=0",
                    params![grant.id, record],
                )
                .map_err(|_| Refusal::Unqualified)?;
        }
        let action = if revoke { "revoke" } else { "grant" };
        let seconds = now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Refusal::Invalid)?
            .as_secs();
        transaction
            .execute(
                "INSERT INTO audit(peer_uid,at,action,record) VALUES (?1,?2,?3,?4)",
                params![
                    peer,
                    i64::try_from(seconds).map_err(|_| Refusal::Invalid)?,
                    action,
                    record
                ],
            )
            .map_err(|_| Refusal::Unqualified)?;
        transaction.commit().map_err(|_| Refusal::Unqualified)
    }
}
impl Authority for Store {
    fn decide(
        &self,
        principal: &str,
        operation: Operation,
        target: &Target,
        now: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        let target = target.canonical()?;
        // ponytail: scan owner-created grants; add expression indexes if grant counts grow.
        let operation_json = serde_json::to_value(operation).map_err(|_| Refusal::Invalid)?;
        let target_json = serde_json::to_string(&target).map_err(|_| Refusal::Invalid)?;
        let record: Option<String> = self
            .0
            .query_row(
                "SELECT record FROM grants WHERE revoked=0 \
            AND json_extract(record,'$.principal')=?1 \
            AND json_extract(record,'$.operation')=?2 \
            AND json_extract(record,'$.target')=?3 \
            ORDER BY json_extract(record,'$.expires_at') DESC,id LIMIT 1",
                params![
                    principal,
                    operation_json.as_str().ok_or(Refusal::Invalid)?,
                    target_json
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| Refusal::Unqualified)?;
        let grant: Grant = serde_json::from_str(&record.ok_or(Refusal::Access)?)
            .map_err(|_| Refusal::Unqualified)?;
        grant.decide(principal, operation, &target, now)
    }
}

#[cfg(test)]
mod tests {
    use super::{Host, Store};
    use maestro_acquisition::policy::authority::{
        Authority as _, AuthorityRefusal, Grant, Operation, Target,
    };
    use maestro_test_scratch::scratch_directory;
    use rustix::process::geteuid;
    use std::{
        fs,
        os::unix::fs::PermissionsExt as _,
        path::PathBuf,
        time::{Duration, UNIX_EPOCH},
    };
    /// Removes even a failed synthetic store test's scratch directory.
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    /// A synthetic store seam, not qualification or a production fallback.
    fn fixture() -> (Scratch, Host, Store, Grant) {
        let directory = Scratch(scratch_directory().unwrap());
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
        let host = Host {
            store: directory.0.clone(),
            socket: directory.0.join("unused"),
            owner_uid: geteuid().as_raw(),
            pipeline_uid: 65534,
            connector_uid: 65533,
            launcher: directory.0.join("never-launched"),
        };
        // Pure store seam, not a claimed host qualification or production fallback.
        let mut store = Store::open(&host).unwrap();
        let target = Target {
            scope: "workspace/default/collection/synthetic".into(),
            source: "handbook".into(),
            account: "public".into(),
            resource: "https://synthetic.example:443/manual".into(),
        };
        let grant = Grant {
            id: "fixture-expiry".into(),
            principal: "65534".into(),
            operation: Operation::Fetch,
            target: target.clone(),
            expires_at: "2099-01-01T00:00:00Z".into(),
        };
        store.change(&grant, false, host.owner_uid).unwrap();
        (directory, host, store, grant)
    }
    #[test]
    fn n05_store_fetch_and_robots_override_are_independent() {
        let (_directory, host, mut store, fetch) = fixture();
        let mut robots = fetch.clone();
        robots.id = "fixture-robots".into();
        robots.operation = Operation::RobotsOverride;
        store.change(&robots, false, host.owner_uid).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
        for grant in [fetch, robots] {
            let permit = store
                .decide(&grant.principal, grant.operation, &grant.target, now)
                .unwrap();
            assert_eq!(permit.grant_id, grant.id);
        }
    }
    #[test]
    fn n05_store_multiple_grants_prefer_live_longest_expiry() {
        let (_directory, host, mut store, longest) = fixture();
        let mut shorter = longest.clone();
        shorter.id = "fixture-shorter".into();
        shorter.expires_at = "2098-01-01T00:00:00Z".into();
        store.change(&shorter, false, host.owner_uid).unwrap();
        let now = shorter.expiry().unwrap() + Duration::from_secs(1);
        let permit = store
            .decide(&longest.principal, longest.operation, &longest.target, now)
            .unwrap();
        assert_eq!(permit.grant_id, longest.id);
    }
    #[test]
    fn n05_expired_read_only_decisions_name_the_grant_and_leave_store_byte_identical() {
        let (_directory, host, store, grant) = fixture();
        let target = grant.target;
        let path = host.store.join("authority.sqlite3");
        let before = fs::read(&path).unwrap();
        assert_eq!(
            store.decide(
                "65534",
                Operation::Fetch,
                &target,
                UNIX_EPOCH + Duration::from_secs(4_110_000_001)
            ),
            Err(AuthorityRefusal::Expired {
                grant_id: "fixture-expiry".into()
            })
        );
        assert_eq!(
            fs::read(path).unwrap(),
            before,
            "read-only expiry decisions wrote authority state"
        );
    }
    #[test]
    fn debt_store_metadata_guards_are_independent() {
        let (_directory, mut host, store, _) = fixture();
        drop(store);
        let path = host.store.join("authority.sqlite3");
        host.owner_uid += 1;
        assert!(Store::open(&host).is_err());
        host.owner_uid -= 1;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Store::open(&host).is_err());
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(Store::open(&host).is_err());
    }
    #[test]
    fn debt_store_expired_grant_and_exact_revoke_are_audited_atomically() {
        use maestro_acquisition::Refusal;
        let (_directory, host, mut store, grant) = fixture();
        let mut expired = grant.clone();
        expired.id = "expired".into();
        expired.expires_at = "2000-01-01T00:00:00Z".into();
        assert_eq!(
            store.change(&expired, false, host.owner_uid),
            Err(Refusal::Access)
        );
        let mut mismatch = grant.clone();
        mismatch.expires_at = "2098-01-01T00:00:00Z".into();
        assert_eq!(
            store.change(&mismatch, true, host.owner_uid),
            Err(Refusal::Access)
        );
        assert!(store.change(&grant, true, host.owner_uid).is_ok());
        assert_eq!(
            store.change(&grant, true, host.owner_uid),
            Err(Refusal::Access)
        );
        let actions: Vec<String> = store
            .0
            .prepare("SELECT action FROM audit ORDER BY sequence")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(actions, ["grant", "expired_refusal", "revoke"]);
    }
}
