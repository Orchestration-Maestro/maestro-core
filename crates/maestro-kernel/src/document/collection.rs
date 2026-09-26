//! Collections, the sources they declare and the documents those sources
//! hold: what every revision is recorded under (docs/architecture/01 §1).

use super::error::Error;
use crate::store::Database;
use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params, types::Type};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;

/// A collection: a logical body of knowledge, as its declaration names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// Its id, such as `ctm`.
    pub id: String,
    /// What it holds, for people.
    pub title: String,
    /// Who may see what it derives, such as `private`: a scope tag on every
    /// record derived from it.
    pub visibility: String,
    /// The profile each stage processes its sources with, by stage:
    /// `chunking` to `structural-500-700/1`, for example.
    pub profiles: BTreeMap<String, String>,
}

/// A source: a declared origin of a collection's documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The collection that declares it.
    pub collection_id: String,
    /// Its id, unique in its collection only, such as `docs-core`.
    pub id: String,
    /// How its documents arrive, such as `import`.
    pub kind: String,
    /// How its documents are fetched, if they are: an import has none.
    pub transport: Option<String>,
    /// Where its documents are found, such as an import's manifest.
    pub reference: String,
    /// Its own profiles, by stage.
    pub profiles: BTreeMap<String, String>,
}

/// A document: the stable identity of one source document, whatever its
/// revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Its id, which the import derives from its source reference.
    pub id: String,
    /// The collection it belongs to.
    pub collection_id: String,
    /// The source of that collection it comes from.
    pub source_id: String,
    /// Where it comes from: its origin URL, or `corpus-path:` and its path.
    pub source_ref: String,
}

impl Database {
    /// Records `collection` as its declaration now names it: a collection
    /// recorded before takes its title, visibility and profiles.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot record it.
    pub fn record_collection(&self, collection: &Collection) -> Result<(), Error> {
        self.write(|transaction| {
            transaction.execute(
                "INSERT INTO collections (id, title, visibility, profiles_json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (id) DO UPDATE SET title = excluded.title,
                   visibility = excluded.visibility, profiles_json = excluded.profiles_json",
                params![
                    collection.id,
                    collection.title,
                    collection.visibility,
                    profiles_json(&collection.profiles),
                ],
            )?;
            Ok(())
        })
    }

    /// The collection `id`, if it is recorded.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn collection(&self, id: &str) -> Result<Option<Collection>, Error> {
        let collection = self
            .reader()?
            .query_row(
                "SELECT id, title, visibility, profiles_json FROM collections WHERE id = ?1",
                [id],
                |row| {
                    Ok(Collection {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        visibility: row.get(2)?,
                        profiles: json(row, 3)?,
                    })
                },
            )
            .optional()?;
        Ok(collection)
    }

    /// Records `source` in its collection as its declaration now names it: a
    /// source recorded before takes its kind, transport, reference and
    /// profiles.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when its collection is not recorded or the database
    /// cannot record it.
    pub fn record_source(&self, source: &Source) -> Result<(), Error> {
        self.write(|transaction| {
            transaction.execute(
                "INSERT INTO sources (collection_id, id, kind, transport, reference, profiles_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (collection_id, id) DO UPDATE SET kind = excluded.kind,
                   transport = excluded.transport, reference = excluded.reference,
                   profiles_json = excluded.profiles_json",
                params![
                    source.collection_id,
                    source.id,
                    source.kind,
                    source.transport,
                    source.reference,
                    profiles_json(&source.profiles),
                ],
            )?;
            Ok(())
        })
    }

    /// The source `id` of the collection `collection_id`, if it is recorded.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn source(&self, collection_id: &str, id: &str) -> Result<Option<Source>, Error> {
        let source = self
            .reader()?
            .query_row(
                "SELECT collection_id, id, kind, transport, reference, profiles_json
                 FROM sources WHERE collection_id = ?1 AND id = ?2",
                [collection_id, id],
                |row| {
                    Ok(Source {
                        collection_id: row.get(0)?,
                        id: row.get(1)?,
                        kind: row.get(2)?,
                        transport: row.get(3)?,
                        reference: row.get(4)?,
                        profiles: json(row, 5)?,
                    })
                },
            )
            .optional()?;
        Ok(source)
    }

    /// Records `document`, unless it is recorded already just so, which
    /// changes nothing.
    ///
    /// # Errors
    ///
    /// [`Error::DocumentConflict`] when its id is recorded under another
    /// collection, source or source reference, [`Error::SourceRefConflict`]
    /// when another document of its collection is recorded from its source
    /// reference, and [`Error::Store`] when its source is not recorded or the
    /// database cannot record it.
    pub fn record_document(&self, document: &Document) -> Result<(), Error> {
        self.write(
            |transaction| match find_document(transaction, &document.id)? {
                Some(recorded) if recorded == *document => Ok(()),
                Some(_) => Err(Error::DocumentConflict(document.id.clone())),
                None => insert_document(transaction, document),
            },
        )
    }

    /// The document `id`, if it is recorded.
    ///
    /// # Errors
    ///
    /// [`Error::Store`] when the database cannot be read.
    pub fn document(&self, id: &str) -> Result<Option<Document>, Error> {
        Ok(find_document(&self.reader()?, id)?)
    }
}

/// Records the new `document` inside `transaction`, unless another document
/// of its collection is recorded from its source reference.
fn insert_document(transaction: &Transaction<'_>, document: &Document) -> Result<(), Error> {
    let recorded = transaction
        .query_row(
            "SELECT id FROM documents WHERE collection_id = ?1 AND source_ref = ?2",
            [&document.collection_id, &document.source_ref],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(recorded) = recorded {
        return Err(Error::SourceRefConflict {
            recorded,
            given: document.id.clone(),
        });
    }
    transaction.execute(
        "INSERT INTO documents (id, collection_id, source_id, source_ref)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            document.id,
            document.collection_id,
            document.source_id,
            document.source_ref,
        ],
    )?;
    Ok(())
}

/// The document `id` that `connection` records, if any.
fn find_document(connection: &Connection, id: &str) -> rusqlite::Result<Option<Document>> {
    connection
        .query_row(
            "SELECT id, collection_id, source_id, source_ref FROM documents WHERE id = ?1",
            [id],
            |row| {
                Ok(Document {
                    id: row.get(0)?,
                    collection_id: row.get(1)?,
                    source_id: row.get(2)?,
                    source_ref: row.get(3)?,
                })
            },
        )
        .optional()
}

/// `profiles` as the JSON object their column holds.
fn profiles_json(profiles: &BTreeMap<String, String>) -> String {
    let object = profiles
        .iter()
        .map(|(stage, profile)| (stage.clone(), Value::from(profile.as_str())))
        .collect();
    Value::Object(object).to_string()
}

/// The value column `index` of `row` holds as JSON text.
pub(super) fn json<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    serde_json::from_str(&text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
