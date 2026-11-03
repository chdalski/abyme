//! The index through its public API, with a real `LanceDB` table in a
//! temporary cache and a deterministic fake embedder instead of a model.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::num::NonZeroU32;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use abyme_core::Snapshot;
use abyme_index::{
    EmbedError, Embedder, Index, IndexError, MAX_QUERY_CHARS, MAX_RESULTS, ModelFile, ModelFiles,
    ModelSpec, ProjectKey, UserCache, indexed_text,
};
use abyme_model::{
    Board, Element, ElementFields, ElementId, Mode, Placement, Rect, Relations, StatementKind,
    TypeRef, Zone,
};
use fastembed::{Pooling, QuantizationMode};
use futures::TryStreamExt as _;
use lancedb::arrow::arrow_array::types::Float32Type;
use lancedb::arrow::arrow_array::{Array, ArrayRef, FixedSizeListArray, RecordBatch, StringArray};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};
use lancedb::query::{ExecutableQuery as _, QueryBase as _, Select};
use rstest::rstest;
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The length of the fake embedder's vectors.
const DIMENSION: usize = 32;

/// The files of the fake model; the index never reads them.
const NO_FILE: ModelFile = ModelFile {
    name: "unused",
    sha256: "",
    size: 0,
};

static SPEC: ModelSpec = ModelSpec {
    name: "fake",
    repository: "test/fake",
    revision: "r1",
    files: ModelFiles {
        onnx: NO_FILE,
        tokenizer: NO_FILE,
        config: NO_FILE,
        special_tokens_map: NO_FILE,
        tokenizer_config: NO_FILE,
    },
    dimension: DIMENSION,
    pooling: Pooling::Cls,
    quantization: QuantizationMode::None,
};

/// The same model name and revision with another dimension, as if the
/// stored table came from a different model build.
static OTHER_DIMENSION: ModelSpec = ModelSpec {
    name: "fake",
    repository: "test/fake",
    revision: "r1",
    files: ModelFiles {
        onnx: NO_FILE,
        tokenizer: NO_FILE,
        config: NO_FILE,
        special_tokens_map: NO_FILE,
        tokenizer_config: NO_FILE,
    },
    dimension: 8,
    pooling: Pooling::Cls,
    quantization: QuantizationMode::None,
};

/// The environment variables that turn `child_refreshes` into the body of a
/// child process.
const CHILD_CACHE: &str = "ABYME_INDEX_TEST_CACHE";
const CHILD_ROOT: &str = "ABYME_INDEX_TEST_ROOT";

/// How many elements the two-process test indexes.
const SHARED_ELEMENTS: usize = 40;

/// Counts words into a vector, so texts that share words lie close.
#[derive(Default)]
struct WordsEmbedder {
    embedded: usize,
}

impl Embedder for WordsEmbedder {
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.embedded += texts.len();
        Ok(texts
            .iter()
            .map(|text| words_vector(text, DIMENSION))
            .collect())
    }
}

fn words_vector(text: &str, dimension: usize) -> Vec<f32> {
    let mut vector = vec![0.0; dimension];
    for word in text.split_whitespace() {
        let hash = word.to_lowercase().bytes().fold(7_usize, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(usize::from(byte))
        });
        if let Some(slot) = vector.get_mut(hash % dimension) {
            *slot += 1.0;
        }
    }
    vector
}

/// Returns given vectors for given texts and zeros for any other.
struct TableEmbedder(BTreeMap<String, Vec<f32>>);

impl Embedder for TableEmbedder {
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(texts
            .iter()
            .map(|text| {
                self.0
                    .get(text)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; DIMENSION])
            })
            .collect())
    }
}

/// Returns broken output, as a faulty model might.
enum BrokenEmbedder {
    TooFew,
    WrongLength,
    NotFinite,
}

impl Embedder for BrokenEmbedder {
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(match self {
            Self::TooFew => texts.iter().skip(1).map(|_| vec![0.0; DIMENSION]).collect(),
            Self::WrongLength => texts.iter().map(|_| vec![0.0; DIMENSION + 1]).collect(),
            Self::NotFinite => texts.iter().map(|_| vec![f32::NAN; DIMENSION]).collect(),
        })
    }
}

fn element(id: &str, name: &str, body: &str) -> TestResult<Element> {
    Ok(Element::new(
        id.parse()?,
        ElementFields {
            statement: None,
            type_ref: TypeRef::try_from("c4/container".to_owned())?,
            name: name.to_owned(),
            summary: None,
            body: body.to_owned(),
            relations: Relations::new(),
            anchor: None,
            verified: None,
        },
    )?)
}

fn snapshot(elements: Vec<Element>) -> Snapshot {
    Snapshot {
        elements: elements
            .into_iter()
            .map(|element| (element.id().clone(), element))
            .collect(),
        boards: BTreeMap::new(),
    }
}

/// Three elements with distinct words.
fn three() -> TestResult<Snapshot> {
    Ok(snapshot(vec![
        element("payment-service", "Payment Service", "stripe charges cards")?,
        element("orders-db", "Orders DB", "postgres tables")?,
        element("checkout", "Checkout", "shopping cart")?,
    ]))
}

fn id(text: &str) -> TestResult<ElementId> {
    Ok(text.parse()?)
}

fn cache(directory: &Path) -> TestResult<UserCache> {
    UserCache::from_env(Some(directory.as_os_str().to_owned()), None)
        .ok_or_else(|| "no cache".into())
}

/// A temporary cache and project root.
fn setup() -> TestResult<(TempDir, PathBuf, PathBuf)> {
    let directory = tempfile::tempdir()?;
    let cache = directory.path().join("cache");
    let root = directory.path().join("project");
    fs::create_dir_all(&cache)?;
    fs::create_dir_all(&root)?;
    Ok((directory, cache, root))
}

/// The database directories of every project and model in the cache.
fn table_dirs(cache: &Path) -> TestResult<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for project in fs::read_dir(cache.join("abyme").join("index"))? {
        for entry in fs::read_dir(project?.path())? {
            let path = entry?.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }
    Ok(dirs)
}

/// Every row stored in the only table in the cache, as ID and text,
/// duplicates included.
async fn stored_rows(cache: &Path) -> TestResult<Vec<(String, String)>> {
    let dirs = table_dirs(cache)?;
    let [dir] = dirs.as_slice() else {
        return Err(format!("expected one table, found {dirs:?}").into());
    };
    let db = lancedb::connect(dir.to_str().ok_or("path")?)
        .execute()
        .await?;
    let table = db.open_table("elements").execute().await?;
    let batches: Vec<RecordBatch> = table
        .query()
        .select(Select::columns(&["id", "text"]))
        .execute()
        .await?
        .try_collect()
        .await?;
    let mut rows = Vec::new();
    for batch in &batches {
        let strings = |name: &str| -> TestResult<Vec<String>> {
            let column = batch.column_by_name(name).ok_or("missing column")?;
            let array = column
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or("not strings")?;
            Ok(array.iter().flatten().map(str::to_owned).collect())
        };
        rows.extend(strings("id")?.into_iter().zip(strings("text")?));
    }
    Ok(rows)
}

/// How many rows of the only table in the cache the full-text index on
/// `text` does not cover yet.
async fn unindexed_text_rows(cache: &Path) -> TestResult<usize> {
    let dirs = table_dirs(cache)?;
    let [dir] = dirs.as_slice() else {
        return Err(format!("expected one table, found {dirs:?}").into());
    };
    let db = lancedb::connect(dir.to_str().ok_or("path")?)
        .execute()
        .await?;
    let table = db.open_table("elements").execute().await?;
    let name = table
        .list_indices()
        .await?
        .into_iter()
        .find(|index| index.columns.iter().any(|column| column == "text"))
        .ok_or("no full-text index")?
        .name;
    let stats = table
        .index_stats(name)
        .await?
        .ok_or("no index statistics")?;
    Ok(stats.num_unindexed_rows)
}

/// Every ID stored in the only table in the cache, duplicates included.
async fn stored_ids(cache: &Path) -> TestResult<Vec<String>> {
    Ok(stored_rows(cache)
        .await?
        .into_iter()
        .map(|(id, _)| id)
        .collect())
}

/// The database directory of `root`'s table for the fake model.
fn fake_table_dir(cache: &Path, root: &Path) -> PathBuf {
    cache
        .join("abyme")
        .join("index")
        .join(ProjectKey::of(root).to_string())
        .join("fake-r1-r1")
}

/// Writes a table of `elements`' rows directly. `statements` holds the value
/// of each row's `statement`, by element ID; without it the table has no
/// such column, as tables did before statement kinds.
async fn write_table(
    cache: &Path,
    root: &Path,
    elements: &Snapshot,
    statements: Option<&[Option<&str>]>,
) -> TestResult {
    let dir = fake_table_dir(cache, root);
    fs::create_dir_all(&dir)?;
    let db = lancedb::connect(dir.to_str().ok_or("path")?)
        .execute()
        .await?;
    let dimension = i32::try_from(DIMENSION)?;
    let mut fields = vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
    ];
    if statements.is_some() {
        fields.push(Field::new("statement", DataType::Utf8, true));
    }
    fields.push(Field::new(
        "vector",
        DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)),
            dimension,
        ),
        false,
    ));
    let rows: Vec<(&ElementId, String)> = elements
        .elements
        .iter()
        .map(|(id, element)| (id, indexed_text(element)))
        .collect();
    let mut columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|(id, _)| id.as_str()),
        )),
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|(_, text)| text.as_str()),
        )),
    ];
    if let Some(statements) = statements {
        columns.push(Arc::new(StringArray::from(statements.to_vec())));
    }
    columns.push(Arc::new(FixedSizeListArray::from_iter_primitive::<
        Float32Type,
        _,
        _,
    >(
        rows.iter()
            .map(|(_, text)| Some(words_vector(text, DIMENSION).into_iter().map(Some))),
        dimension,
    )));
    let batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)?;
    db.create_table("elements", batch).execute().await?;
    Ok(())
}

/// Refreshes the index of `root` and searches it once.
async fn refresh_and_search(
    cache: &Path,
    root: &Path,
    snapshot: &Snapshot,
    query: &str,
) -> TestResult<Vec<ElementId>> {
    let index = Index::new(self::cache(cache)?, root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    lock.refresh(snapshot, &mut embedder).await?;
    Ok(lock
        .search(snapshot, query, None, &mut embedder, 10)
        .await?)
}

#[tokio::test]
async fn ranks_the_element_with_a_unique_word_first() -> TestResult {
    let (_directory, cache, root) = setup()?;

    let found = refresh_and_search(&cache, &root, &three()?, "postgres").await?;

    assert_eq!(found.first(), Some(&id("orders-db")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn finds_an_element_by_its_vector_without_a_shared_word() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let snapshot = snapshot(vec![
        element("near", "Near", "alpha")?,
        element("far", "Far", "beta")?,
    ]);
    let mut toward = vec![0.0; DIMENSION];
    let mut away = vec![0.0; DIMENSION];
    if let (Some(first), Some(second)) = (toward.get_mut(0), away.get_mut(1)) {
        *first = 1.0;
        *second = 1.0;
    }
    let text = |element: &Element| abyme_index::indexed_text(element);
    let mut embedder = TableEmbedder(BTreeMap::from([
        (text(&snapshot.elements[&id("near")?]), toward.clone()),
        (text(&snapshot.elements[&id("far")?]), away),
        ("zzz".to_owned(), toward),
    ]));
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;

    lock.refresh(&snapshot, &mut embedder).await?;
    let found = lock
        .search(&snapshot, "zzz", None, &mut embedder, 10)
        .await?;

    assert_eq!(found.first(), Some(&id("near")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn full_text_ranks_a_word_match_first_although_its_vector_points_away() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let snapshot = snapshot(vec![
        element("match", "Match", "postgres")?,
        element("decoy", "Decoy", "nothing alike")?,
    ]);
    let mut toward = vec![0.0; DIMENSION];
    let mut away = vec![0.0; DIMENSION];
    if let (Some(first), Some(second)) = (toward.get_mut(0), away.get_mut(1)) {
        *first = 1.0;
        *second = 1.0;
    }
    let mut embedder = TableEmbedder(BTreeMap::from([
        (indexed_text(&snapshot.elements[&id("match")?]), away),
        (
            indexed_text(&snapshot.elements[&id("decoy")?]),
            toward.clone(),
        ),
        ("postgres".to_owned(), toward),
    ]));
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;

    lock.refresh(&snapshot, &mut embedder).await?;
    let found = lock
        .search(&snapshot, "postgres", None, &mut embedder, 10)
        .await?;

    assert_eq!(found.first(), Some(&id("match")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn completes_a_full_text_index_that_a_cut_off_refresh_left_out() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let elements = three()?;
    // The state a refresh leaves when it stops after adding the rows and
    // before building the full-text index: every row stored, no index.
    let statements = vec![None; elements.elements.len()];
    write_table(&cache, &root, &elements, Some(&statements)).await?;
    let mut embedder = WordsEmbedder::default();
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;

    lock.refresh(&elements, &mut embedder).await?;
    let found = lock
        .search(&elements, "postgres", None, &mut embedder, 10)
        .await?;

    assert_eq!(embedder.embedded, 1, "only the query is embedded");
    assert_eq!(unindexed_text_rows(&cache).await?, 0);
    assert_eq!(found.first(), Some(&id("orders-db")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn embeds_only_new_and_changed_texts() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let mut embedder = WordsEmbedder::default();
    let first = three()?;

    index.lock()?.refresh(&first, &mut embedder).await?;
    index.lock()?.refresh(&first, &mut embedder).await?;
    assert_eq!(embedder.embedded, 3);

    let mut changed = first.clone();
    changed
        .elements
        .insert(id("checkout")?, element("checkout", "Checkout", "basket")?);
    index.lock()?.refresh(&changed, &mut embedder).await?;

    assert_eq!(embedder.embedded, 4);
    Ok(())
}

#[tokio::test]
async fn finds_changed_text_by_its_new_words_only() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let first = three()?;
    refresh_and_search(&cache, &root, &first, "cart").await?;
    let mut changed = first.clone();
    changed
        .elements
        .insert(id("checkout")?, element("checkout", "Checkout", "basket")?);

    let by_new = refresh_and_search(&cache, &root, &changed, "basket").await?;

    assert_eq!(by_new.first(), Some(&id("checkout")?), "{by_new:?}");
    let rows = stored_rows(&cache).await?;
    assert_eq!(rows.len(), 3);
    let checkout: Vec<&String> = rows
        .iter()
        .filter(|(id, _)| id == "checkout")
        .map(|(_, text)| text)
        .collect();
    assert_eq!(checkout.len(), 1);
    assert!(
        checkout
            .iter()
            .all(|text| text.contains("basket") && !text.contains("cart")),
        "{checkout:?}"
    );
    Ok(())
}

#[tokio::test]
async fn never_returns_an_element_that_is_gone() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let all = three()?;
    refresh_and_search(&cache, &root, &all, "postgres").await?;
    let mut fewer = all.clone();
    fewer.elements.remove(&id("orders-db")?);

    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let before_refresh = lock
        .search(&fewer, "postgres", None, &mut WordsEmbedder::default(), 10)
        .await?;
    lock.refresh(&fewer, &mut WordsEmbedder::default()).await?;
    let after_refresh = lock
        .search(&fewer, "postgres", None, &mut WordsEmbedder::default(), 10)
        .await?;

    assert!(
        !before_refresh.contains(&id("orders-db")?),
        "{before_refresh:?}"
    );
    assert!(
        !after_refresh.contains(&id("orders-db")?),
        "{after_refresh:?}"
    );
    assert_eq!(stored_ids(&cache).await?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn rebuilds_a_deleted_index() -> TestResult {
    let (_directory, cache, root) = setup()?;
    refresh_and_search(&cache, &root, &three()?, "cart").await?;
    for dir in table_dirs(&cache)? {
        fs::remove_dir_all(dir)?;
    }

    let found = refresh_and_search(&cache, &root, &three()?, "cart").await?;

    assert_eq!(found.first(), Some(&id("checkout")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn finds_an_element_added_after_the_full_text_index_was_built() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let mut growing = three()?;
    refresh_and_search(&cache, &root, &growing, "cart").await?;
    growing
        .elements
        .insert(id("mailer")?, element("mailer", "Mailer", "newsletter")?);

    let found = refresh_and_search(&cache, &root, &growing, "newsletter").await?;

    assert_eq!(found.first(), Some(&id("mailer")?), "{found:?}");
    assert_eq!(unindexed_text_rows(&cache).await?, 0);
    Ok(())
}

#[tokio::test]
async fn an_empty_snapshot_yields_no_results() -> TestResult {
    let (_directory, cache, root) = setup()?;

    let found = refresh_and_search(&cache, &root, &Snapshot::default(), "anything").await?;

    assert_eq!(found, Vec::<ElementId>::new());
    Ok(())
}

#[tokio::test]
async fn removes_thousands_of_elements_at_once() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let many = snapshot(
        (0..2000)
            .map(|number| {
                let id = format!("{number:0>59}-elem");
                element(&id, "Name", "body")
            })
            .collect::<TestResult<Vec<_>>>()?,
    );
    refresh_and_search(&cache, &root, &many, "body").await?;

    let found = refresh_and_search(&cache, &root, &Snapshot::default(), "body").await?;

    assert_eq!(found, Vec::<ElementId>::new());
    assert_eq!(stored_ids(&cache).await?.len(), 0);
    Ok(())
}

#[tokio::test]
async fn takes_hostile_element_text_as_data() -> TestResult {
    let (directory, cache, root) = setup()?;
    let hostile = snapshot(vec![
        element(
            "quote",
            "it's \"quoted\"",
            "') OR 1=1 -- ../ / \u{0}\u{1b}[31m",
        )?,
        element("plain", "Plain", "harmless words")?,
    ]);

    let found = refresh_and_search(&cache, &root, &hostile, "quoted").await?;
    refresh_and_search(&cache, &root, &hostile, "harmless").await?;

    assert_eq!(found.first(), Some(&id("quote")?), "{found:?}");
    let stored: BTreeSet<String> = stored_ids(&cache).await?.into_iter().collect();
    assert_eq!(
        stored,
        BTreeSet::from(["plain".to_owned(), "quote".to_owned()])
    );
    assert!(
        fs::read_dir(&root)?.next().is_none(),
        "the project directory changed"
    );
    let top: BTreeSet<_> = fs::read_dir(directory.path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(top, BTreeSet::from(["cache".into(), "project".into()]));
    Ok(())
}

#[tokio::test]
async fn keeps_its_files_in_a_cache_path_with_special_characters() -> TestResult {
    let directory = tempfile::tempdir()?;
    let cache = directory.path().join("c a#b?c%d");
    let root = directory.path().join("project");
    fs::create_dir_all(&cache)?;
    fs::create_dir_all(&root)?;

    let found = refresh_and_search(&cache, &root, &three()?, "cart").await?;

    assert_eq!(found.first(), Some(&id("checkout")?), "{found:?}");
    assert_eq!(table_dirs(&cache)?.len(), 1);
    let top: Vec<_> = fs::read_dir(directory.path())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(top.len(), 2, "{top:?}");
    Ok(())
}

#[tokio::test]
async fn creates_private_directories_and_lock_file() -> TestResult {
    let (_directory, cache, root) = setup()?;

    refresh_and_search(&cache, &root, &three()?, "cart").await?;

    let mode =
        |path: &Path| fs::metadata(path).map(|metadata| metadata.permissions().mode() & 0o777);
    let abyme = cache.join("abyme");
    assert_eq!(mode(&abyme)?, 0o700);
    let project = fs::read_dir(abyme.join("index"))?
        .next()
        .ok_or("no project")??
        .path();
    assert_eq!(mode(&project)?, 0o700);
    assert_eq!(mode(&project.join("lock"))?, 0o600);
    Ok(())
}

#[tokio::test]
async fn rebuilds_a_damaged_table() -> TestResult {
    let (_directory, cache, root) = setup()?;
    // A child process builds the table, so this process holds nothing of
    // it in memory and reads the damaged files.
    let status = Command::new(env::current_exe()?)
        .args(["child_refreshes", "--exact", "--test-threads=1"])
        .env(CHILD_CACHE, &cache)
        .env(CHILD_ROOT, &root)
        .status()?;
    assert!(status.success());
    for dir in table_dirs(&cache)? {
        let versions = dir.join("elements.lance").join("_versions");
        for entry in fs::read_dir(versions)? {
            fs::write(entry?.path(), b"truncated")?;
        }
    }

    let found = refresh_and_search(&cache, &root, &shared()?, "word7").await?;

    assert_eq!(found.first(), Some(&id("shared-7")?), "{found:?}");
    assert_eq!(stored_ids(&cache).await?.len(), SHARED_ELEMENTS);
    Ok(())
}

#[tokio::test]
async fn rebuilds_a_table_of_another_dimension() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let index = Index::new(self::cache(&cache)?, &root, &OTHER_DIMENSION);
    let mut small = TableEmbedder(BTreeMap::new());
    let elements = three()?;
    let lock = index.lock()?;
    let small_vectors: BTreeMap<String, Vec<f32>> = elements
        .elements
        .values()
        .map(|element| (abyme_index::indexed_text(element), vec![0.5; 8]))
        .collect();
    small.0 = small_vectors;
    lock.refresh(&elements, &mut small).await?;
    drop(lock);

    let found = refresh_and_search(&cache, &root, &elements, "cart").await?;

    assert_eq!(found.first(), Some(&id("checkout")?), "{found:?}");
    Ok(())
}

#[tokio::test]
async fn rejects_unusable_vectors_and_keeps_the_index() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let first = three()?;
    refresh_and_search(&cache, &root, &first, "cart").await?;
    let mut more = first.clone();
    more.elements
        .insert(id("mailer")?, element("mailer", "Mailer", "newsletter")?);
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);

    for mut broken in [
        BrokenEmbedder::TooFew,
        BrokenEmbedder::WrongLength,
        BrokenEmbedder::NotFinite,
    ] {
        let result = index.lock()?.refresh(&more, &mut broken).await;

        assert!(
            matches!(
                result,
                Err(IndexError::VectorCount { .. }
                    | IndexError::VectorLength { .. }
                    | IndexError::NotFinite)
            ),
            "{result:?}"
        );
        let stored: BTreeSet<String> = stored_ids(&cache).await?.into_iter().collect();
        assert_eq!(stored.len(), 3);
    }
    Ok(())
}

#[tokio::test]
async fn answers_hostile_queries_with_results_or_an_error() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let elements = three()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    lock.refresh(&elements, &mut embedder).await?;
    let long = "x".repeat(MAX_QUERY_CHARS);

    for query in [
        "\"",
        "'",
        "*",
        ":",
        "' OR 1=1 --",
        "a AND b",
        "\"phrase\" -not +must",
        "\u{0}",
        "☎️😀",
        "",
        "   ",
        "?!.,;",
        &long,
    ] {
        let result = lock.search(&elements, query, None, &mut embedder, 10).await;

        assert!(
            result.is_ok() || matches!(result, Err(IndexError::Lance(_))),
            "{query:?}: {result:?}"
        );
    }
    let too_long = "x".repeat(MAX_QUERY_CHARS + 1);
    assert!(matches!(
        lock.search(&elements, &too_long, None, &mut embedder, 10)
            .await,
        Err(IndexError::QueryTooLong { .. })
    ));
    assert_eq!(
        lock.search(&elements, "cart", None, &mut embedder, 0)
            .await?,
        Vec::new()
    );
    assert!(matches!(
        lock.search(&elements, "cart", None, &mut embedder, MAX_RESULTS + 1)
            .await,
        Err(IndexError::TooManyResults { .. })
    ));
    Ok(())
}

#[tokio::test]
async fn holds_the_lock_only_for_one_operation() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let first = Index::new(self::cache(&cache)?, &root, &SPEC);
    let second = Index::new(self::cache(&cache)?, &root, &SPEC);
    let elements = three()?;

    first
        .lock()?
        .refresh(&elements, &mut WordsEmbedder::default())
        .await?;
    second
        .lock()?
        .refresh(&elements, &mut WordsEmbedder::default())
        .await?;
    let found = first
        .lock()?
        .search(&elements, "cart", None, &mut WordsEmbedder::default(), 10)
        .await?;

    assert_eq!(found.first(), Some(&id("checkout")?), "{found:?}");
    Ok(())
}

/// Elements that both processes of the two-process test index.
fn shared() -> TestResult<Snapshot> {
    Ok(snapshot(
        (0..SHARED_ELEMENTS)
            .map(|number| {
                element(
                    &format!("shared-{number}"),
                    "Shared",
                    &format!("word{number}"),
                )
            })
            .collect::<TestResult<Vec<_>>>()?,
    ))
}

/// Not a test of its own: the body of the child processes that
/// `two_processes_store_each_element_once` starts.
#[tokio::test]
async fn child_refreshes() -> TestResult {
    let (Ok(cache), Ok(root)) = (env::var(CHILD_CACHE), env::var(CHILD_ROOT)) else {
        return Ok(());
    };
    let index = Index::new(self::cache(Path::new(&cache))?, Path::new(&root), &SPEC);
    let elements = shared()?;
    for _ in 0..3 {
        index
            .lock()?
            .refresh(&elements, &mut WordsEmbedder::default())
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn two_processes_store_each_element_once() -> TestResult {
    let (_directory, cache, root) = setup()?;
    // This process holds an index value, and its table stays open in
    // LanceDB's cache, while the children write.
    let held = Index::new(self::cache(&cache)?, &root, &SPEC);
    held.lock()?
        .refresh(&Snapshot::default(), &mut WordsEmbedder::default())
        .await?;

    let children: Vec<_> = (0..2)
        .map(|_| {
            Command::new(env::current_exe()?)
                .args(["child_refreshes", "--exact", "--test-threads=1"])
                .env(CHILD_CACHE, &cache)
                .env(CHILD_ROOT, &root)
                .spawn()
        })
        .collect::<Result<_, _>>()?;
    for mut child in children {
        assert!(child.wait()?.success());
    }
    held.lock()?
        .refresh(&shared()?, &mut WordsEmbedder::default())
        .await?;

    let stored = stored_ids(&cache).await?;
    let unique: BTreeSet<&String> = stored.iter().collect();
    assert_eq!(stored.len(), SHARED_ELEMENTS, "{stored:?}");
    assert_eq!(unique.len(), SHARED_ELEMENTS);
    Ok(())
}

/// `elements` on boards given by ID, the kind of their zone if they are one,
/// and the elements they place.
fn zoned(
    elements: Vec<Element>,
    boards: &[(&str, Option<StatementKind>, &[&str])],
) -> TestResult<Snapshot> {
    let rect = Rect {
        x: 0,
        y: 0,
        width: NonZeroU32::MIN,
        height: NonZeroU32::MIN,
    };
    let mut snapshot = snapshot(elements);
    for (board, statement, placed) in boards {
        let board = Board {
            id: board.parse()?,
            title: (*board).to_owned(),
            owner: None,
            summary: None,
            zone: statement.map(|statement| Zone {
                mode: Mode::Free,
                statement,
            }),
            placements: placed
                .iter()
                .map(|placed| {
                    Ok((
                        placed.parse()?,
                        Placement {
                            rect,
                            pinned: false,
                            attached_to: None,
                        },
                    ))
                })
                .collect::<TestResult<_>>()?,
        };
        snapshot.boards.insert(board.id.clone(), board);
    }
    Ok(snapshot)
}

/// A vector along one axis, so that vectors of other axes lie far from it.
fn along(axis: usize) -> Vec<f32> {
    let mut vector = vec![0.0; DIMENSION];
    if let Some(slot) = vector.get_mut(axis) {
        *slot = 1.0;
    }
    vector
}

/// The elements in `found`, for comparing with the expected ones.
fn set(found: &[ElementId]) -> BTreeSet<&str> {
    found.iter().map(ElementId::as_str).collect()
}

/// Six ideas that match the query `needle` best, by their word and by their
/// vector; two current elements whose bodies are `fact_body` and whose
/// vectors point away; a third current element that matches worst of all,
/// with no word of the query and a vector pointing the opposite way; and an
/// embedder that says so.
fn ideas_ahead_of_facts(fact_body: &str) -> TestResult<(Snapshot, TableEmbedder)> {
    let mut elements = Vec::new();
    for number in 0..6 {
        elements.push(element(
            &format!("idea-{number}"),
            "Idea",
            "needle needle needle",
        )?);
    }
    elements.push(element("fact-0", "Fact", fact_body)?);
    elements.push(element("fact-1", "Fact", fact_body)?);
    elements.push(element("fact-worst", "Worst", "unrelated")?);
    let ideas = ["idea-0", "idea-1", "idea-2", "idea-3", "idea-4", "idea-5"];
    let snapshot = zoned(
        elements,
        &[
            ("ideas", Some(StatementKind::Idea), &ideas),
            (
                "facts",
                Some(StatementKind::Current),
                &["fact-0", "fact-1", "fact-worst"],
            ),
        ],
    )?;
    let mut vectors = BTreeMap::from([("needle".to_owned(), along(0))]);
    for (id, element) in &snapshot.elements {
        let vector = match id.as_str() {
            "fact-worst" => along(0).iter().map(|value| -value).collect(),
            "fact-0" | "fact-1" => along(1),
            _ => along(0),
        };
        vectors.insert(indexed_text(element), vector);
    }
    Ok((snapshot, TableEmbedder(vectors)))
}

#[rstest]
#[case::found_by_full_text_too("needle among many other words that dilute it")]
#[case::found_by_vector_alone("nothing the query names")]
#[tokio::test]
async fn filters_before_ranking_so_elements_of_other_kinds_take_no_place(
    #[case] fact_body: &str,
) -> TestResult {
    let (_directory, cache, root) = setup()?;
    let (snapshot, mut embedder) = ideas_ahead_of_facts(fact_body)?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    lock.refresh(&snapshot, &mut embedder).await?;

    let unfiltered = lock
        .search(&snapshot, "needle", None, &mut embedder, 2)
        .await?;
    let filtered = lock
        .search(
            &snapshot,
            "needle",
            Some(StatementKind::Current),
            &mut embedder,
            2,
        )
        .await?;

    assert!(
        unfiltered.iter().all(|id| id.as_str().starts_with("idea")),
        "the ideas must fill the results without a filter: {unfiltered:?}"
    );
    assert_eq!(set(&filtered), BTreeSet::from(["fact-0", "fact-1"]));
    Ok(())
}

#[rstest]
#[case::a_word_elements_of_other_kinds_hold("needle")]
#[case::a_word_no_element_holds("absent")]
#[tokio::test]
async fn answers_empty_when_no_element_has_the_kind(#[case] query: &str) -> TestResult {
    let (_directory, cache, root) = setup()?;
    let (snapshot, mut embedder) = ideas_ahead_of_facts("other words")?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    lock.refresh(&snapshot, &mut embedder).await?;

    let found = lock
        .search(
            &snapshot,
            query,
            Some(StatementKind::Question),
            &mut embedder,
            3,
        )
        .await?;

    assert_eq!(found, Vec::<ElementId>::new());
    Ok(())
}

/// `thing` and `other`: `thing` lies on the zone `ideas`, `other` on no zone.
fn with_and_without_a_kind() -> TestResult<Snapshot> {
    zoned(
        vec![
            element("thing", "Thing", "shared word")?,
            element("other", "Other", "shared word")?,
        ],
        &[("ideas", Some(StatementKind::Idea), &["thing"])],
    )
}

#[tokio::test]
async fn embeds_nothing_again_when_elements_without_a_kind_did_not_change() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let snapshot = with_and_without_a_kind()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();

    lock.refresh(&snapshot, &mut embedder).await?;
    lock.refresh(&snapshot, &mut embedder).await?;

    assert_eq!(embedder.embedded, 2);
    Ok(())
}

#[tokio::test]
async fn no_filter_finds_an_element_without_a_kind() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let mixed = with_and_without_a_kind()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    lock.refresh(&mixed, &mut embedder).await?;

    for kind in StatementKind::ALL {
        let found = lock
            .search(&mixed, "shared", Some(kind), &mut embedder, 10)
            .await?;

        let expected = if kind == StatementKind::Idea {
            BTreeSet::from(["thing"])
        } else {
            BTreeSet::new()
        };
        assert_eq!(set(&found), expected, "{kind:?}");
    }
    let found = lock
        .search(&mixed, "shared", None, &mut embedder, 10)
        .await?;
    assert_eq!(set(&found), BTreeSet::from(["other", "thing"]));
    Ok(())
}

#[tokio::test]
async fn finds_nothing_under_a_filter_when_no_element_of_the_project_has_a_kind() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let elements = three()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    lock.refresh(&elements, &mut embedder).await?;

    let filtered = lock
        .search(
            &elements,
            "cart",
            Some(StatementKind::Current),
            &mut embedder,
            10,
        )
        .await?;
    let unfiltered = lock
        .search(&elements, "cart", None, &mut embedder, 10)
        .await?;

    assert_eq!(filtered, Vec::<ElementId>::new());
    assert_eq!(unfiltered.first(), Some(&id("checkout")?));
    Ok(())
}

#[tokio::test]
async fn finds_an_element_under_its_new_kind_only_after_a_change_on_a_board() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let thing = || element("thing", "Thing", "shared word");
    let states: Vec<(Snapshot, Option<StatementKind>)> = vec![
        (
            zoned(
                vec![thing()?],
                &[("board", Some(StatementKind::Idea), &["thing"])],
            )?,
            Some(StatementKind::Idea),
        ),
        // The zone of its home changes.
        (
            zoned(
                vec![thing()?],
                &[("board", Some(StatementKind::Current), &["thing"])],
            )?,
            Some(StatementKind::Current),
        ),
        // Its home moves to a board with a lower ID.
        (
            zoned(
                vec![thing()?],
                &[
                    ("alpha", Some(StatementKind::Target), &["thing"]),
                    ("board", Some(StatementKind::Current), &["thing"]),
                ],
            )?,
            Some(StatementKind::Target),
        ),
        // Its new home is no zone.
        (
            zoned(
                vec![thing()?],
                &[
                    ("alpha", None, &["thing"]),
                    ("board", Some(StatementKind::Current), &["thing"]),
                ],
            )?,
            None,
        ),
    ];
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();

    for (snapshot, expected) in &states {
        lock.refresh(snapshot, &mut embedder).await?;

        for kind in StatementKind::ALL {
            let found = lock
                .search(snapshot, "shared", Some(kind), &mut embedder, 10)
                .await?;
            let wanted = if *expected == Some(kind) {
                BTreeSet::from(["thing"])
            } else {
                BTreeSet::new()
            };
            assert_eq!(set(&found), wanted, "{expected:?}, filter {kind:?}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn takes_the_kind_from_the_snapshot_when_a_refresh_failed() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let before = zoned(
        vec![element("thing", "Thing", "shared word")?],
        &[("board", Some(StatementKind::Idea), &["thing"])],
    )?;
    let after = zoned(
        vec![element("thing", "Thing", "shared word")?],
        &[("board", Some(StatementKind::Current), &["thing"])],
    )?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    lock.refresh(&before, &mut embedder).await?;
    assert!(
        lock.refresh(&after, &mut BrokenEmbedder::TooFew)
            .await
            .is_err()
    );

    let found = lock
        .search(
            &after,
            "shared",
            Some(StatementKind::Idea),
            &mut embedder,
            10,
        )
        .await?;

    assert_eq!(found, Vec::<ElementId>::new());
    Ok(())
}

/// The columns of the only table in the cache, and each row's statement.
async fn stored_statements(
    cache: &Path,
) -> TestResult<(Vec<String>, BTreeMap<String, Option<String>>)> {
    let dirs = table_dirs(cache)?;
    let [dir] = dirs.as_slice() else {
        return Err(format!("expected one table, found {dirs:?}").into());
    };
    let db = lancedb::connect(dir.to_str().ok_or("path")?)
        .execute()
        .await?;
    let table = db.open_table("elements").execute().await?;
    let columns = table
        .schema()
        .await?
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();
    let batches: Vec<RecordBatch> = table
        .query()
        .select(Select::columns(&["id", "statement"]))
        .execute()
        .await?
        .try_collect()
        .await?;
    let mut statements = BTreeMap::new();
    for batch in &batches {
        let strings = |name: &str| -> TestResult<Vec<Option<String>>> {
            let column = batch.column_by_name(name).ok_or("missing column")?;
            let array = column
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or("not strings")?;
            Ok(array.iter().map(|value| value.map(str::to_owned)).collect())
        };
        for (id, statement) in strings("id")?.into_iter().zip(strings("statement")?) {
            statements.insert(id.ok_or("no ID")?, statement);
        }
    }
    Ok((columns, statements))
}

#[tokio::test]
async fn rebuilds_a_table_from_before_statement_kinds() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let snapshot = with_and_without_a_kind()?;
    write_table(&cache, &root, &snapshot, None).await?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();

    lock.refresh(&snapshot, &mut embedder).await?;
    let after_the_rebuild = embedder.embedded;
    lock.refresh(&snapshot, &mut embedder).await?;
    let found = lock
        .search(
            &snapshot,
            "shared",
            Some(StatementKind::Idea),
            &mut embedder,
            10,
        )
        .await?;

    assert_eq!(after_the_rebuild, 2, "every element is embedded anew");
    assert_eq!(embedder.embedded, 3, "then only the query");
    assert_eq!(set(&found), BTreeSet::from(["thing"]));
    let (columns, _) = stored_statements(&cache).await?;
    assert_eq!(columns, ["id", "text", "statement", "vector"]);
    let mode = fs::metadata(fake_table_dir(&cache, &root))?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
    Ok(())
}

#[tokio::test]
async fn replaces_stored_values_that_are_no_kind() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let snapshot = zoned(
        vec![
            element("empty-value", "Empty", "shared word")?,
            element("unknown-value", "Unknown", "shared word")?,
        ],
        &[("facts", Some(StatementKind::Current), &["empty-value"])],
    )?;
    // By element ID: `empty-value` holds "", `unknown-value` holds "fact".
    write_table(&cache, &root, &snapshot, Some(&[Some(""), Some("fact")])).await?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();

    lock.refresh(&snapshot, &mut embedder).await?;
    let found = lock
        .search(
            &snapshot,
            "shared",
            Some(StatementKind::Current),
            &mut embedder,
            10,
        )
        .await?;

    assert_eq!(embedder.embedded, 3, "both rows, then the query");
    assert_eq!(set(&found), BTreeSet::from(["empty-value"]));
    let (_, statements) = stored_statements(&cache).await?;
    assert_eq!(
        statements,
        BTreeMap::from([
            ("empty-value".to_owned(), Some("current".to_owned())),
            ("unknown-value".to_owned(), None),
        ])
    );
    Ok(())
}

/// `element(id, name, body)` holding `kind` itself, if one is given.
fn holding(id: &str, kind: Option<StatementKind>) -> TestResult<Element> {
    let mut fields = element(id, "Thing", "shared word")?.into_fields();
    fields.statement = kind;
    Ok(Element::new(id.parse()?, fields)?)
}

/// The IDs a filtered search finds for "shared", after a refresh saw `snapshot`.
async fn found_as(
    lock: &abyme_index::IndexLock<'_>,
    snapshot: &Snapshot,
    kind: StatementKind,
) -> TestResult<Vec<ElementId>> {
    let mut embedder = WordsEmbedder::default();
    lock.refresh(snapshot, &mut embedder).await?;
    Ok(lock
        .search(snapshot, "shared", Some(kind), &mut embedder, 10)
        .await?)
}

#[tokio::test]
async fn filters_by_an_own_kind_that_is_set_changed_and_removed_without_a_board_change()
-> TestResult {
    let (_directory, cache, root) = setup()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let thing: ElementId = "thing".parse()?;
    let on_kind = |kind| -> TestResult<Snapshot> { Ok(snapshot(vec![holding("thing", kind)?])) };

    let none = on_kind(None)?;
    assert_eq!(found_as(&lock, &none, StatementKind::Idea).await?, []);

    let idea = on_kind(Some(StatementKind::Idea))?;
    assert_eq!(
        found_as(&lock, &idea, StatementKind::Idea).await?,
        std::slice::from_ref(&thing)
    );

    let question = on_kind(Some(StatementKind::Question))?;
    assert_eq!(found_as(&lock, &question, StatementKind::Idea).await?, []);
    assert_eq!(
        found_as(&lock, &question, StatementKind::Question).await?,
        [thing]
    );

    assert_eq!(found_as(&lock, &none, StatementKind::Question).await?, []);
    Ok(())
}

#[tokio::test]
async fn takes_an_own_kind_from_the_snapshot_when_a_refresh_failed() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let mut embedder = WordsEmbedder::default();
    let idea = snapshot(vec![holding("thing", Some(StatementKind::Idea))?]);
    let current = snapshot(vec![holding("thing", Some(StatementKind::Current))?]);
    let none = snapshot(vec![holding("thing", None)?]);
    lock.refresh(&idea, &mut embedder).await?;
    assert!(
        lock.refresh(&current, &mut BrokenEmbedder::TooFew)
            .await
            .is_err()
    );

    let as_idea = lock
        .search(
            &current,
            "shared",
            Some(StatementKind::Idea),
            &mut embedder,
            10,
        )
        .await?;

    assert_eq!(as_idea, Vec::<ElementId>::new());

    lock.refresh(&current, &mut embedder).await?;
    assert!(
        lock.refresh(&none, &mut BrokenEmbedder::TooFew)
            .await
            .is_err()
    );

    let as_current = lock
        .search(
            &none,
            "shared",
            Some(StatementKind::Current),
            &mut embedder,
            10,
        )
        .await?;

    assert_eq!(as_current, Vec::<ElementId>::new());
    Ok(())
}

#[tokio::test]
async fn an_own_kind_on_a_zone_of_another_kind_is_found_by_the_own_kind_alone() -> TestResult {
    let (_directory, cache, root) = setup()?;
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;
    let thing: ElementId = "thing".parse()?;
    let snapshot = zoned(
        vec![holding("thing", Some(StatementKind::Idea))?],
        &[("board", Some(StatementKind::Current), &["thing"])],
    )?;

    assert_eq!(
        found_as(&lock, &snapshot, StatementKind::Current).await?,
        []
    );
    assert_eq!(
        found_as(&lock, &snapshot, StatementKind::Idea).await?,
        std::slice::from_ref(&thing)
    );
    Ok(())
}
