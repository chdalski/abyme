//! The `LanceDB` table that holds the index: `id`, `text`, and `vector`, with
//! a full-text index on `text`.

use std::collections::BTreeMap;
use std::sync::Arc;

use abyme_model::ElementId;
use futures::TryStreamExt as _;
use lancedb::arrow::arrow_array::types::Float32Type;
use lancedb::arrow::arrow_array::{Array, FixedSizeListArray, RecordBatch, StringArray};
use lancedb::arrow::arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use lancedb::expr::{col, lit};
use lancedb::index::Index as LanceIndex;
use lancedb::index::scalar::{FtsIndexBuilder, FullTextSearchQuery};
use lancedb::query::{ExecutableQuery as _, QueryBase as _, Select};
use lancedb::table::{OptimizeAction, OptimizeOptions};
use lancedb::{Connection, Table};

/// The name of the table in its database directory.
const TABLE: &str = "elements";

/// What a stored table holds, or that it cannot be used and must be
/// rebuilt.
pub enum Loaded {
    /// The table and its stored text per element.
    Ready(Table, BTreeMap<ElementId, String>),
    /// The table cannot be opened, has another schema, or holds rows that
    /// are not one per valid ID.
    Damaged,
}

/// The table's schema for vectors of `dimension` values.
pub fn schema(dimension: i32) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimension,
            ),
            false,
        ),
    ]))
}

/// Opens the table, creating it when it does not exist, and reads what it
/// stores.
pub async fn load(db: &Connection, dimension: i32) -> Result<Loaded, lancedb::Error> {
    let table = match db.open_table(TABLE).execute().await {
        Ok(table) => table,
        Err(lancedb::Error::TableNotFound { .. }) => {
            return Ok(Loaded::Ready(create(db, dimension).await?, BTreeMap::new()));
        }
        Err(_) => return Ok(Loaded::Damaged),
    };
    let Ok(stored_schema) = table.schema().await else {
        return Ok(Loaded::Damaged);
    };
    if !same_columns(&stored_schema, &schema(dimension)) {
        return Ok(Loaded::Damaged);
    }
    Ok(stored(&table)
        .await
        .map_or(Loaded::Damaged, |stored| Loaded::Ready(table, stored)))
}

/// Creates the empty table.
pub async fn create(db: &Connection, dimension: i32) -> Result<Table, lancedb::Error> {
    db.create_empty_table(TABLE, schema(dimension))
        .execute()
        .await
}

/// Opens the table if it exists.
pub async fn open(db: &Connection) -> Result<Option<Table>, lancedb::Error> {
    match db.open_table(TABLE).execute().await {
        Ok(table) => Ok(Some(table)),
        Err(lancedb::Error::TableNotFound { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Removes the rows of `ids` through a typed expression; no SQL text is
/// built from the IDs.
pub async fn delete(table: &Table, ids: &[ElementId]) -> Result<(), lancedb::Error> {
    if ids.is_empty() {
        return Ok(());
    }
    let predicate = col("id").in_list(ids.iter().map(|id| lit(id.as_str())).collect(), false);
    table.delete(&predicate).await.map(|_| ())
}

/// Adds one row per element with its text and vector. The caller has
/// checked that there is one vector of `dimension` values per row.
pub async fn add(
    table: &Table,
    rows: &[(ElementId, String)],
    vectors: Vec<Vec<f32>>,
    dimension: i32,
) -> Result<(), AddError> {
    if rows.is_empty() {
        return Ok(());
    }
    let ids = StringArray::from_iter_values(rows.iter().map(|(id, _)| id.as_str()));
    let texts = StringArray::from_iter_values(rows.iter().map(|(_, text)| text.as_str()));
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        vectors
            .into_iter()
            .map(|vector| Some(vector.into_iter().map(Some))),
        dimension,
    );
    let batch = RecordBatch::try_new(
        schema(dimension),
        vec![Arc::new(ids), Arc::new(texts), Arc::new(vectors)],
    )?;
    table.add(batch).execute().await?;
    Ok(())
}

/// Builds the full-text index on `text` when it is missing, or adds the
/// rows it does not cover yet, so that a refresh cut off before this step
/// is completed by the next one, whether or not elements changed.
pub async fn update_full_text_index(table: &Table) -> Result<(), lancedb::Error> {
    let index = table
        .list_indices()
        .await?
        .into_iter()
        .find(|index| index.columns.iter().any(|column| column == "text"));
    match index {
        Some(index) => {
            let unindexed = table
                .index_stats(&index.name)
                .await?
                .map_or(0, |stats| stats.num_unindexed_rows);
            if unindexed > 0 {
                table
                    .optimize(OptimizeAction::Index(OptimizeOptions::default()))
                    .await?;
            }
            Ok(())
        }
        None if table.count_rows(None).await? == 0 => Ok(()),
        None => {
            table
                .create_index(&["text"], LanceIndex::FTS(FtsIndexBuilder::default()))
                .execute()
                .await
        }
    }
}

/// The IDs of the rows that best match `query`, by full text and by
/// `vector`, best first. The query is a set of plain terms, never query
/// syntax.
pub async fn search(
    table: &Table,
    query: &str,
    vector: Vec<f32>,
    limit: usize,
) -> Result<Vec<String>, lancedb::Error> {
    let batches: Vec<RecordBatch> = table
        .query()
        .full_text_search(FullTextSearchQuery::new(query.to_owned()))
        .nearest_to(vector)?
        .limit(limit)
        .execute()
        .await?
        .try_collect()
        .await?;
    column_values(&batches, "id").ok_or_else(|| lancedb::Error::Runtime {
        message: "the search result has no column of element IDs".to_owned(),
    })
}

/// Why rows could not be added.
#[derive(Debug, thiserror::Error)]
pub enum AddError {
    /// The rows did not form a valid batch.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    /// `LanceDB` rejected them.
    #[error(transparent)]
    Lance(#[from] lancedb::Error),
}

/// Whether two schemas have the same columns with the same types.
fn same_columns(stored: &Schema, expected: &Schema) -> bool {
    stored.fields().len() == expected.fields().len()
        && stored
            .fields()
            .iter()
            .zip(expected.fields().iter())
            .all(|(stored, expected)| {
                stored.name() == expected.name() && stored.data_type() == expected.data_type()
            })
}

/// The stored text per element, or none when a row's ID is invalid or
/// appears twice, or the rows cannot be read.
async fn stored(table: &Table) -> Option<BTreeMap<ElementId, String>> {
    let batches: Vec<RecordBatch> = table
        .query()
        .select(Select::columns(&["id", "text"]))
        .execute()
        .await
        .ok()?
        .try_collect()
        .await
        .ok()?;
    let ids = column_values(&batches, "id")?;
    let texts = column_values(&batches, "text")?;
    if ids.len() != texts.len() {
        return None;
    }
    let mut stored = BTreeMap::new();
    for (id, text) in ids.into_iter().zip(texts) {
        let id: ElementId = id.parse().ok()?;
        if stored.insert(id, text).is_some() {
            return None;
        }
    }
    Some(stored)
}

/// The values of a string column across `batches`, or none when a batch
/// lacks it or it holds another type or nulls.
fn column_values(batches: &[RecordBatch], name: &str) -> Option<Vec<String>> {
    let mut values = Vec::new();
    for batch in batches {
        let column = batch.column_by_name(name)?;
        let strings = column.as_any().downcast_ref::<StringArray>()?;
        if strings.null_count() > 0 {
            return None;
        }
        values.extend(strings.iter().flatten().map(str::to_owned));
    }
    Some(values)
}
