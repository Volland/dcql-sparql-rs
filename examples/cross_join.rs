//! Cross-credential join: diploma and passport must refer to the same subject,
//! and the holder must be over 18.
//!
//! Run with: `cargo run --example cross_join`

use dcql_plus_to_sparql_rs::{
    ClaimFilter, ClaimQuery, CredentialFormat, CredentialLink, CredentialQuery, ExtendedDcqlQuery,
    FilterOp, FilterValue, LinkRelation, PathElement, SparqlTranslator,
};

fn main() {
    let query = ExtendedDcqlQuery {
        credentials: vec![
            CredentialQuery {
                id: "passport".into(),
                format: CredentialFormat::LdpVc,
                meta: None,
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
                claim_sets: None,
                trusted_authorities: None,
                multiple: None,
            },
            CredentialQuery {
                id: "diploma".into(),
                format: CredentialFormat::LdpVc,
                meta: None,
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
                        id: Some("degree".into()),
                        path: vec![
                            PathElement::Key("credentialSubject".into()),
                            PathElement::Key("degree".into()),
                        ],
                        values: None,
                        filter: None,
                    },
                ]),
                claim_sets: None,
                trusted_authorities: None,
                multiple: None,
            },
        ],
        credential_sets: None,
        credential_links: Some(vec![CredentialLink {
            left_credential: "passport".into(),
            left_claim: "subject_id".into(),
            right_credential: "diploma".into(),
            right_claim: "subject_id".into(),
            relation: LinkRelation::Equal,
        }]),
        aggregates: None,
    };

    let sparql = SparqlTranslator::new()
        .translate(&query)
        .expect("translation failed");

    println!("{sparql}");
}
