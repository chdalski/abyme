# Backlog

Open findings and cleanup wishes that no plan holds yet, one
line each: what, where, and where it came from. The lead adds
entries and names them in the completion report. An entry goes
when it becomes a plan or the user drops it.

This is no status log and no home for facts. A missed plan
target goes here only after the user has lowered the target.

## Open
- `RelationKind` also reads the object form (`"kind": {"uses": null}`), the differential closed for `Mode` and `StatementKind`; give it the same string-only reader: `crates/abyme-model/src/relation.rs` (security engineer, plan D Task 1, rated Low).
- Consolidate the test files that grew across plans (`crates/abyme-core/src/apply/tests.rs`, `crates/abyme-core/src/outline/tests.rs`, `crates/abyme-mcp/tests/server.rs`) in a plan of its own before the dogfooding plan (plan reviewer, plan D; the user chose a later plan).
- A full `cargo test --workspace` once ended the test process `element_properties` with SIGKILL; it did not recur alone or in later full runs, cause unknown: `crates/abyme-model/tests/element_properties.rs` (plan D Task 4).
