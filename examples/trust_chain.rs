//! Trust chain: the issuer of an employee ID credential must itself appear as
//! the credentialSubject of a root-authority credential — proving that the
//! employer is an accredited entity.
//!
//!   root_ca ──(credentialSubject)──► did:example:employer
//!   employee_id ──(issuer)──────────► did:example:employer  ← same IRI
//!
//! The credential_link equates employee_id.issuer = root_ca.credentialSubject.id,
//! which the translator converts to a SPARQL FILTER across the two GRAPH blocks.
//!
//! Run with: `cargo run --example trust_chain`

use dcql_plus_to_sparql_rs::{ExtendedDcqlQuery, SparqlTranslator};

fn main() {
    let json = r#"{
      "credentials": [
        {
          "id": "root_ca",
          "format": "ldp_vc",
          "claims": [
            {
              "id": "ca_subject",
              "path": ["credentialSubject", "id"]
            }
          ]
        },
        {
          "id": "employee_id",
          "format": "ldp_vc",
          "claims": [
            {
              "id": "emp_issuer",
              "path": ["issuer"]
            },
            {
              "id": "emp_name",
              "path": ["credentialSubject", "name"]
            },
            {
              "id": "emp_role",
              "path": ["credentialSubject", "jobTitle"]
            }
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
    }"#;

    let query = ExtendedDcqlQuery::from_json(json).expect("invalid query");
    let sparql = SparqlTranslator::new()
        .translate(&query)
        .expect("translation failed");

    println!("# Trust-chain DCQL+ → SPARQL");
    println!("#");
    println!("# This query matches two credentials from a wallet and asserts that");
    println!("# the issuer of the employee ID is the subject of a root authority VC.");
    println!("#");
    println!("{sparql}");

    // Demonstrate: the generated FILTER ties the two GRAPH blocks together.
    assert!(sparql.contains("FILTER(?val_employee_id_emp_issuer = ?val_root_ca_ca_subject)"),
        "expected cross-credential FILTER in output");

    println!("# Assertion passed: cross-credential FILTER is present.");
}
