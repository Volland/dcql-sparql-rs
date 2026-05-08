use dcql_plus_to_sparql_rs::{
    AggregateFunction, AggregateHaving, AggregateQuery, ClaimQuery, CredentialFormat,
    CredentialLink, CredentialQuery, ExtendedDcqlQuery, FilterOp, FilterValue, LinkRelation,
    PathElement, SparqlTranslator,
};

fn translator() -> SparqlTranslator {
    SparqlTranslator::new()
}

fn claim(id: &str, path: Vec<PathElement>) -> ClaimQuery {
    ClaimQuery {
        id: Some(id.to_string()),
        path,
        values: None,
        filter: None,
    }
}

fn ldp_cred(id: &str, claims: Vec<ClaimQuery>) -> CredentialQuery {
    CredentialQuery {
        id: id.to_string(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(claims),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }
}

// ─── same-subject join ───────────────────────────────────────────────────────

/// "The diploma and the passport must refer to the same person."
/// Both credentials have a credentialSubject.id field; the link equates them.
#[test]
fn same_subject_join() {
    let q = ExtendedDcqlQuery {
        credentials: vec![
            ldp_cred(
                "passport",
                vec![claim(
                    "subject_id",
                    vec![
                        PathElement::Key("credentialSubject".into()),
                        PathElement::Key("id".into()),
                    ],
                )],
            ),
            ldp_cred(
                "diploma",
                vec![claim(
                    "subject_id",
                    vec![
                        PathElement::Key("credentialSubject".into()),
                        PathElement::Key("id".into()),
                    ],
                )],
            ),
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

    let sparql = translator().translate(&q).unwrap();

    assert!(sparql.contains("GRAPH ?vc_passport"), "{sparql}");
    assert!(sparql.contains("GRAPH ?vc_diploma"), "{sparql}");
    assert!(
        sparql.contains("FILTER(?val_passport_subject_id = ?val_diploma_subject_id)"),
        "{sparql}"
    );
}

// ─── trust chain ─────────────────────────────────────────────────────────────

/// "The issuer of the employment credential must appear as the credentialSubject
/// of a root-authority credential — proving the employer is an accredited entity."
#[test]
fn trust_chain_issuer_equals_subject() {
    let q = ExtendedDcqlQuery {
        credentials: vec![
            ldp_cred(
                "root_auth",
                vec![claim(
                    "auth_subject",
                    vec![
                        PathElement::Key("credentialSubject".into()),
                        PathElement::Key("id".into()),
                    ],
                )],
            ),
            ldp_cred(
                "employment",
                vec![claim(
                    "emp_issuer",
                    vec![PathElement::Key("issuer".into())],
                )],
            ),
        ],
        credential_sets: None,
        credential_links: Some(vec![CredentialLink {
            left_credential: "employment".into(),
            left_claim: "emp_issuer".into(),
            right_credential: "root_auth".into(),
            right_claim: "auth_subject".into(),
            relation: LinkRelation::Equal,
        }]),
        aggregates: None,
    };

    let sparql = translator().translate(&q).unwrap();

    // employment issuer must equal root_auth subject
    assert!(
        sparql.contains("FILTER(?val_employment_emp_issuer = ?val_root_auth_auth_subject)"),
        "{sparql}"
    );
    // employment issuer is a VC-level field → direct triple on ?vc_employment
    assert!(sparql.contains("?vc_employment cred:issuer ?val_employment_emp_issuer"), "{sparql}");
    // root_auth subject comes from credentialSubject.id
    assert!(sparql.contains("?subject_root_auth ex:id ?val_root_auth_auth_subject"), "{sparql}");
}

// ─── cross-credential inequality ─────────────────────────────────────────────

#[test]
fn cross_credential_not_equal() {
    let q = ExtendedDcqlQuery {
        credentials: vec![
            ldp_cred("a", vec![claim("country_a", vec![PathElement::Key("credentialSubject".into()), PathElement::Key("country".into())])]),
            ldp_cred("b", vec![claim("country_b", vec![PathElement::Key("credentialSubject".into()), PathElement::Key("country".into())])]),
        ],
        credential_sets: None,
        credential_links: Some(vec![CredentialLink {
            left_credential: "a".into(),
            left_claim: "country_a".into(),
            right_credential: "b".into(),
            right_claim: "country_b".into(),
            relation: LinkRelation::NotEqual,
        }]),
        aggregates: None,
    };
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("FILTER(?val_a_country_a != ?val_b_country_b)"), "{sparql}");
}

// ─── range comparison across credentials ─────────────────────────────────────

/// "The credit limit on the premium card must be greater than the base card's limit."
#[test]
fn cross_credential_greater_than() {
    let q = ExtendedDcqlQuery {
        credentials: vec![
            ldp_cred(
                "base_card",
                vec![claim(
                    "limit",
                    vec![PathElement::Key("credentialSubject".into()), PathElement::Key("creditLimit".into())],
                )],
            ),
            ldp_cred(
                "premium_card",
                vec![claim(
                    "limit",
                    vec![PathElement::Key("credentialSubject".into()), PathElement::Key("creditLimit".into())],
                )],
            ),
        ],
        credential_sets: None,
        credential_links: Some(vec![CredentialLink {
            left_credential: "premium_card".into(),
            left_claim: "limit".into(),
            right_credential: "base_card".into(),
            right_claim: "limit".into(),
            relation: LinkRelation::GreaterThan,
        }]),
        aggregates: None,
    };
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("FILTER(?val_premium_card_limit > ?val_base_card_limit)"), "{sparql}");
}

// ─── aggregation: sum with HAVING ────────────────────────────────────────────

/// "The total credit limit across all bank account credentials must exceed 10 000."
#[test]
fn aggregate_sum_with_having() {
    let q = ExtendedDcqlQuery {
        credentials: vec![ldp_cred(
            "bank_acct",
            vec![claim(
                "balance",
                vec![PathElement::Key("credentialSubject".into()), PathElement::Key("balance".into())],
            )],
        )],
        credential_sets: None,
        credential_links: None,
        aggregates: Some(vec![AggregateQuery {
            id: "total_balance".into(),
            credential_id: "bank_acct".into(),
            claim_id: "balance".into(),
            function: AggregateFunction::Sum,
            having: Some(AggregateHaving {
                op: FilterOp::Gt,
                value: FilterValue::Integer(10_000),
            }),
        }]),
    };

    let sparql = translator().translate(&q).unwrap();

    assert!(sparql.contains("SUM(?val_bank_acct_balance)"), "{sparql}");
    assert!(sparql.contains("?agg_total_balance"), "{sparql}");
    assert!(sparql.contains("GROUP BY"), "{sparql}");
    assert!(sparql.contains("HAVING"), "{sparql}");
    assert!(sparql.contains("10000"), "{sparql}");
}

// ─── aggregation: count ───────────────────────────────────────────────────────

#[test]
fn aggregate_count() {
    let q = ExtendedDcqlQuery {
        credentials: vec![ldp_cred(
            "email_cred",
            vec![claim(
                "addr",
                vec![PathElement::Key("credentialSubject".into()), PathElement::Key("email".into())],
            )],
        )],
        credential_sets: None,
        credential_links: None,
        aggregates: Some(vec![AggregateQuery {
            id: "email_count".into(),
            credential_id: "email_cred".into(),
            claim_id: "addr".into(),
            function: AggregateFunction::Count,
            having: None,
        }]),
    };
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("COUNT(?val_email_cred_addr)"), "{sparql}");
    assert!(sparql.contains("GROUP BY"), "{sparql}");
}

// ─── validation: unknown credential id in link ───────────────────────────────

#[test]
fn validation_unknown_credential_in_link() {
    let json = r#"{
      "credentials": [
        {
          "id": "real_cred",
          "format": "ldp_vc",
          "claims": [{"id": "c1", "path": ["issuer"]}]
        }
      ],
      "credential_links": [{
        "left_credential": "real_cred",
        "left_claim": "c1",
        "right_credential": "ghost_cred",
        "right_claim": "c2"
      }]
    }"#;
    let err = ExtendedDcqlQuery::from_json(json);
    assert!(err.is_err(), "expected validation error for unknown credential id");
    let msg = format!("{}", err.unwrap_err());
    assert!(msg.contains("ghost_cred"), "{msg}");
}

// ─── validation: unknown claim id in link ────────────────────────────────────

#[test]
fn validation_unknown_claim_in_link() {
    let json = r#"{
      "credentials": [
        {
          "id": "cred_a",
          "format": "ldp_vc",
          "claims": [{"id": "real_claim", "path": ["issuer"]}]
        },
        {
          "id": "cred_b",
          "format": "ldp_vc",
          "claims": [{"id": "other_claim", "path": ["issuer"]}]
        }
      ],
      "credential_links": [{
        "left_credential": "cred_a",
        "left_claim": "real_claim",
        "right_credential": "cred_b",
        "right_claim": "no_such_claim"
      }]
    }"#;
    let err = ExtendedDcqlQuery::from_json(json);
    assert!(err.is_err(), "expected validation error for unknown claim id");
}

// ─── validation: duplicate credential id ─────────────────────────────────────

#[test]
fn validation_duplicate_credential_id() {
    let json = r#"{
      "credentials": [
        {"id": "dup", "format": "ldp_vc"},
        {"id": "dup", "format": "ldp_vc"}
      ]
    }"#;
    let err = ExtendedDcqlQuery::from_json(json);
    assert!(err.is_err());
}

// ─── JSON round-trip ─────────────────────────────────────────────────────────

#[test]
fn json_roundtrip_extended_query() {
    let q = ExtendedDcqlQuery {
        credentials: vec![
            ldp_cred("a", vec![claim("x", vec![PathElement::Key("issuer".into())])]),
            ldp_cred("b", vec![claim("y", vec![PathElement::Key("credentialSubject".into()), PathElement::Key("name".into())])]),
        ],
        credential_sets: None,
        credential_links: Some(vec![CredentialLink {
            left_credential: "a".into(),
            left_claim: "x".into(),
            right_credential: "b".into(),
            right_claim: "y".into(),
            relation: LinkRelation::Equal,
        }]),
        aggregates: None,
    };

    let json = serde_json::to_string(&q).unwrap();
    let parsed: ExtendedDcqlQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.credentials.len(), 2);
    assert_eq!(parsed.credential_links.as_ref().unwrap().len(), 1);
    assert_eq!(
        parsed.credential_links.as_ref().unwrap()[0].relation,
        LinkRelation::Equal
    );
}

// ─── full trust-chain SPARQL from JSON ───────────────────────────────────────

/// Parses a complete DCQL+ query from JSON and verifies the SPARQL output
/// has the correct structure for a trust-chain scenario.
#[test]
fn full_trust_chain_from_json() {
    let json = r#"{
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
          "left_credential": "employee_id",
          "left_claim":      "emp_issuer",
          "right_credential": "root_ca",
          "right_claim":      "ca_subject",
          "relation":         "equal"
        }
      ]
    }"#;

    let q = ExtendedDcqlQuery::from_json(json).unwrap();
    let sparql = translator().translate(&q).unwrap();

    assert!(sparql.contains("GRAPH ?vc_root_ca"), "{sparql}");
    assert!(sparql.contains("GRAPH ?vc_employee_id"), "{sparql}");
    assert!(sparql.contains("?vc_employee_id cred:issuer ?val_employee_id_emp_issuer"), "{sparql}");
    assert!(sparql.contains("FILTER(?val_employee_id_emp_issuer = ?val_root_ca_ca_subject)"), "{sparql}");
    assert!(sparql.contains("?subject_employee_id ex:name ?val_employee_id_emp_name"), "{sparql}");
}
