# dcql-sparql

Extended DCQL (DCQL+) — a superset of the Digital Credentials Query Language — with a translator that converts queries to SPARQL 1.1.

Standard DCQL (defined in [OpenID4VP Draft 24](https://openid.net/specs/openid-4-verifiable-presentations-1_0-24.html)) lets a verifier request specific claims from specific credential formats. What it cannot express:

| Requirement | Base DCQL | DCQL+ |
|---|:---:|:---:|
| Select claims from a credential | ✓ | ✓ |
| Filter by exact scalar value | ✓ | ✓ |
| Filter by range (age ≥ 18) | ✗ | ✓ |
| Assert two credentials share a subject | ✗ | ✓ |
| Assert issuer of A is the subject of B (trust chain) | ✗ | ✓ |
| Aggregate across multiple credentials (sum of balances) | ✗ | ✓ |

DCQL+ is a strict superset: any valid DCQL query is valid DCQL+ with no changes.

---

## Contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [Core concepts](#core-concepts)
  - [Standard DCQL](#standard-dcql)
  - [DCQL+ extensions](#dcql-extensions)
- [Query reference](#query-reference)
  - [credential_links](#credential_links)
  - [claim filter](#claim-filter)
  - [aggregates](#aggregates)
- [SPARQL output](#sparql-output)
- [Use cases](#use-cases)
  - [Trust chain](#use-case-1-trust-chain)
  - [Same subject join](#use-case-2-same-subject-join)
  - [Age predicate](#use-case-3-age-predicate)
  - [Aggregate balance check](#use-case-4-aggregate-balance-check)
- [Rust API](#rust-api)
- [Namespace configuration](#namespace-configuration)
- [Triplestore setup](#triplestore-setup)
- [Limitations](#limitations)
- [Architecture](#architecture)

---

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
dcql-sparql = "0.1"
```

---

## Quick start

```rust
use dcql_sparql::{ExtendedDcqlQuery, SparqlTranslator};

let json = r#"{
  "credentials": [
    {
      "id": "identity",
      "format": "ldp_vc",
      "claims": [
        {"id": "issuer", "path": ["issuer"]},
        {"id": "name",   "path": ["credentialSubject", "name"]},
        {"id": "age",    "path": ["credentialSubject", "age"],
         "filter": {"op": "ge", "value": 18}}
      ]
    }
  ]
}"#;

let query  = ExtendedDcqlQuery::from_json(json)?;
let sparql = SparqlTranslator::new().translate(&query)?;

println!("{sparql}");
```

Output:

```sparql
PREFIX cred: <https://www.w3.org/2018/credentials#>
PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX xsd:  <http://www.w3.org/2001/XMLSchema#>
PREFIX ex:   <https://example.org/vocab#>

SELECT ?vc_identity ?val_identity_issuer ?val_identity_name ?val_identity_age
WHERE {
  GRAPH ?vc_identity {
    ?vc_identity a cred:VerifiableCredential .
    ?vc_identity cred:issuer ?val_identity_issuer .
    ?vc_identity cred:credentialSubject ?subject_identity .
    ?subject_identity ex:name ?val_identity_name .
    ?subject_identity ex:age  ?val_identity_age .
  }
  FILTER(?val_identity_age >= 18)
}
```

---

## Core concepts

### Standard DCQL

A DCQL query is a JSON object with two top-level keys:

```json
{
  "credentials": [ /* one or more CredentialQuery */ ],
  "credential_sets": [ /* optional disjunctive groups */ ]
}
```

**CredentialQuery** — selects one credential from a wallet:

| Field | Required | Description |
|---|:---:|---|
| `id` | yes | Local reference name (alphanumeric, `-`, `_`) |
| `format` | yes | `ldp_vc`, `jwt_vc_json`, `dc+sd-jwt`, `mso_mdoc` |
| `meta` | no | Format-specific constraints (alg, vct_values, doctype, …) |
| `claims` | no | List of claim requirements |
| `claim_sets` | no | Ordered alternative subsets of claims |
| `trusted_authorities` | no | Acceptable issuer trust roots |
| `multiple` | no | Allow multiple matching credentials |

**ClaimQuery** — selects one claim value:

| Field | Required | Description |
|---|:---:|---|
| `id` | conditional | Required when referenced by `claim_sets` or `credential_links` |
| `path` | yes | JSON path: `string` = key, `null` = array wildcard, `number` = index |
| `values` | no | Acceptable scalar values (equality, OR-combined) |

**Path examples:**

| Path | Selects |
|---|---|
| `["issuer"]` | The credential's issuer field |
| `["credentialSubject", "name"]` | `name` inside `credentialSubject` |
| `["credentialSubject", "address", "country"]` | Nested field |
| `["credentialSubject", null, "type"]` | `type` field of every array element |
| `["credentialSubject", 0, "name"]` | `name` in the first array element |

**CredentialSetQuery** — expresses OR-of-AND requirements across credential groups:

```json
{
  "options": [
    ["passport"],
    ["national_id", "address_proof"]
  ],
  "required": true,
  "purpose": "Prove identity and address"
}
```

This reads as: provide either a passport, or both a national ID and an address proof.

---

### DCQL+ extensions

DCQL+ adds three new top-level fields to the query object:

```json
{
  "credentials":        [ … ],          // standard DCQL
  "credential_sets":    [ … ],          // standard DCQL
  "credential_links":   [ … ],          // DCQL+ — cross-credential join conditions
  "aggregates":         [ … ]           // DCQL+ — aggregate functions
}
```

And one new field inside `ClaimQuery`:

```json
{
  "id":     "age_claim",
  "path":   ["credentialSubject", "age"],
  "filter": { "op": "ge", "value": 18 } // DCQL+ — range / regex predicate
}
```

---

## Query reference

### `credential_links`

Asserts a binary relation between claim values from two different credentials. The claims must each have an `id`.

```json
{
  "left_credential":  "cred_a",
  "left_claim":       "claim_id_in_a",
  "right_credential": "cred_b",
  "right_claim":      "claim_id_in_b",
  "relation":         "equal"
}
```

| Field | Type | Description |
|---|---|---|
| `left_credential` | string | `id` of the left CredentialQuery |
| `left_claim` | string | `id` of a ClaimQuery inside that credential |
| `right_credential` | string | `id` of the right CredentialQuery |
| `right_claim` | string | `id` of a ClaimQuery inside that credential |
| `relation` | enum | See table below |

**Supported relations:**

| Value | SPARQL | Meaning |
|---|---|---|
| `equal` (default) | `=` | Both values must be identical |
| `not_equal` | `!=` | Values must differ |
| `lt` | `<` | Left < right |
| `lte` | `<=` | Left ≤ right |
| `gt` | `>` | Left > right |
| `gte` | `>=` | Left ≥ right |

Multiple `credential_links` are combined with AND — all conditions must be satisfied.

**Validation:** `from_json` returns an error if a referenced `credential` or `claim` id does not exist.

---

### Claim `filter`

Replaces the standard `values` equality check with a richer predicate.

```json
{
  "id": "age",
  "path": ["credentialSubject", "age"],
  "filter": {
    "op": "ge",
    "value": 18
  }
}
```

| `op` | SPARQL | Notes |
|---|---|---|
| `eq` | `=` | |
| `ne` | `!=` | |
| `lt` | `<` | |
| `le` | `<=` | |
| `gt` | `>` | |
| `ge` | `>=` | |
| `regex` | `REGEX(…)` | `value` is the pattern string |
| `lang_matches` | `LANGMATCHES(LANG(…))` | `value` is the language tag |

`value` may be a string, integer, float, or boolean.

---

### `aggregates`

Aggregates a claim across all matching instances of a credential (requires `multiple: true` on the referenced credential).

```json
{
  "id":            "total_limit",
  "credential_id": "bank_cred",
  "claim_id":      "limit_claim",
  "function":      "sum",
  "having": {
    "op":    "gt",
    "value": 10000
  }
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Name for the aggregate result variable |
| `credential_id` | string | Which credential to aggregate over |
| `claim_id` | string | Which claim within that credential |
| `function` | enum | `sum`, `count`, `min`, `max`, `avg` |
| `having` | FilterExpr | Condition on the aggregate result (same `op`/`value` as claim filter) |

The translator generates a SPARQL `GROUP BY` / `HAVING` clause. The aggregate result is available as `?agg_{id}` in the output.

---

## SPARQL output

### Named-graph strategy

Each credential occupies its own named graph in the RDF dataset. The graph variable `?vc_{id}` serves double duty as both the graph name and the VC resource IRI:

```sparql
GRAPH ?vc_identity {
  ?vc_identity a cred:VerifiableCredential .
  ?vc_identity cred:issuer ?val_identity_issuer .
  ?vc_identity cred:credentialSubject ?subject_identity .
  ?subject_identity ex:name ?val_identity_name .
}
```

This isolation means cross-credential joins are expressed as `FILTER` expressions linking variables from different `GRAPH` blocks — straightforward SPARQL 1.1 with no extensions required.

### Variable naming

| Concept | Pattern | Example |
|---|---|---|
| Credential named graph / root IRI | `?vc_{id}` | `?vc_identity` |
| CredentialSubject node | `?subject_{id}` | `?subject_identity` |
| Claim value | `?val_{cred_id}_{claim_id}` | `?val_identity_name` |
| Aggregate result | `?agg_{aggregate_id}` | `?agg_total_limit` |
| Intermediate path node | `?mid_{cred_id}_{claim_idx}_{n}` | `?mid_identity_2_0` |

All `id` values are sanitized: any character that is not alphanumeric or `_` is replaced with `_`.

### VC-level field mapping

Top-level path keys that correspond to VC metadata are mapped to standard predicates:

| Path key | SPARQL predicate |
|---|---|
| `issuer` | `cred:issuer` |
| `validFrom` | `cred:validFrom` |
| `validUntil` | `cred:validUntil` |
| `type` / `@type` | `rdf:type` |
| `id` / `@id` | `BIND(?vc_{id} AS …)` |
| `holder` | `cred:holder` |
| `credentialStatus` | `cred:credentialStatus` |

All other top-level keys fall through to `credentialSubject` handling.

---

## Use cases

### Use case 1: Trust chain

**Scenario:** An employee ID credential is only acceptable if its issuer can itself be verified as an accredited organisation — i.e., the employer appears as the `credentialSubject` of a root authority credential.

```json
{
  "credentials": [
    {
      "id": "root_ca",
      "format": "ldp_vc",
      "claims": [
        {"id": "ca_subject", "path": ["credentialSubject", "id"]}
      ]
    },
    {
      "id": "employee_id",
      "format": "ldp_vc",
      "claims": [
        {"id": "emp_issuer", "path": ["issuer"]},
        {"id": "emp_name",   "path": ["credentialSubject", "name"]}
      ]
    }
  ],
  "credential_links": [
    {
      "left_credential":  "employee_id",
      "left_claim":       "emp_issuer",
      "right_credential": "root_ca",
      "right_claim":      "ca_subject",
      "relation":         "equal"
    }
  ]
}
```

Generated SPARQL:

```sparql
PREFIX cred: <https://www.w3.org/2018/credentials#>
PREFIX ex:   <https://example.org/vocab#>
...

SELECT ?vc_root_ca ?val_root_ca_ca_subject
       ?vc_employee_id ?val_employee_id_emp_issuer ?val_employee_id_emp_name
WHERE {
  GRAPH ?vc_root_ca {
    ?vc_root_ca a cred:VerifiableCredential .
    ?vc_root_ca cred:credentialSubject ?subject_root_ca .
    ?subject_root_ca ex:id ?val_root_ca_ca_subject .
  }
  GRAPH ?vc_employee_id {
    ?vc_employee_id a cred:VerifiableCredential .
    ?vc_employee_id cred:issuer ?val_employee_id_emp_issuer .
    ?vc_employee_id cred:credentialSubject ?subject_employee_id .
    ?subject_employee_id ex:name ?val_employee_id_emp_name .
  }
  FILTER(?val_employee_id_emp_issuer = ?val_root_ca_ca_subject)
}
```

Run with: `cargo run --example trust_chain`

---

### Use case 2: Same subject join

**Scenario:** A diploma and a passport are only valid together if they describe the same person (matched on their subject DID).

```json
{
  "credentials": [
    {
      "id": "passport",
      "format": "ldp_vc",
      "claims": [
        {"id": "subject_id", "path": ["credentialSubject", "id"]},
        {"id": "age", "path": ["credentialSubject", "age"],
         "filter": {"op": "ge", "value": 18}}
      ]
    },
    {
      "id": "diploma",
      "format": "ldp_vc",
      "claims": [
        {"id": "subject_id", "path": ["credentialSubject", "id"]},
        {"id": "degree",     "path": ["credentialSubject", "degree"]}
      ]
    }
  ],
  "credential_links": [
    {
      "left_credential":  "passport",
      "left_claim":       "subject_id",
      "right_credential": "diploma",
      "right_claim":      "subject_id",
      "relation":         "equal"
    }
  ]
}
```

Key SPARQL output:

```sparql
  FILTER(?val_passport_age >= 18)
  FILTER(?val_passport_subject_id = ?val_diploma_subject_id)
```

Run with: `cargo run --example cross_join`

---

### Use case 3: Age predicate

**Scenario:** Accept any credential of type `dc+sd-jwt` where the holder's age is at least 18, without disclosing the exact value.

```json
{
  "credentials": [
    {
      "id": "age_proof",
      "format": "dc+sd-jwt",
      "claims": [
        {
          "id": "age",
          "path": ["credentialSubject", "age"],
          "filter": {"op": "ge", "value": 18}
        }
      ]
    }
  ]
}
```

---

### Use case 4: Aggregate balance check

**Scenario:** Accept a set of bank account credentials if their combined balance exceeds €10 000.

```json
{
  "credentials": [
    {
      "id": "bank_account",
      "format": "jwt_vc_json",
      "multiple": true,
      "claims": [
        {"id": "balance", "path": ["credentialSubject", "balance"]}
      ]
    }
  ],
  "aggregates": [
    {
      "id":            "total_balance",
      "credential_id": "bank_account",
      "claim_id":      "balance",
      "function":      "sum",
      "having":        {"op": "gt", "value": 10000}
    }
  ]
}
```

Key SPARQL output:

```sparql
SELECT ?vc_bank_account ?val_bank_account_balance
       (SUM(?val_bank_account_balance) AS ?agg_total_balance)
WHERE {
  GRAPH ?vc_bank_account {
    …
    ?subject_bank_account ex:balance ?val_bank_account_balance .
  }
}
GROUP BY ?vc_bank_account
HAVING (?agg_total_balance > 10000)
```

---

## Rust API

### Parsing a query

```rust
use dcql_sparql::ExtendedDcqlQuery;

// From JSON string — validates structure and referential integrity
let query = ExtendedDcqlQuery::from_json(json_str)?;

// From a standard DcqlQuery (no extensions)
use dcql_sparql::DcqlQuery;
let base: DcqlQuery = serde_json::from_str(json_str)?;
let query = ExtendedDcqlQuery::from(base);
```

### Translating to SPARQL

```rust
use dcql_sparql::SparqlTranslator;

// Default options (ex: namespace)
let sparql = SparqlTranslator::new().translate(&query)?;

// Custom options
use dcql_sparql::sparql::translator::TranslationOptions;
use std::collections::HashMap;

let mut mappings = HashMap::new();
mappings.insert("givenName".into(), "schema:givenName".into());

let opts = TranslationOptions {
    default_prefix:    "ex".into(),
    default_namespace: "https://example.org/vocab#".into(),
    extra_prefixes:    vec![("schema".into(), "https://schema.org/".into())],
    field_mappings:    mappings,
};
let sparql = SparqlTranslator::with_options(opts).translate(&query)?;
```

### Building queries in Rust

```rust
use dcql_sparql::{
    ClaimFilter, ClaimQuery, CredentialFormat, CredentialLink, CredentialQuery,
    ExtendedDcqlQuery, FilterOp, FilterValue, LinkRelation, PathElement,
};

let query = ExtendedDcqlQuery {
    credentials: vec![
        CredentialQuery {
            id: "passport".into(),
            format: CredentialFormat::LdpVc,
            claims: Some(vec![
                ClaimQuery {
                    id: Some("subject_id".into()),
                    path: vec![
                        PathElement::Key("credentialSubject".into()),
                        PathElement::Key("id".into()),
                    ],
                    values: None,
                    filter: None,
                },
                ClaimQuery {
                    id: Some("age".into()),
                    path: vec![
                        PathElement::Key("credentialSubject".into()),
                        PathElement::Key("age".into()),
                    ],
                    values: None,
                    filter: Some(ClaimFilter {
                        op: FilterOp::Ge,
                        value: FilterValue::Integer(18),
                    }),
                },
            ]),
            meta: None, claim_sets: None, trusted_authorities: None, multiple: None,
        },
    ],
    credential_sets: None,
    credential_links: None,
    aggregates: None,
};
```

### Error handling

```rust
use dcql_sparql::DcqlError;

match ExtendedDcqlQuery::from_json(json) {
    Ok(q)  => { /* use q */ }
    Err(DcqlError::Parse(e))                   => eprintln!("JSON error: {e}"),
    Err(DcqlError::Validation(msg))            => eprintln!("Invalid query: {msg}"),
    Err(DcqlError::UnknownCredentialId(id))    => eprintln!("No credential with id '{id}'"),
    Err(DcqlError::UnknownClaimId(claim, cred))=> eprintln!("No claim '{claim}' in '{cred}'"),
    Err(DcqlError::EmptyPath)                  => eprintln!("Path must not be empty"),
    Err(e) => eprintln!("{e}"),
}
```

---

## Namespace configuration

Claim fields in `credentialSubject` do not have a standard namespace — they come from the credential's own JSON-LD context. The translator resolves them using a configurable field map and a default prefix.

**Default behaviour:** unknown fields → `ex:{field}` where `ex:` = `<https://example.org/vocab#>`.

**Custom schema.org mapping:**

```rust
use dcql_sparql::sparql::translator::TranslationOptions;
use std::collections::HashMap;

let mut map = HashMap::new();
map.insert("givenName".into(),  "schema:givenName".into());
map.insert("familyName".into(), "schema:familyName".into());
map.insert("birthDate".into(),  "schema:birthDate".into());

let opts = TranslationOptions {
    default_prefix:    "ex".into(),
    default_namespace: "https://example.org/vocab#".into(),
    extra_prefixes:    vec![("schema".into(), "https://schema.org/".into())],
    field_mappings:    map,
};
```

With this configuration, a path `["credentialSubject", "givenName"]` generates `?subject schema:givenName ?val` instead of `?subject ex:givenName ?val`.

---

## Triplestore setup

The generated SPARQL runs against any SPARQL 1.1 endpoint that stores credentials as named graphs. The expected storage pattern:

```turtle
# Graph <did:example:employer> holds the credential
GRAPH <did:example:employer> {
  <did:example:employer> a cred:VerifiableCredential ;
    cred:issuer <did:example:root-ca> ;
    cred:validFrom "2024-01-01T00:00:00Z"^^xsd:dateTimeStamp ;
    cred:credentialSubject <did:example:alice> .
  <did:example:alice> ex:name "Alice" ;
                      ex:age  30 .
}
```

Compatible triplestores: Apache Jena Fuseki, Oxigraph, Blazegraph, Stardog, GraphDB, Virtuoso.

**Loading a JSON-LD credential:**

1. Expand the JSON-LD document using a JSON-LD processor (e.g., [json-ld](https://crates.io/crates/json-ld) in Rust or [jsonld-java](https://github.com/jsonld-java/jsonld-java)).
2. Convert to N-Quads — each credential gets a named graph whose IRI is the credential's `id` field.
3. Load into the triplestore via its SPARQL Update `INSERT DATA` endpoint or bulk loader.

---

## Limitations

### Array index paths

DCQL path elements of type `u64` (e.g., `["credentialSubject", "degrees", 0, "name"]`) select a specific array index. SPARQL 1.1 has no native indexed array access. The translator emits a wildcard-style pattern with a comment:

```sparql
# array index 0
?subject_cred ?apred_cred_0 ?item_cred_0_0 .
?item_cred_0_0 ex:name ?val_cred_name .
```

This matches any element at that predicate, not specifically index 0. Correct index handling requires `rdf:List`-encoded arrays with property path expressions, which not all datasets use.

### Format conversion

The library translates query structure; it does not parse credentials. Credentials in `dc+sd-jwt`, `mso_mdoc`, or `jwt_vc_json` format must be deserialized and converted to RDF before the generated SPARQL can run against them.

### ZKP

Zero-knowledge proof systems are out of scope. DCQL+ assumes the verifier has full access to the credential graph.

### Ontological reasoning

Matching is syntactic. Semantic equivalences (`schema:Person` ≡ `foaf:Person`, numeric type coercion) require an external OWL reasoner to materialise inferred triples before querying.

### `credential_sets` UNION translation

DCQL's `credential_sets.options` (OR-of-AND groups) is parsed and validated but not yet translated to SPARQL `UNION` blocks. All credentials in the `credentials` array are included in the query unconditionally. `credential_sets` translation is planned for v0.2.

---

## Architecture

```
src/
├── lib.rs                    public re-exports
├── error.rs                  DcqlError enum
├── model/
│   ├── dcql.rs               standard DCQL types (CredentialQuery, ClaimQuery, …)
│   └── extended.rs           DCQL+ types (ExtendedDcqlQuery, CredentialLink, …)
├── sparql/
│   ├── builder.rs            fluent SPARQL string builder
│   └── translator.rs         core translation algorithm
└── matcher.rs                (future) in-process credential matching
```

**Translation algorithm summary** (`sparql/translator.rs`):

1. Build a `claim_var_index`: `(cred_id, claim_id) → ?val_…` for every claim in the query.
2. For each `CredentialQuery`, emit one `GRAPH ?vc_{id} { … }` block containing:
   - `?vc_{id} a cred:VerifiableCredential .`
   - One triple per VC-level claim (`issuer`, `validFrom`, …) on `?vc_{id}`
   - `?vc_{id} cred:credentialSubject ?subject_{id} .` (when any subject claims exist)
   - One chain of triples per `credentialSubject` claim, walked recursively through the path
3. Emit `FILTER(…)` for standard `values`, extended `filter`, and `credential_links`.
4. If `aggregates` are present, add `(FN(?val) AS ?agg_{id})` to `SELECT` and `GROUP BY` / `HAVING`.

---

## Contributing

The library is in early development. Contributions welcome, especially:

- `credential_sets` UNION translation
- Temporal ordering link relations (`before`, `after`)
- Credential format adapters (SD-JWT → RDF, mdoc → RDF)
- An in-process matcher that evaluates queries against JSON credential objects

Run tests:

```sh
cargo test
```

Run examples:

```sh
cargo run --example basic
cargo run --example cross_join
cargo run --example trust_chain
```
