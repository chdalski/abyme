**Repository:** root
**Status:** InProgress
**Created:** 2026-10-22

## Goal

The Working Skeleton's search: an element that the AI writes with
`apply` over MCP is found by `search` over MCP. Search is hybrid, full
text plus vectors, over a local LanceDB index. Embeddings come from a
model that Abyme downloads and verifies itself, and each project picks
that model from a built-in set. The LanceDB build cost and the model
handling show before features depend on them. The canvas follows in
its own plan.

## Context

- The decided concept is in `docs/4 AI Interface/Search and Index.md`,
  `docs/5 Architecture/Hexagonal Architecture.md` (crates, Security
  Baseline), and `docs/5 Architecture/First Slice.md#Working Skeleton`.
  Task 2 brings these docs in line with the decisions below.
- Libraries:
  - `lancedb` 0.40.0 (Apache-2.0), which needs `protoc` at build time;
  - `fastembed` 7.2.0 (Apache-2.0), which pins `ort` =2.0.0-rc.15
    (MIT OR Apache-2.0). `ort-sys` downloads ONNX Runtime 1.31.0 (MIT)
    at build time from cdn.pyke.io, checked against a SHA-256 in its
    sources, and links it statically;
  - `ureq` 3.5.0 and `sha2` 0.11.1 (both MIT OR Apache-2.0).
- Models are the files fastembed uses for these variants, both
  384-dimensional:
  - BGE-small-en-v1.5, quantized (`BGESmallENV15Q`, MIT);
  - paraphrase-multilingual-MiniLM-L12-v2, quantized
    (`ParaphraseMLMiniLML12V2Q`, Apache-2.0).
- Risk categories:
  - a model downloaded at runtime: a new external input source, whose
    bytes native code parses;
  - native ONNX Runtime code downloaded at build time;
  - index and model files in a cache shared by every process of the
    user;
  - a new MCP input (`search`) whose text reaches a query engine;
  - two processes writing one index.

## Steps

- [x] Share the outline's element rows
- [x] Document the decided search design
- [x] Provide protoc in the devcontainer
- [x] Build the index in the user cache
- [x] Download, verify, and load the model
- [ ] Serve `search` over MCP
- [ ] Let each project choose its model

## Tasks

### Task 1: Share the outline's element rows

`search` shows its results the way `read_board` shows elements, with
the same escaping. Before search exists, the rendering of element lines
becomes a function that both use. Behavior is unchanged.

Review path: Security-Hybrid

- [x] `abyme-core` renders a given sequence of element IDs, in the
      given order, as the outline's element lines; `outline` uses that
      function.
- [x] A test shows that the given order is kept, and that an ID
      without an element file shows as `⚠ missing`.
- [x] These tests pass with their assertions unchanged:
      `crates/abyme-core/src/outline/tests.rs`,
      `crates/abyme-core/tests/outline.rs`, and
      `crates/abyme-mcp/tests/server.rs`.

### Task 2: Document the decided search design

The docs say the index lives in `.abyme/cache/`, is kept current
through a file hash, and that the model is downloaded on first use and
still open. The decisions below replace all of that.

Review path: Security-Hybrid

- [x] From `Search and Index.md`, a reader can find:
      - where search keeps its files, and how to reset them;
      - what is embedded per element, and what the index stores;
      - how a project chooses its model;
      - when the model is downloaded and how it is checked.
- [x] From `Storage in Git.md`, a reader can set the `embedding_model`
      key of the project file. The `.abyme/` tree lists nothing Abyme
      does not use.
- [x] The Security Baseline in `Hexagonal Architecture.md` says where
      search writes and what it downloads.
- [x] No page under `docs/` still says any of these:
      - that the search index lives in `.abyme/`;
      - that its freshness comes from a file hash or `source_hash`;
      - that the model is downloaded on first use;
      - that the embedding model is an open question.
      This includes the search row of `Decisions.md`.
- [x] `Glossary.md` defines embedding model and search index.

### Task 3: Provide protoc in the devcontainer

LanceDB's build scripts need `protoc`, and the devcontainer image does
not install it.

Review path: Security-Hybrid

- [x] The devcontainer image installs `protoc` from the distribution's
      packages, and after a rebuild `protoc --version` succeeds.
- [x] The root `CLAUDE.md` names `protoc` among the toolchains the
      devcontainer provides.

### Task 4: Build the index in the user cache

The new crate `abyme-index` takes a project's snapshot and an
embedder, keeps a LanceDB table in the user cache, and returns element
IDs ranked by hybrid search. The table holds each element's ID, the
text that is embedded (type, name, summary, body), and its vector.
Embedding goes through a small `Embedder` trait. A deterministic fake
lets the tests run without a model.

Review path: Security-Hybrid

- [x] For several elements, a query with a word that only one of them
      contains ranks that element first. An element that shares no
      word with the query, but whose vector is nearest to the query's,
      appears in the results. Tests show both.
- [x] Only elements whose embedded text changed are embedded again,
      and an element removed from the snapshot is never returned;
      tests show both.
- [x] After its directory is deleted, the index is rebuilt from the
      snapshot. An element added after the full-text index was built
      is found. A snapshot without elements yields no results, not an
      error. Tests show each.
- [x] Two processes that refresh the same project's index at the same
      time both complete without error, and each element is stored
      exactly once afterwards; a test with two processes shows it.
- [x] Every index file lies under the user cache's `abyme` directory,
      in a directory named from the project root's hash and the model.
      No path is built from element content.
- [x] `lancedb` is pinned exactly, every new dependency is
      MIT-compatible, and the handoff lists the licenses.
- [x] The root `CLAUDE.md` lists `abyme-index` among the components.

### Task 5: Download, verify, and load the model

For the default model, `abyme-index` downloads the files of a pinned
revision into the user cache. It checks each against a SHA-256 and a
size limit built into the source, and loads the model from the checked
bytes.

Review path: Security-Hybrid

- [x] A file whose hash or size does not match is never loaded. This
      holds for a fresh download and for a file changed in the cache
      afterwards; tests with a fake download source show both.
- [x] Leftovers of an interrupted download are removed before the next
      one, and two processes that fetch at once download each file
      once; tests show both.
- [x] `cargo tree -i hf-hub` finds nothing, and `fastembed` and `ort`
      are pinned exactly.
- [x] An ignored test downloads the real model and embeds two related
      texts and an unrelated one. The related texts lie closer, and
      the handoff reports the run.
- [x] The repository, revision, file names, and hashes are checked
      against Hugging Face, and the handoff says how.

### Task 6: Serve `search` over MCP

The MCP server gets the tool `search`. It takes a `query` and returns
up to 10 elements, best first, as outline rows from the current files.
The server gets its model through an exchangeable model loader, which
tests replace with one that returns the fake embedder. When the server
starts in a project, it prepares that project's model in the
background, and `search` waits for it when it is not ready yet.

Review path: Security-Hybrid

- [ ] `search` answers with up to 10 element lines, best first, in the
      outline's format. A reply larger than `read_board` may return is
      an error.
- [ ] A blank `query` and arguments that do not fit `search`'s input
      schema come back as tool errors. In a project without elements,
      `search` answers that no element matches. Tests show each.
- [ ] A server started where no project existed yet, or where the
      project file was rejected, loads the model at the first `search`
      once the project is valid; a test shows it.
- [ ] A failed model download is a tool error of `search`, and the
      next `search` tries again. With neither `XDG_CACHE_HOME` nor
      `HOME` set, `search` returns a tool error, and `apply` and
      `read_board` keep working. Tests show each case.
- [ ] An in-process MCP test with the fake loader:
      - `apply` creates several elements, and `search` ranks the
        intended one first;
      - preparing at start followed by a `search` loads the model
        once.
- [ ] An ignored test runs the built binary with the real model:
      - `apply` writes elements, and `search` finds one by a word it
        contains and one by meaning, without a shared word;
      - everything on standard output is a protocol message;
      - the handoff reports the run.
- [ ] The binary's tests cover `search` in two places:
      - "no tool takes a path";
      - "without a project, every tool asks for init and writes
        nothing". Without a project, `search` neither downloads a model
        nor creates anything in the user cache.
      Apart from the ignored test, they never download a model or
      touch the user's cache.
- [ ] After standard input closes, the process exits with a success
      code even while a download that never completes is running; a
      test with a blocking fake download source shows it.
- [ ] From `crates/abyme-mcp/README.md`, a user learns:
      - what `search` does;
      - that the server downloads a model when it starts in a project;
      - where that model and the index are cached.
      No sentence there still counts only two tools. The server's
      instructions and the root `CLAUDE.md` row for
      `crates/abyme-mcp` name `search`.

### Task 7: Let each project choose its model

The project file takes an optional key `embedding_model` with the
values `bge-small-en-v1.5` (default) and
`paraphrase-multilingual-minilm-l12-v2`. `search` uses the model the
project names.

Review path: Security-Hybrid

- [ ] The project file is accepted without the key and with either
      value. Another value makes every call fail with an error naming
      the project file. Tests cover each case.
- [ ] `search` uses the project's model, and a change of the key
      during a session takes effect at the next `search`; tests with
      the fake loader show both.
- [ ] The multilingual model's repository, revision, file names, and
      hashes are checked against Hugging Face, and the handoff says
      how.
- [ ] An ignored test with the real multilingual model finds an
      English element through a German query, and the handoff reports
      the run.
- [ ] From `crates/abyme-mcp/README.md`, a user learns how a project
      picks its model.

## Decisions

- **The index lives in the user cache,** under
  `$XDG_CACHE_HOME/abyme/index/` (or `$HOME/.cache/abyme/index/`).
  Each project gets a directory named from the hash of its canonical
  root, with one subdirectory per model and text recipe.
  - Rejected: `.abyme/cache/`, because a repository can commit index
    files there that LanceDB would read.
  - Consequences: a moved project rebuilds its index once, and old
    indexes stay until the user deletes them.
- **`abyme-index` owns the user-cache paths;** core's paths stay inside
  `abyme/` and `.abyme/`.
- **The index is kept current by the embedded text:** an element is
  embedded again when its text differs from the stored one. Rejected:
  a hash of each element file, because context that is not in the file
  (board path, statement kind) will be embedded later.
- **Abyme downloads the model itself,** from a pinned revision with
  hashes in the source, not through fastembed's own download. That
  download follows the latest revision, checks nothing, and lets
  `HF_HOME` and `HF_TOKEN` take effect.
- **The download starts in the background when the server starts in a
  project;** `search` waits for it. Rejected:
  - the first `search`, because a long download can outlast the
    client's request;
  - a separate command, because it is an extra step for the user.
- **ONNX Runtime comes prebuilt from ort's download,** checked against
  its SHA-256. Rejected: building it from source, a long C++ build for
  no gain in the skeleton.
- **The model is chosen per project from a built-in set.** The key
  names carry the model version. An invalid value fails every call,
  like any error in the project file. `init` keeps writing only
  `format = 1`.
- **`search` takes only `query`** and returns the 10 best elements,
  shown from the current files. It never shows text from the index;
  there are no filters and no scores yet.
- **Freshness, model loading, and rendering are separate:** the index
  returns ranked IDs, and core renders them.
- **Real-model tests are ignored by default** and run as named
  acceptance steps of Tasks 5–7.
- **Downloads use rustls,** the TLS stack without system OpenSSL.
- **No cache cleanup code;** the docs say where the cache lies.

## Non-Goals

- The canvas and its file watcher (next plan)
- Statement kinds, board paths, filters, and scores in search
- An approximate vector index; flat search suffices at this size
