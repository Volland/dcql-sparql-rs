# DCQL+ SPARQL Translator — Execution Plan

## Milestone 0 — Project Scaffold

**Goal:** Establish a compilable Rust workspace with the correct directory layout.

| File | Description |
|---|---|
| `Cargo.toml` | Workspace manifest declaring the `dcql_sparql` crate, edition 2021, and all dependencies (`serde`, `serde_json`, `thiserror`). |
| `src/lib.rs` | Crate root that declares all modules and re-exports the primary public API (`translate`, `QueryBuilder`, `ExtendedDcqlQuery`). |
| `src/error.rs` | `TranslationError` enum covering all failure modes: unknown credential id, missing claim id, conflicting `values`+`filter`, unsupported path step, and SPARQL build errors. |

**Verification:** `cargo check` produces zero errors.

---

## Milestone 1 — Core Data Model

**Goal:** Implement fully deserializable Rust types for both base DCQL and DCQL+ extensions, with round-trip serde coverage.

| File | Description |
|---|---|
| `src/model/mod.rs` | Module declaration re-exporting all public model types. |
| `src/model/dcql.rs` | Base DCQL types: `DcqlQuery`, `CredentialQuery`, `ClaimQuery`, `PathStep`, `CredentialSetQuery`, `CredentialFormat`. |
| `src/model/extended.rs` | DCQL+ extension types: `ExtendedDcqlQuery`, `CredentialLink`, `LinkRelation`, `Aggregate`, `AggFunction`, `FilterExpr`, `FilterOp`. |

**Key decisions:**
- `PathStep` is an enum with `serde(untagged)` to deserialize `string | null | u64` from a JSON array.
- `ExtendedDcqlQuery` uses `#[serde(flatten)]` over `DcqlQuery` so DCQL+ queries are a strict superset.
- `FilterExpr.value` is `serde_json::Value` to handle string, integer, and boolean filter targets uniformly.
- All optional DCQL+ fields default to `None` so valid DCQL queries deserialize without error.

**Verification:** `cargo test --lib model` — unit tests confirm round-trip serde for each type.

---

## Milestone 2 — SPARQL Builder

**Goal:** Provide a thin string-based API for constructing well-formed SPARQL 1.1 query text without an external SPARQL AST library.

| File | Description |
|---|---|
| `src/sparql/mod.rs` | Module declaration; re-exports `SparqlBuilder` and `GraphBlock`. |
| `src/sparql/builder.rs` | `SparqlBuilder` struct that accumulates PREFIX declarations, SELECT variables, GRAPH blocks, FILTER clauses, UNION branches, and sub-selects, then serializes them to a `String` via a `build()` method. |

**Key builder methods:**

```rust
impl SparqlBuilder {
    pub fn new() -> Self;
    pub fn prefix(&mut self, alias: &str, iri: &str) -> &mut Self;
    pub fn select_var(&mut self, var: &str) -> &mut Self;
    pub fn graph_block(&mut self, graph_var: &str, body: &str) -> &mut Self;
    pub fn filter(&mut self, expr: &str) -> &mut Self;
    pub fn union(&mut self, branches: Vec<String>) -> &mut Self;
    pub fn sub_select(&mut self, projection: &str, body: &str, having: &str) -> &mut Self;
    pub fn build(&self) -> String;
}
```

**Verification:** Unit tests assert that `build()` output contains expected PREFIX lines, SELECT variables, and GRAPH keywords in the correct order.

---

## Milestone 3 — SPARQL Translator

**Goal:** Implement the core algorithm that converts an `ExtendedDcqlQuery` into a SPARQL query string.

| File | Description |
|---|---|
| `src/sparql/translator.rs` | `Translator` struct and `translate(query: &ExtendedDcqlQuery) -> Result<String, TranslationError>` free function; orchestrates path translation, FILTER emission, UNION construction, and aggregate sub-selects. |
| `src/namespace.rs` | `PrefixMap` struct holding alias-to-IRI mappings; `resolve_key(key: &str) -> String` that checks well-known VC predicates first, then the custom map, then falls back to `ex:{key}`. |

**Translation algorithm (sequential steps):**

1. Validate the query: all `credential_links` reference existing credential and claim IDs; no ClaimQuery has both `values` and `filter`.
2. For each `CredentialQuery`, build a GRAPH block:
   a. Bind `?vc_{id} a cred:VerifiableCredential`.
   b. For each `ClaimQuery`, walk the `path` array and emit a chain of triple patterns terminating in `?val_{cred_id}_{claim_id}`.
   c. If the ClaimQuery has `values`, emit `FILTER(?val_... IN (...))`.
   d. If the ClaimQuery has `filter`, emit `FILTER(?val_... {op} {value})`.
3. If `credential_sets` is present, wrap GRAPH blocks in UNION branches as specified.
4. For each `credential_link`, emit a top-level `FILTER(?val_left ... {op} ?val_right)`.
5. For each `aggregate`, emit a sub-select with the appropriate SPARQL aggregate function and HAVING clause.
6. Assemble SELECT variables: all `?graph_{id}` and `?vc_{id}` variables, plus aggregate result variables.
7. Serialize via `SparqlBuilder`.

**Verification:** `cargo test --lib sparql` — unit tests for each step in isolation; integration tests in `tests/` (Milestone 4).

---

## Milestone 4 — Tests

**Goal:** Achieve confidence in correctness through isolated unit tests and end-to-end integration tests against known query/output pairs.

| File | Description |
|---|---|
| `tests/path_translation.rs` | Unit tests for `PathStep` → SPARQL triple pattern conversion: one test per path step type (`Key`, `Wildcard`, `Index`), including multi-step paths and well-known key resolution. |
| `tests/full_query.rs` | Integration tests that feed complete `ExtendedDcqlQuery` JSON fixtures through `translate()` and assert that the output SPARQL contains the expected PREFIX declarations, GRAPH blocks, FILTER expressions, UNION keywords, and aggregate clauses. |

**Test fixtures** (inline JSON strings within the test file, one per use case):
- Basic single credential, single claim, no extensions
- Range filter (`ge` on age)
- Same-subject link (`equal` on `credentialSubject.id`)
- Trust chain link (issuer equals root subject)
- Aggregation (`sum` with `having`)
- `credential_sets` with two options (UNION)

**Verification:** `cargo test` — all tests pass with zero failures.

---

## Milestone 5 — Examples

**Goal:** Provide runnable, self-contained example programs that demonstrate the library from a caller's perspective.

| File | Description |
|---|---|
| `examples/basic.rs` | Constructs a minimal single-credential DCQL+ query (format, one claim, one range filter) and prints the translated SPARQL to stdout. |
| `examples/cross_join.rs` | Constructs a two-credential query with a `credential_link` asserting that both credentials share the same `credentialSubject.id`, prints the translated SPARQL. |
| `examples/trust_chain.rs` | Constructs the full trust chain query from DESIGN.md section 4.1, prints the translated SPARQL, and also demonstrates `QueryBuilder::with_prefix` to override the default `ex:` namespace. |

**Verification:** `cargo run --example basic`, `cargo run --example cross_join`, `cargo run --example trust_chain` — each exits with code 0 and prints non-empty SPARQL.

---

## Milestone 6 — Matcher

**Goal:** Implement an in-process evaluator that tests a set of credential JSON objects against an `ExtendedDcqlQuery` without a SPARQL endpoint — useful for local testing and unit test fixtures.

| File | Description |
|---|---|
| `src/matcher.rs` | `Matcher` struct with a `match_query(query: &ExtendedDcqlQuery, credentials: &[serde_json::Value]) -> MatchResult` function; evaluates path navigation, scalar equality (`values`), filter predicates, cross-credential links, and aggregate HAVING conditions directly against JSON values. |

**`MatchResult` type:**

```rust
pub struct MatchResult {
    /// Maps each credential query id to the credential JSON objects that satisfy it.
    pub matched: HashMap<String, Vec<serde_json::Value>>,
    /// True only if all required credential_sets options are satisfied.
    pub satisfied: bool,
}
```

**Algorithm:**
1. For each `CredentialQuery`, filter `credentials` to those satisfying all `ClaimQuery` conditions (path navigation + values/filter check).
2. Evaluate `credential_links`: for each matched pair (left credential instance, right credential instance), check the relation on extracted claim values.
3. Evaluate `aggregates`: collect claim values from all matched instances of the target credential, apply the aggregate function, check the HAVING condition.
4. Evaluate `credential_sets`: verify at least one option is fully matched.
5. Return `MatchResult` with the per-credential match lists and the overall `satisfied` flag.

**Key design note:** The matcher does not do RDF conversion. It navigates raw JSON using the same `PathStep` types as the translator. `null` (Wildcard) steps iterate over all array elements; `Index(n)` steps select by position. The matcher is intentionally simpler than a SPARQL endpoint — it handles no ontological inference, no blank node merging, and no named-graph isolation.

**Verification:** `cargo test --lib matcher` — tests cover: path miss (no match), scalar equality match, range filter match and reject, cross-credential link match and reject, aggregate sum above and below threshold.
