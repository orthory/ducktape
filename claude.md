# ducktape

a markdown-document tree with an in-memory cache and explicit-commit persistence. backing format is plain markdown files with structured `/comment`, `/task`, and `---` frontmatter sections.

## layout

```
bin / ducktape (entrypoint, clap + tokio)
        ↓
       api  (poem handlers; thin wrapper around DocumentService)
        ↓
    doctree  (Tree, Entry, PersistedTree, build_tree, drivers::*)
        ↓
   document  (Document::from_reader; the parsed-document type)
        ↓
   sections  (Section trait; comment/, frontmatter/, task/ versioned subpkgs; Parser)
       ↓ ↓
    auth   utils  (leaves: User type, time helpers)
```

doctree is the meaty crate — it holds the in-memory tree (pure data structure), the storage drivers (`drivers/{stdfs,vfs}.rs`), the build glue (`build.rs`), and the persistence wrapper (`persisted.rs`).

## key abstractions

**`Tree`** (doctree::tree) — pure in-memory data structure, `#[derive(Clone)]`. `root: Arc<Entry>` with `Arc`-shared subtrees inside `Entry::Directory(Vec<(String, Arc<Entry>)>)`, so cloning is cheap. mutations like `with_new_document(path)` return a new `Tree` (mvcc-shaped) — the caller owns the version swap.

**`Driver`** (doctree::drivers) — storage primitive trait. `load(&self, &Path)` for reads, `write(&mut self, &Path, &[u8])` for buffer-finalization writes. `&mut self` makes exclusive access type-level. impls: `Stdfs` (real fs, no atomicity yet — todo: temp+rename), `Vfs` (in-memory, used for tests).

**`PersistedTree`** (doctree::persisted) — couples a tree with a driver in working-copy / commit style:
- reads always go through tree (driver is never touched on read path)
- `create_document` mints a basename, updates the tree in-memory only, and adds the basename to a `pending: Mutex<HashSet<String>>`
- `commit()` is the explicit sync point: drains pending and writes each entry through the driver. on partial failure unprocessed entries return to pending so retry works
- concurrency: `tree: Mutex<Arc<Tree>>` (readers grab arc + drop lock), `driver: Mutex<Box<dyn Driver>>` (writes serialize)

**`Sections`** (sections::lib) — version-agnostic discriminated union (`Frontmatter`, `Comment`, `Task`) holding the `*Latest` shape of each section. parser-side dispatch lives in `try_parse_sections`, which delegates to per-section `try_parse_latest` functions. each version implements `Section::try_match`; per-section `try_parse_latest` tries versions newest-first and migrates older shapes forward to `*Latest`. add a new version by dropping `vN.rs` next to `mod.rs`, repointing `*Latest`, and prepending the new try in `try_parse_latest`.

## known stubs / gaps

- **document → bytes serialization is not implemented.** `commit` currently writes `b""` for every pending entry. when a serializer lands, that's the place to plug it in (`persisted.rs` has a comment marking the spot).
- **`Stdfs::write` is not crash-safe.** uses `std::fs::write` (open + truncate + write); a crash mid-write corrupts the file. the canonical fix is temp-file + atomic rename — not done yet.
- **the `update_document` poem handler returns hardcoded `"asdfasdf"`** — placeholder.
- **`api::services::Service::document` field is dead-code-warned** — leftover from earlier wiring; safe to ignore.

## conventions

- write tests against `Vfs` fixtures rather than the real fs. `Vfs::new()`, `vfs.write_file(path, bytes)`, then pass to `PersistedTree::open(vfs, basedir)`.
- mutations on `Tree` follow the `with_X` builder pattern (returns new `Tree`, caller swaps). don't add `&mut self` methods on `Tree`.
- exclusive access on `Driver::write` (`&mut self`) is enforced by the `Mutex` in `PersistedTree`. don't bypass with interior mutability.
- prose / commits / docstrings: lowercase casual style preferred.
- commit per logical change; don't batch multi-step work into one giant commit.

## commands

```sh
cargo check --workspace
cargo test --workspace --lib
cargo test -p doctree --lib
cargo run -- <args>   # bin/src/main.rs
```

