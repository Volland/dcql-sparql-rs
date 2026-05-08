# DCQL+ Design Document

## 1. Executive Summary

**Extended DCQL (DCQL+)** is a superset of the Digital Credentials Query Language defined in OpenID4VP Draft 24. It adds cross-credential join conditions, range predicates on claims, aggregation across credential sets, and trust chain queries — none of which are expressible in base DCQL.

The core idea is to translate a DCQL+ query into a SPARQL query that runs against an RDF dataset where each Verifiable Credential occupies its own named graph. This gives verifiers a declarative, composable language for expressing complex presentation requirements while keeping the credential storage format standard.

DCQL+ does not replace DCQL. Every valid DCQL query is a valid DCQL+ query. The extensions activate only when the additional fields are present.

---

## 2. Standard DCQL Overview

DCQL is defined as part of OpenID4VP Draft 24. A query is a JSON object with two top-level keys:

```
{
  "credentials": CredentialQuery[],
  "credential_sets": CredentialSetQuery[]  // optional
}
```

**CredentialQuery** selects a single credential:

```
{
  "id": "string",              // local reference name
  "format": "dc+sd-jwt | jwt_vc_json | ldp_vc | mso_mdoc",
  "meta": { ... },            // format-specific metadata
  "claims": ClaimQuery[],     // optional list of claim requirements
  "claim_sets": [...],        // optional alternatives for claim lists
  "trusted_authorities": [...],
  "multiple": bool            // allow multiple matching credentials
}
```

**ClaimQuery** selects a single claim value:

```
{
  "id": "string",             // optional local reference name
  "path": (string | null | u64)[],  // JSON pointer: key | wildcard | index
  "values": (string | i64 | bool)[] // required scalar values (equality only)
}
```

**ClaimQuery path semantics:**
- `string` — object key
- `null` — array wildcard (match any element)
- `u64` — array index (zero-based)

**CredentialSetQuery** expresses disjunctive requirements across credential groups:

```
{
  "options": string[][],  // each inner array is a set of credential IDs
  "required": bool,       // default true
  "purpose": "string"
}
```

### Gaps in Base DCQL

| Requirement | Expressible in DCQL? |
|---|---|
| Subject binding across two credentials | No |
| Range predicate on a claim (age >= 18) | No |
| Trust chain (issuer of A is subject of B) | No |
| Aggregate across credentials (sum of balances) | No |
| Temporal ordering between credentials | No |
| Ontological reasoning | No |

---

## 3. DCQL+ Extensions

DCQL+ adds three new top-level fields to the query object and one new field inside ClaimQuery.

### 3.1 Extended Query Structure

```json
{
  "credentials": [ ... ],
  "credential_sets": [ ... ],
  "credential_links": [ ... ],
  "aggregates": [ ... ]
}
```

### 3.2 credential_links — Cross-Credential Join Conditions

`credential_links` is an array of binary join conditions between claims in different credentials. Each element:

```json
{
  "left_credential": "cred_a",
  "left_claim": "claim_id_in_a",
  "right_credential": "cred_b",
  "right_claim": "claim_id_in_b",
  "relation": "equal"
}
```

**Fields:**

| Field | Type | Description |
|---|---|---|
| `left_credential` | string | `id` of the left CredentialQuery |
| `left_claim` | string | `id` of a ClaimQuery inside the left credential |
| `right_credential` | string | `id` of the right CredentialQuery |
| `right_claim` | string | `id` of a ClaimQuery inside the right credential |
| `relation` | enum | `equal`, `not_equal`, `lt`, `lte`, `gt`, `gte` |

**Constraint:** Both referenced ClaimQuery entries must have an `id` field. Both credentials must appear in the `credentials` array. Self-links (same credential on both sides) are rejected at parse time.

### 3.3 Extended ClaimQuery filter

Standard DCQL's `values` field only checks scalar equality. DCQL+ adds an optional `filter` field to ClaimQuery for richer predicates:

```json
{
  "id": "age_claim",
  "path": ["credentialSubject", "age"],
  "filter": {
    "op": "gte",
    "value": 18
  }
}
```

**Supported operators:**

| op | SPARQL equivalent |
|---|---|
| `eq` | `=` |
| `ne` | `!=` |
| `lt` | `<` |
| `le` | `<=` |
| `gt` | `>` |
| `ge` | `>=` |
| `regex` | `regex(?val, "pattern")` |

`filter` and `values` are mutually exclusive within a single ClaimQuery. If both are present, the translator returns an error.

### 3.4 aggregates — Aggregate Functions Across Credentials

`aggregates` expresses requirements over numeric aggregations of claims across one or more credentials:

```json
{
  "id": "total_balance",
  "credential_id": "bank_cred",
  "claim_id": "balance_claim",
  "function": "sum",
  "having": {
    "op": "gt",
    "value": 10000
  }
}
```

**Fields:**

| Field | Type | Description |
|---|---|---|
| `id` | string | Local name for this aggregate |
| `credential_id` | string | Which credential's claim to aggregate over |
| `claim_id` | string | Which claim within that credential (must have `id`) |
| `function` | enum | `sum`, `count`, `min`, `max`, `avg` |
| `having` | FilterExpr | Condition on the aggregate result |

When `multiple: true` is set on the referenced credential, the aggregate collects values across all matching credential instances.

---

## 4. Use Case Examples

### 4.1 Trust Chain — Issuer of Identity Credential is a Root Authority

Require that the entity that issued the identity credential appears as a `credentialSubject` in a separately presented root authority credential.

```json
{
  "credentials": [
    {
      "id": "identity",
      "format": "ldp_vc",
      "claims": [
        { "id": "issuer_claim", "path": ["issuer"] }
      ]
    },
    {
      "id": "root_authority",
      "format": "ldp_vc",
      "claims": [
        { "id": "root_subject_claim", "path": ["credentialSubject", "id"] }
      ]
    }
  ],
  "credential_links": [
    {
      "left_credential": "identity",
      "left_claim": "issuer_claim",
      "right_credential": "root_authority",
      "right_claim": "root_subject_claim",
      "relation": "equal"
    }
  ]
}
```

### 4.2 Same Subject — Diploma and Passport Belong to the Same Person

```json
{
  "credentials": [
    {
      "id": "diploma",
      "format": "jwt_vc_json",
      "claims": [
        { "id": "diploma_subject", "path": ["credentialSubject", "id"] }
      ]
    },
    {
      "id": "passport",
      "format": "jwt_vc_json",
      "claims": [
        { "id": "passport_subject", "path": ["credentialSubject", "id"] }
      ]
    }
  ],
  "credential_links": [
    {
      "left_credential": "diploma",
      "left_claim": "diploma_subject",
      "right_credential": "passport",
      "right_claim": "passport_subject",
      "relation": "equal"
    }
  ]
}
```

### 4.3 Derived Predicate — Age Must Be At Least 18

```json
{
  "credentials": [
    {
      "id": "id_card",
      "format": "dc+sd-jwt",
      "claims": [
        {
          "id": "age_claim",
          "path": ["credentialSubject", "age"],
          "filter": { "op": "ge", "value": 18 }
        }
      ]
    }
  ]
}
```

### 4.4 Aggregation — Total Credit Limit Across All Bank Credentials Exceeds 10000

```json
{
  "credentials": [
    {
      "id": "bank_cred",
      "format": "jwt_vc_json",
      "multiple": true,
      "claims": [
        { "id": "limit_claim", "path": ["credentialSubject", "creditLimit"] }
      ]
    }
  ],
  "aggregates": [
    {
      "id": "total_limit",
      "credential_id": "bank_cred",
      "claim_id": "limit_claim",
      "function": "sum",
      "having": { "op": "gt", "value": 10000 }
    }
  ]
}
```

---

## 5. SPARQL Translation Architecture

### 5.1 Named-Graph-Per-Credential Strategy

Each Verifiable Credential is stored as a named graph in the RDF dataset. The graph name is the credential's `id` URI (or a blank node for credentials without stable identifiers). All triples for a single credential live inside its named graph.

```sparql
GRAPH ?graph_identity {
  ?vc_identity a cred:VerifiableCredential .
  ?vc_identity cred:issuer ?subject_identity_issuer_claim .
  ...
}
```

This isolation allows cross-credential joins via shared SPARQL variables while preventing accidental triple leakage between credentials.

### 5.2 Variable Naming Convention

| Concept | Variable pattern | Example |
|---|---|---|
| Credential root subject | `?vc_{id}` | `?vc_identity` |
| Named graph for credential | `?graph_{id}` | `?graph_identity` |
| Claim value | `?val_{cred_id}_{claim_id}` | `?val_identity_issuer_claim` |
| Credential subject node | `?subject_{id}` | `?subject_identity` |

All variable names are sanitized: non-alphanumeric characters in `id` strings are replaced with `_`.

### 5.3 Path Translation Rules

A DCQL path is a JSON array. The translator walks the path array left to right, emitting a chain of triple patterns.

| Path step type | SPARQL pattern emitted |
|---|---|
| First string key (well-known) | `?vc_{id} cred:{key} ?node_1 .` |
| First string key (custom) | `?vc_{id} ex:{key} ?node_1 .` |
| Subsequent string key | `?node_N ex:{key} ?node_N1 .` |
| `null` (array wildcard) | `?node_N rdf:rest*/rdf:first ?node_N1 .` |
| `u64` array index N | `?node_N rdf:rest{N}/rdf:first ?node_N1 .` (property path) |

Well-known top-level keys and their standard predicates:

| Path key | Predicate |
|---|---|
| `issuer` | `cred:issuer` |
| `credentialSubject` | `cred:credentialSubject` |
| `validFrom` | `cred:validFrom` |
| `validUntil` | `cred:validUntil` |
| `type` | `rdf:type` |
| `id` (on subject node) | No triple; bind `?node` = subject IRI directly |

The final node in the chain is bound to the claim value variable `?val_{cred_id}_{claim_id}`.

### 5.4 Cross-Credential Links → FILTER

A `credential_link` with `relation: equal` becomes:

```sparql
FILTER(?val_identity_issuer_claim = ?val_root_authority_root_subject_claim)
```

Relation mapping:

| relation | SPARQL operator |
|---|---|
| `equal` | `=` |
| `not_equal` | `!=` |
| `lt` | `<` |
| `lte` | `<=` |
| `gt` | `>` |
| `gte` | `>=` |

All FILTER clauses are placed in the outermost SELECT WHERE block, after all GRAPH blocks.

### 5.5 credential_sets Options → UNION Blocks

A `credential_sets` entry with multiple options is translated to a UNION of WHERE clauses:

```sparql
{
  GRAPH ?graph_a { ... }
  GRAPH ?graph_b { ... }
} UNION {
  GRAPH ?graph_c { ... }
}
```

Each option in `options` produces one branch of the UNION. The inner credential GRAPH blocks for credentials not referenced in a given option are omitted from that branch.

### 5.6 Aggregates → GROUP BY / HAVING

An aggregate entry produces a sub-select that groups across credential instances and applies a HAVING filter:

```sparql
SELECT (SUM(?val_bank_cred_limit_claim) AS ?agg_total_limit)
WHERE {
  GRAPH ?graph_bank_cred {
    ?vc_bank_cred a cred:VerifiableCredential .
    ?vc_bank_cred cred:credentialSubject ?subject_bank_cred .
    ?subject_bank_cred ex:creditLimit ?val_bank_cred_limit_claim .
  }
}
HAVING (SUM(?val_bank_cred_limit_claim) > 10000)
```

The sub-select is wrapped in the main SELECT using a VALUES-based or sub-query join.

### 5.7 Complete Translated SPARQL — Trust Chain Example

Input DCQL+ query from section 4.1 translates to:

```sparql
PREFIX cred: <https://www.w3.org/2018/credentials#>
PREFIX ex:   <https://example.org/vocab#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?graph_identity ?vc_identity ?graph_root_authority ?vc_root_authority
WHERE {
  GRAPH ?graph_identity {
    ?vc_identity a cred:VerifiableCredential .
    ?vc_identity cred:issuer ?val_identity_issuer_claim .
  }
  GRAPH ?graph_root_authority {
    ?vc_root_authority a cred:VerifiableCredential .
    ?vc_root_authority cred:credentialSubject ?subject_root_authority .
    ?subject_root_authority ex:id ?val_root_authority_root_subject_claim .
  }
  FILTER(?val_identity_issuer_claim = ?val_root_authority_root_subject_claim)
}
```

### 5.8 Complete Translated SPARQL — Age Predicate Example

```sparql
PREFIX cred: <https://www.w3.org/2018/credentials#>
PREFIX ex:   <https://example.org/vocab#>

SELECT ?graph_id_card ?vc_id_card
WHERE {
  GRAPH ?graph_id_card {
    ?vc_id_card a cred:VerifiableCredential .
    ?vc_id_card cred:credentialSubject ?subject_id_card .
    ?subject_id_card ex:age ?val_id_card_age_claim .
  }
  FILTER(?val_id_card_age_claim >= 18)
}
```

---

## 6. Rust Library Architecture

### 6.1 Module Structure

```
dcql_sparql/
├── src/
│   ├── lib.rs               -- public re-exports, top-level docs
│   ├── model/
│   │   ├── mod.rs
│   │   ├── dcql.rs          -- base DCQL types (CredentialQuery, ClaimQuery, etc.)
│   │   └── extended.rs      -- DCQL+ extensions (CredentialLink, Aggregate, FilterExpr)
│   ├── sparql/
│   │   ├── mod.rs
│   │   ├── builder.rs       -- string-based SPARQL query construction helpers
│   │   └── translator.rs    -- core algorithm: ExtendedDcqlQuery -> String (SPARQL)
│   ├── namespace.rs         -- prefix registry and well-known predicate resolution
│   ├── matcher.rs           -- evaluate ExtendedDcqlQuery against credential JSON objects
│   └── error.rs             -- TranslationError enum
├── examples/
│   ├── basic.rs
│   ├── cross_join.rs
│   └── trust_chain.rs
└── tests/
    ├── path_translation.rs
    └── full_query.rs
```

### 6.2 Key Public Types

```rust
// model/dcql.rs
pub struct DcqlQuery {
    pub credentials: Vec<CredentialQuery>,
    pub credential_sets: Option<Vec<CredentialSetQuery>>,
}

pub struct CredentialQuery {
    pub id: String,
    pub format: CredentialFormat,
    pub meta: Option<serde_json::Value>,
    pub claims: Option<Vec<ClaimQuery>>,
    pub claim_sets: Option<Vec<Vec<String>>>,
    pub trusted_authorities: Option<serde_json::Value>,
    pub multiple: Option<bool>,
}

pub struct ClaimQuery {
    pub id: Option<String>,
    pub path: Vec<PathStep>,
    pub values: Option<Vec<serde_json::Value>>,
    pub filter: Option<FilterExpr>,   // DCQL+ extension
}

pub enum PathStep {
    Key(String),
    Wildcard,       // null
    Index(u64),
}

// model/extended.rs
pub struct ExtendedDcqlQuery {
    #[serde(flatten)]
    pub base: DcqlQuery,
    pub credential_links: Option<Vec<CredentialLink>>,
    pub aggregates: Option<Vec<Aggregate>>,
}

pub struct CredentialLink {
    pub left_credential: String,
    pub left_claim: String,
    pub right_credential: String,
    pub right_claim: String,
    pub relation: LinkRelation,
}

pub enum LinkRelation { Equal, NotEqual, Lt, Lte, Gt, Gte }

pub struct Aggregate {
    pub id: String,
    pub credential_id: String,
    pub claim_id: String,
    pub function: AggFunction,
    pub having: FilterExpr,
}

pub enum AggFunction { Sum, Count, Min, Max, Avg }

pub struct FilterExpr {
    pub op: FilterOp,
    pub value: serde_json::Value,
}

pub enum FilterOp { Eq, Ne, Lt, Le, Gt, Ge, Regex }
```

### 6.3 Public API

```rust
// Primary entry point
pub fn translate(query: &ExtendedDcqlQuery) -> Result<String, TranslationError>;

// Builder for incremental construction
pub struct QueryBuilder {
    prefixes: PrefixMap,
}

impl QueryBuilder {
    pub fn new() -> Self;
    pub fn with_prefix(mut self, alias: &str, iri: &str) -> Self;
    pub fn build(self, query: &ExtendedDcqlQuery) -> Result<String, TranslationError>;
}

// Namespace
pub struct PrefixMap(HashMap<String, String>);

impl PrefixMap {
    pub fn default() -> Self;  // includes cred:, sec:, rdf:, ex:
    pub fn resolve_key(&self, key: &str) -> String;  // key -> "prefix:local"
}
```

---

## 7. Namespace Resolution

### 7.1 Well-Known VC Fields

The following top-level path keys are mapped to standard predicates without requiring any prefix configuration:

| Path key | Resolved predicate | Source |
|---|---|---|
| `issuer` | `cred:issuer` | W3C VC Data Model v2.0 |
| `credentialSubject` | `cred:credentialSubject` | W3C VC Data Model v2.0 |
| `validFrom` | `cred:validFrom` | W3C VC Data Model v2.0 |
| `validUntil` | `cred:validUntil` | W3C VC Data Model v2.0 |
| `type` | `rdf:type` | RDF |
| `proof` | `sec:proof` | W3C Security Vocab |

### 7.2 Custom Prefix Map for credentialSubject Fields

Keys appearing inside `credentialSubject` (i.e., after the `credentialSubject` path step) are not in the W3C standard namespace. They resolve using the configured prefix map.

Default prefix map:

```
cred: <https://www.w3.org/2018/credentials#>
sec:  <https://w3id.org/security#>
rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
xsd:  <http://www.w3.org/2001/XMLSchema#>
ex:   <https://example.org/vocab#>
```

When a key is not found in any registered prefix, it defaults to `ex:{key}`.

### 7.3 Overriding the Default Namespace

Callers may override the default prefix map via `QueryBuilder::with_prefix`:

```rust
let sparql = QueryBuilder::new()
    .with_prefix("schema", "https://schema.org/")
    .build(&query)?;
```

After this call, `schema:age` would be used instead of `ex:age` for keys that match the `schema` prefix — but only if the path key itself encodes a prefix (`schema:age`). Bare keys (e.g., `"age"`) always fall through to `ex:`.

---

## 8. Limitations and Future Work

### 8.1 Array Index Paths (u64)

DCQL allows path steps of type `u64` to index into arrays (e.g., `["credentialSubject", "degrees", 0, "name"]`). SPARQL 1.1 does not have a native indexed array access operator. The current translation approximates this with property path expressions:

```sparql
?node rdf:rest{0}/rdf:first ?next_node .
```

This only works correctly when the RDF dataset uses the standard `rdf:List` encoding for JSON arrays. Datasets that use alternative array encodings (e.g., JSON-LD `@list` with blank nodes generated differently) may not match. This is a known limitation; array index support is best-effort.

### 8.2 ZKP Integration

Zero-knowledge proof systems (BBS+, Groth16) are explicitly out of scope. DCQL+ assumes that the verifier has access to the full credential graph at query time. ZKP integration would require a separate proof-generation layer that is orthogonal to query translation.

### 8.3 Ontological Reasoning

DCQL+ FILTER and link conditions are evaluated syntactically — a value matches if the literal in the graph is equal to the query value. Semantic equivalences (e.g., `schema:Person` ≡ `foaf:Person`, or that `xsd:integer 18` satisfies `>= xsd:decimal 18.0`) are not resolved automatically. Callers that require ontological reasoning must configure an external OWL reasoner and materialize inferred triples into the dataset before running DCQL+ queries.

### 8.4 Credential Format Differences

The SPARQL translation is format-agnostic: it assumes credentials have been converted to RDF before querying. The library does not perform credential parsing or format conversion. Callers are responsible for deserializing `dc+sd-jwt`, `mso_mdoc`, and other formats into an RDF dataset prior to calling `translate`.

### 8.5 Planned Future Extensions

- **Temporal ordering links**: `relation: before | after` on `validFrom` / `validUntil` claims across credentials
- **Negation**: `credential_links` with a `required: false` flag to assert that no credential satisfying a condition exists
- **Sub-query reuse**: Named sub-patterns that can be referenced by multiple credential queries to avoid SPARQL verbosity
- **Streaming evaluation**: Incremental SPARQL evaluation as credentials arrive during a presentation exchange
