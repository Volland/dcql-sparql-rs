//! Basic single-credential query: select name and birth date from an identity credential.
//!
//! Run with: `cargo run --example basic`

use dcql_plus_to_sparql_rs::{ExtendedDcqlQuery, SparqlTranslator};

fn main() {
    let json = r#"{
      "credentials": [
        {
          "id": "identity",
          "format": "ldp_vc",
          "claims": [
            {"id": "issuer",    "path": ["issuer"]},
            {"id": "name",      "path": ["credentialSubject", "name"]},
            {"id": "birthDate", "path": ["credentialSubject", "birthDate"]},
            {"id": "country",   "path": ["credentialSubject", "address", "country"],
             "values": ["DE", "AT", "CH"]}
          ]
        }
      ]
    }"#;

    let query = ExtendedDcqlQuery::from_json(json).expect("invalid query");
    let sparql = SparqlTranslator::new()
        .translate(&query)
        .expect("translation failed");

    println!("{sparql}");
}
