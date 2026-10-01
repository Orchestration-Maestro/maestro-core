//! Unique run attempts, immutable receipt snapshots and bounded stage inventories.
use super::privacy::{self, Handle, Progress, ProtectedArtifact, Reason, ReceiptError, Status};
use crate::{
    journal::{NewEvent, event},
    scope::{Scope, ScopeSet},
    store::Database,
};
use rusqlite::{OptionalExtension as _, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, iter::once};

/// Measured usage, separate from item and attempt counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetUsage {
    /// Elapsed wall-clock milliseconds.
    pub elapsed_ms: u64,
    /// Received bytes across dispatch attempts.
    pub response_bytes: u64,
    /// Bytes charged to staging storage.
    pub staging_bytes: u64,
}
/// The counting stage; the same distinct item may appear in several stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Eligible items enumerated at the source.
    Discovery,
    /// Items dispatched to a transport.
    Capture,
    /// Captures interpreted by an extractor.
    Extraction,
    /// Accepted revisions sent downstream.
    Publication,
}
/// A distinct item's disposition at one stage, not a dispatch attempt counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemDisposition {
    /// The item was discovered.
    Discovered,
    /// The stage accepted the item.
    Accepted,
    /// The stage verified an unchanged item.
    Unchanged,
    /// Policy denied the item.
    Denied,
    /// A capability or authority blocked the item.
    Blocked,
    /// The stage refused the item.
    Refused,
    /// An authorized lifecycle action withdrew the item.
    Withdrawn,
    /// Work remains pending, never completed.
    Pending,
}
/// One distinct item in a partition and counting stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageItem {
    /// Opaque item identity; never a source URL.
    pub item: Handle,
    /// This stage's disposition.
    pub disposition: ItemDisposition,
    /// Protected capture, refusal or other evidence, if available.
    pub evidence: Option<Handle>,
}
/// The exact inventory wire schema; unsupported versions refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventorySchema {
    /// Version one of the acquisition inventory contract.
    #[serde(rename = "maestro-acquisition-inventory/1")]
    V1,
}
/// The exact receipt wire schema; unsupported versions refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptSchema {
    /// Version one of the acquisition receipt contract.
    #[serde(rename = "maestro-acquisition-receipt/1")]
    V1,
}
/// A bounded immutable inventory page. Large inventories use multiple handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryPage {
    /// Required versioned wire contract.
    pub schema: InventorySchema,
    /// Protected partition coverage evidence.
    pub partition: Handle,
    /// The stage whose distinct items this page counts.
    pub stage: Stage,
    /// Whether this partition's coverage evidence is complete.
    pub complete: bool,
    /// At most 1,000 distinct items; duplicates within a stage refuse.
    pub items: Vec<StageItem>,
}
/// A versioned immutable receipt snapshot; a pending attempt survives crashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// Required versioned wire contract.
    pub schema: ReceiptSchema,
    /// Logical run identity, retained by resumed attempts.
    pub run: Handle,
    /// Unique attempt identity; terminal snapshots never overwrite another attempt.
    pub attempt: Handle,
    /// Protected frozen inputs, including source, owner, policy and profile refs.
    pub inputs: Handle,
    /// Protected current authority/policy tightening decisions.
    pub tightening: Vec<Handle>,
    /// Protected partition/stage inventories, paged rather than one unbounded frame.
    pub inventories: Vec<Handle>,
    /// Dispatch attempts, not added to distinct stage items.
    pub attempts: u64,
    /// Resource observations.
    pub budget: BudgetUsage,
    /// Protected downstream revision/generation references.
    pub downstream: Vec<Handle>,
    /// Only complete is successful completion; pending is nonterminal.
    pub status: Status,
    /// Typed dispatch outcome; error text remains in protected evidence.
    pub reason: Reason,
}
impl Receipt {
    /// All protected references this snapshot inherits transitively.
    fn references(&self) -> Vec<Handle> {
        once(self.inputs)
            .chain(self.tightening.iter().copied())
            .chain(self.inventories.iter().copied())
            .chain(self.downstream.iter().copied())
            .collect()
    }
    /// The content-free projection safe for all progress sinks.
    fn progress(&self) -> Progress {
        Progress {
            receipt: self.attempt,
            status: self.status,
            reason: self.reason,
        }
    }
}
/// Kernel receipt port. Adapters preserve immutability, scope inheritance and
/// fresh access checks. Writers are trusted kernel callers, never view principals.
pub trait Receipts: Send + Sync {
    /// Retains protected bytes and transitive opaque references, without publishing them.
    /// # Errors
    /// Invalid collection scope, payload/reference bounds, absent reference or storage failure.
    fn retain(
        &self,
        scope: &Scope,
        bytes: &[u8],
        references: &[Handle],
    ) -> Result<Handle, ReceiptError>;
    /// Reads protected bytes after rechecking current grants for the entire closure.
    /// Unknown and denied handles both return none, without metadata.
    /// # Errors
    /// Content-free storage failure.
    fn read(
        &self,
        principal: &str,
        handle: Handle,
    ) -> Result<Option<ProtectedArtifact>, ReceiptError>;
    /// Stores a bounded, unique stage inventory under its partition's inherited scope.
    /// # Errors
    /// Invalid page/duplicate items/references or storage failure.
    fn retain_inventory(&self, scope: &Scope, page: &InventoryPage)
    -> Result<Handle, ReceiptError>;
    /// Durably starts one pending attempt before any acquisition dispatch.
    /// # Errors
    /// Nonpending status, duplicate attempt, invalid reference/bound or storage failure.
    fn begin(&self, scope: &Scope, receipt: &Receipt) -> Result<(), ReceiptError>;
    /// Finalizes an attempt exactly once, retaining its initial snapshot as well.
    /// # Errors
    /// Missing/terminal attempt, changed frozen inputs, false completion or storage failure.
    fn finish(&self, receipt: &Receipt) -> Result<(), ReceiptError>;
    /// Authorized local view; historical and transitive references recheck current grants.
    /// # Errors
    /// Content-free storage/schema failure.
    fn inspect(&self, principal: &str, attempt: Handle) -> Result<Option<Receipt>, ReceiptError>;
    /// Reads at most 1–1,000 authorized attempts of a run in opaque ID order.
    /// # Errors
    /// Invalid page limit or content-free storage failure.
    fn page(
        &self,
        principal: &str,
        run: Handle,
        after: Option<Handle>,
        limit: u16,
    ) -> Result<Vec<Progress>, ReceiptError>;
}
impl Receipts for Database {
    fn retain(
        &self,
        scope: &Scope,
        bytes: &[u8],
        references: &[Handle],
    ) -> Result<Handle, ReceiptError> {
        privacy::validate(scope, bytes, references)?;
        let digest = self.put(bytes, "application/octet-stream")?;
        self.write(|tx| privacy::retain_on(tx, scope, &digest, references))
    }
    fn read(
        &self,
        principal: &str,
        handle: Handle,
    ) -> Result<Option<ProtectedArtifact>, ReceiptError> {
        privacy::read(self, principal, handle)
    }
    fn retain_inventory(
        &self,
        scope: &Scope,
        page: &InventoryPage,
    ) -> Result<Handle, ReceiptError> {
        validate_page(page)?;
        let references = inventory_references(page);
        self.retain(scope, &serde_json::to_vec(page)?, &references)
    }
    fn begin(&self, scope: &Scope, receipt: &Receipt) -> Result<(), ReceiptError> {
        if receipt.status != Status::Pending || receipt.reason != Reason::None {
            return Err(ReceiptError::Invalid);
        }
        let bytes = serde_json::to_vec(receipt)?;
        let references = receipt.references();
        privacy::validate(scope, &bytes, &references)?;
        validate_inventories(self, receipt)?;
        let digest = self.put(&bytes, "application/octet-stream")?;
        self.write(|tx| {
            let snapshot = privacy::retain_on(tx, scope, &digest, &references)?;
            tx.execute(
                "INSERT INTO acquisition_receipts (attempt, run, scope, initial)
                VALUES (?1, ?2, ?3, ?4)",
                params![
                    receipt.attempt.to_string(),
                    receipt.run.to_string(),
                    scope.as_str(),
                    snapshot.to_string()
                ],
            )?;
            publish(tx, scope, receipt.progress())
        })
    }
    fn finish(&self, receipt: &Receipt) -> Result<(), ReceiptError> {
        if receipt.status == Status::Pending {
            return Err(ReceiptError::Invalid);
        }
        let Some((scope, initial)) = current(self, receipt.attempt)? else {
            return Err(ReceiptError::Conflict);
        };
        let frozen: Receipt = serde_json::from_slice(&privacy::snapshot(self, &initial)?)?;
        if frozen.run != receipt.run || frozen.inputs != receipt.inputs {
            return Err(ReceiptError::Conflict);
        }
        validate_inventories(self, receipt)?;
        let bytes = serde_json::to_vec(receipt)?;
        let mut references = receipt.references();
        references.push(initial.parse()?);
        privacy::validate(&scope, &bytes, &references)?;
        let digest = self.put(&bytes, "application/octet-stream")?;
        self.write(|tx| {
            let snapshot = privacy::retain_on(tx, &scope, &digest, &references)?;
            let changed = tx.execute(
                "UPDATE acquisition_receipts SET terminal = ?2
                WHERE attempt = ?1 AND terminal IS NULL",
                params![receipt.attempt.to_string(), snapshot.to_string()],
            )?;
            if changed == 0 {
                return Err(ReceiptError::Conflict);
            }
            publish(tx, &scope, receipt.progress())
        })
    }
    fn inspect(&self, principal: &str, attempt: Handle) -> Result<Option<Receipt>, ReceiptError> {
        let Some((_, snapshot)) = current(self, attempt)? else {
            return Ok(None);
        };
        let handle = snapshot.parse()?;
        self.read(principal, handle)?
            .map(|artifact| serde_json::from_slice(artifact.bytes()).map_err(Into::into))
            .transpose()
    }
    fn page(
        &self,
        principal: &str,
        run: Handle,
        after: Option<Handle>,
        limit: u16,
    ) -> Result<Vec<Progress>, ReceiptError> {
        if limit == 0 || limit > 1000 {
            return Err(ReceiptError::Invalid);
        }
        let reader = self.reader()?;
        let scopes = self.visible(principal)?;
        let query = format!(
            "WITH RECURSIVE closure(root, id) AS (
            SELECT coalesce(terminal, initial), coalesce(terminal, initial)
                FROM acquisition_receipts WHERE run = ?1 AND (?2 IS NULL OR attempt > ?2)
            UNION SELECT closure.root, child FROM acquisition_evidence_links
                JOIN closure ON parent = closure.id)
            SELECT attempt FROM acquisition_receipts AS receipt
                WHERE run = ?1 AND (?2 IS NULL OR attempt > ?2)
            AND NOT EXISTS (SELECT 1 FROM closure
                JOIN acquisition_evidence AS linked ON linked.id = closure.id
                WHERE closure.root = coalesce(receipt.terminal, receipt.initial) AND NOT {})
            ORDER BY attempt LIMIT ?4",
            ScopeSet::condition("linked.scope", 3)
        );
        let attempts: Vec<String> = reader
            .prepare(&query)?
            .query_map(
                params![
                    run.to_string(),
                    after.map(|id| id.to_string()),
                    scopes.parameter(),
                    limit
                ],
                |row| row.get(0),
            )?
            .collect::<Result<_, _>>()?;
        let mut result = Vec::new();
        for attempt in attempts {
            let handle = attempt.parse()?;
            if let Some(receipt) = self.inspect(principal, handle)? {
                result.push(receipt.progress());
            }
        }
        Ok(result)
    }
}
/// Finds the latest snapshot; finalization's conditional update owns the race guard.
fn current(db: &Database, attempt: Handle) -> Result<Option<(Scope, String)>, ReceiptError> {
    let found: Option<(String, String)> = db.reader()?.query_row(
        "SELECT scope, coalesce(terminal, initial) FROM acquisition_receipts WHERE attempt = ?1",
        [attempt.to_string()], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
    found
        .map(|(scope, snapshot)| Ok((scope.parse().map_err(|_| ReceiptError::Storage)?, snapshot)))
        .transpose()
}
/// Checks the page bound and distinct counting units at the common boundary.
fn validate_page(page: &InventoryPage) -> Result<(), ReceiptError> {
    let mut seen = BTreeSet::new();
    if page.items.len() > 1000 || !page.items.iter().all(|item| seen.insert(item.item)) {
        return Err(ReceiptError::Invalid);
    }
    Ok(())
}
/// Protected partition and item references, without duplicate scope edges.
fn inventory_references(page: &InventoryPage) -> Vec<Handle> {
    once(page.partition)
        .chain(page.items.iter().filter_map(|item| item.evidence))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
/// Reconciles counting units and refuses incomplete inventories labeled successful.
fn validate_inventories(db: &Database, receipt: &Receipt) -> Result<(), ReceiptError> {
    let mut seen = BTreeSet::new();
    let mut complete = receipt.reason == Reason::None;
    for handle in &receipt.inventories {
        let page: InventoryPage =
            serde_json::from_slice(&privacy::snapshot(db, &handle.to_string())?)?;
        validate_page(&page)?;
        if !privacy::linked(&db.reader()?, *handle, &inventory_references(&page))? {
            return Err(ReceiptError::Invalid);
        }
        complete &= page.complete;
        for item in page.items {
            if !seen.insert((page.stage, item.item)) {
                return Err(ReceiptError::Invalid);
            }
            match item.disposition {
                ItemDisposition::Discovered if page.stage == Stage::Discovery => {}
                ItemDisposition::Discovered
                | ItemDisposition::Pending
                | ItemDisposition::Blocked
                | ItemDisposition::Refused => complete = false,
                ItemDisposition::Accepted
                | ItemDisposition::Unchanged
                | ItemDisposition::Denied
                | ItemDisposition::Withdrawn => {}
            }
        }
    }
    if receipt.status == Status::Complete && !complete {
        return Err(ReceiptError::Invalid);
    }
    Ok(())
}
/// Emits only the typed allow-list, atomically with the snapshot reference.
fn publish(tx: &Transaction<'_>, scope: &Scope, progress: Progress) -> Result<(), ReceiptError> {
    let stream = format!("acquisition/{}", progress.receipt);
    event::record(
        tx,
        &NewEvent {
            stream: &stream,
            subject: &stream,
            scope: scope.as_str(),
            r#type: "maestro.acquisition.progress.v1",
            data: &serde_json::to_value(progress)?,
        },
    )?;
    Ok(())
}
