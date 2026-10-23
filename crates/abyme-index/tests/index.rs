//! The index through its public API, with a real `LanceDB` table in a
//! temporary cache and a deterministic fake embedder instead of a model.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use abyme_core::Snapshot;
use abyme_index::{
    EmbedError, Embedder, Index, IndexError, MAX_QUERY_CHARS, MAX_RESULTS, ModelSpec, ProjectKey,
    UserCache, indexed_text,
};
use abyme_model::{Element, ElementId, TypeRef};
use futures::TryStreamExt as _;
use lancedb::arrow::arrow_array::types::Float32Type;
use lancedb::arrow::arrow_array::{Array, FixedSizeListArray, RecordBatch, StringArray};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};
use lancedb::query::{ExecutableQuery as _, QueryBase as _, Select};
use tempfile::TempDir;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The length of the fake embedder's vectors.
const DIMENSION: usize = 32;

static SPEC: ModelSpec = ModelSpec {
    name: "fake",
    revision: "r1",
    dimension: DIMENSION,
};

/// The same model name and revision with another dimension, as if the
/// stored table came from a different model build.
static OTHER_DIMENSION: ModelSpec = ModelSpec {
    name: "fake",
    revision: "r1",
    dimension: 8,
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
        TypeRef::try_from("c4/container".to_owned())?,
        name.to_owned(),
        None,
        body.to_owned(),
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
    Ok(lock.search(snapshot, query, &mut embedder, 10).await?)
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
    let found = lock.search(&snapshot, "zzz", &mut embedder, 10).await?;

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
        .search(&snapshot, "postgres", &mut embedder, 10)
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
    let dir = cache
        .join("abyme")
        .join("index")
        .join(ProjectKey::of(&root).to_string())
        .join("fake-r1-r1");
    fs::create_dir_all(&dir)?;
    let db = lancedb::connect(dir.to_str().ok_or("path")?)
        .execute()
        .await?;
    let dimension = i32::try_from(DIMENSION)?;
    let schema = Arc::new(Schema::new(vec![
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
    ]));
    let rows: Vec<(&ElementId, String)> = elements
        .elements
        .iter()
        .map(|(id, element)| (id, indexed_text(element)))
        .collect();
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|(id, _)| id.as_str()),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|(_, text)| text.as_str()),
            )),
            Arc::new(
                FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                    rows.iter()
                        .map(|(_, text)| Some(words_vector(text, DIMENSION).into_iter().map(Some))),
                    dimension,
                ),
            ),
        ],
    )?;
    db.create_table("elements", batch).execute().await?;
    let mut embedder = WordsEmbedder::default();
    let index = Index::new(self::cache(&cache)?, &root, &SPEC);
    let lock = index.lock()?;

    lock.refresh(&elements, &mut embedder).await?;
    let found = lock
        .search(&elements, "postgres", &mut embedder, 10)
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
        .search(&fewer, "postgres", &mut WordsEmbedder::default(), 10)
        .await?;
    lock.refresh(&fewer, &mut WordsEmbedder::default()).await?;
    let after_refresh = lock
        .search(&fewer, "postgres", &mut WordsEmbedder::default(), 10)
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
        let result = lock.search(&elements, query, &mut embedder, 10).await;

        assert!(
            result.is_ok() || matches!(result, Err(IndexError::Lance(_))),
            "{query:?}: {result:?}"
        );
    }
    let too_long = "x".repeat(MAX_QUERY_CHARS + 1);
    assert!(matches!(
        lock.search(&elements, &too_long, &mut embedder, 10).await,
        Err(IndexError::QueryTooLong { .. })
    ));
    assert_eq!(
        lock.search(&elements, "cart", &mut embedder, 0).await?,
        Vec::new()
    );
    assert!(matches!(
        lock.search(&elements, "cart", &mut embedder, MAX_RESULTS + 1)
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
        .search(&elements, "cart", &mut WordsEmbedder::default(), 10)
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
