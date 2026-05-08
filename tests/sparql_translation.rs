use dcql_plus_to_sparql_rs::{
    ClaimFilter, ClaimQuery, CredentialFormat, CredentialQuery, ExtendedDcqlQuery, FilterOp,
    FilterValue, PathElement, SparqlTranslator,
};

fn translator() -> SparqlTranslator {
    SparqlTranslator::new()
}

fn make_query(credentials: Vec<CredentialQuery>) -> ExtendedDcqlQuery {
    ExtendedDcqlQuery {
        credentials,
        credential_sets: None,
        credential_links: None,
        aggregates: None,
    }
}

fn claim(id: &str, path: Vec<PathElement>) -> ClaimQuery {
    ClaimQuery {
        id: Some(id.to_string()),
        path,
        values: None,
        filter: None,
    }
}

fn cred(id: &str, claims: Vec<ClaimQuery>) -> CredentialQuery {
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

// ─── standard prefixes ──────────────────────────────────────────────────────

#[test]
fn emits_standard_prefixes() {
    let q = make_query(vec![cred("x", vec![claim("i", vec![PathElement::Key("issuer".into())])])]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("PREFIX cred: <https://www.w3.org/2018/credentials#>"), "{sparql}");
    assert!(sparql.contains("PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>"), "{sparql}");
    assert!(sparql.contains("PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>"), "{sparql}");
    assert!(sparql.contains("PREFIX ex: <https://example.org/vocab#>"), "{sparql}");
}

// ─── VC-level field: issuer ──────────────────────────────────────────────────

#[test]
fn vc_level_issuer_field() {
    let q = make_query(vec![cred(
        "passport",
        vec![claim("iss", vec![PathElement::Key("issuer".into())])],
    )]);
    let sparql = translator().translate(&q).unwrap();

    assert!(sparql.contains("GRAPH ?vc_passport"), "{sparql}");
    assert!(sparql.contains("?vc_passport a cred:VerifiableCredential"), "{sparql}");
    assert!(sparql.contains("?vc_passport cred:issuer ?val_passport_iss"), "{sparql}");
    assert!(sparql.contains("SELECT"), "{sparql}");
    assert!(sparql.contains("?vc_passport"), "{sparql}");
    assert!(sparql.contains("?val_passport_iss"), "{sparql}");
}

// ─── VC-level field: validFrom ───────────────────────────────────────────────

#[test]
fn vc_level_valid_from() {
    let q = make_query(vec![cred(
        "id_cred",
        vec![claim("vf", vec![PathElement::Key("validFrom".into())])],
    )]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("?vc_id_cred cred:validFrom ?val_id_cred_vf"), "{sparql}");
}

// ─── credentialSubject single field ─────────────────────────────────────────

#[test]
fn subject_single_field() {
    let q = make_query(vec![cred(
        "identity",
        vec![claim(
            "name",
            vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("name".into()),
            ],
        )],
    )]);
    let sparql = translator().translate(&q).unwrap();

    assert!(sparql.contains("?vc_identity cred:credentialSubject ?subject_identity"), "{sparql}");
    assert!(sparql.contains("?subject_identity ex:name ?val_identity_name"), "{sparql}");
}

// ─── credentialSubject nested path ──────────────────────────────────────────

#[test]
fn subject_nested_path() {
    let q = make_query(vec![cred(
        "addr",
        vec![claim(
            "street",
            vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("address".into()),
                PathElement::Key("streetAddress".into()),
            ],
        )],
    )]);
    let sparql = translator().translate(&q).unwrap();

    // intermediate variable connects address → streetAddress
    assert!(sparql.contains("ex:address"), "{sparql}");
    assert!(sparql.contains("ex:streetAddress ?val_addr_street"), "{sparql}");
}

// ─── credentialSubject wildcard path ────────────────────────────────────────

#[test]
fn subject_wildcard_path() {
    let q = make_query(vec![cred(
        "deg",
        vec![claim(
            "dtype",
            vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Wildcard,
                PathElement::Key("type".into()),
            ],
        )],
    )]);
    let sparql = translator().translate(&q).unwrap();

    // wildcard generates an ?apred_ variable
    assert!(sparql.contains("?apred_deg_"), "{sparql}");
    // final hop on the item uses ex:type
    assert!(sparql.contains("ex:type ?val_deg_dtype"), "{sparql}");
}

// ─── credentialSubject array index path ─────────────────────────────────────

#[test]
fn subject_index_path() {
    let q = make_query(vec![cred(
        "multi",
        vec![claim(
            "first_name",
            vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Index(0),
                PathElement::Key("name".into()),
            ],
        )],
    )]);
    let sparql = translator().translate(&q).unwrap();

    // index emits a comment and a wildcard-style predicate variable
    assert!(sparql.contains("# array index 0"), "{sparql}");
    assert!(sparql.contains("?val_multi_first_name"), "{sparql}");
}

// ─── values filter (standard DCQL) ──────────────────────────────────────────

#[test]
fn values_single_filter() {
    use dcql_plus_to_sparql_rs::ClaimValue;
    let q = make_query(vec![CredentialQuery {
        id: "vc".into(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(vec![ClaimQuery {
            id: Some("country".into()),
            path: vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("country".into()),
            ],
            values: Some(vec![ClaimValue::Text("DE".into())]),
            filter: None,
        }]),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("FILTER(?val_vc_country = \"DE\""), "{sparql}");
}

#[test]
fn values_multi_filter_uses_or() {
    use dcql_plus_to_sparql_rs::ClaimValue;
    let q = make_query(vec![CredentialQuery {
        id: "vc".into(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(vec![ClaimQuery {
            id: Some("country".into()),
            path: vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("country".into()),
            ],
            values: Some(vec![
                ClaimValue::Text("DE".into()),
                ClaimValue::Text("AT".into()),
            ]),
            filter: None,
        }]),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("||"), "{sparql}");
    assert!(sparql.contains("\"DE\""), "{sparql}");
    assert!(sparql.contains("\"AT\""), "{sparql}");
}

// ─── extended filter ─────────────────────────────────────────────────────────

#[test]
fn extended_filter_gt() {
    let q = make_query(vec![CredentialQuery {
        id: "age_cred".into(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(vec![ClaimQuery {
            id: Some("age".into()),
            path: vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("age".into()),
            ],
            values: None,
            filter: Some(ClaimFilter {
                op: FilterOp::Gt,
                value: FilterValue::Integer(18),
            }),
        }]),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("FILTER(?val_age_cred_age > 18)"), "{sparql}");
}

#[test]
fn extended_filter_regex() {
    let q = make_query(vec![CredentialQuery {
        id: "email_cred".into(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(vec![ClaimQuery {
            id: Some("email".into()),
            path: vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("email".into()),
            ],
            values: None,
            filter: Some(ClaimFilter {
                op: FilterOp::Regex,
                value: FilterValue::Text("@example\\.org$".into()),
            }),
        }]),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("FILTER(REGEX("), "{sparql}");
    assert!(sparql.contains("@example"), "{sparql}");
}

// ─── JSON round-trip for PathElement (null ↔ Wildcard) ──────────────────────

#[test]
fn path_element_null_roundtrip() {
    let path: Vec<PathElement> =
        serde_json::from_str(r#"["credentialSubject", null, "type"]"#).unwrap();
    assert_eq!(path[0], PathElement::Key("credentialSubject".into()));
    assert_eq!(path[1], PathElement::Wildcard);
    assert_eq!(path[2], PathElement::Key("type".into()));

    let back = serde_json::to_string(&path).unwrap();
    assert_eq!(back, r#"["credentialSubject",null,"type"]"#);
}

#[test]
fn path_element_index_roundtrip() {
    let path: Vec<PathElement> = serde_json::from_str(r#"["credentialSubject", 2, "name"]"#).unwrap();
    assert_eq!(path[1], PathElement::Index(2));
}

// ─── ExtendedDcqlQuery JSON parsing ─────────────────────────────────────────

#[test]
fn parse_extended_query_from_json() {
    let json = r#"{
      "credentials": [
        {
          "id": "id_card",
          "format": "ldp_vc",
          "claims": [
            {"id": "name", "path": ["credentialSubject", "name"]},
            {"id": "dob",  "path": ["credentialSubject", "dateOfBirth"]}
          ]
        }
      ]
    }"#;
    let q = ExtendedDcqlQuery::from_json(json).unwrap();
    assert_eq!(q.credentials.len(), 1);
    assert_eq!(q.credentials[0].id, "id_card");
    assert_eq!(q.credentials[0].claims.as_ref().unwrap().len(), 2);
}

// ─── two credentials, no links ───────────────────────────────────────────────

#[test]
fn two_credentials_emit_two_graph_blocks() {
    let q = make_query(vec![
        cred(
            "passport",
            vec![claim("iss", vec![PathElement::Key("issuer".into())])],
        ),
        cred(
            "diploma",
            vec![claim(
                "uni",
                vec![
                    PathElement::Key("credentialSubject".into()),
                    PathElement::Key("university".into()),
                ],
            )],
        ),
    ]);
    let sparql = translator().translate(&q).unwrap();
    assert!(sparql.contains("GRAPH ?vc_passport"), "{sparql}");
    assert!(sparql.contains("GRAPH ?vc_diploma"), "{sparql}");
}

// ─── empty path → error ──────────────────────────────────────────────────────

#[test]
fn empty_path_returns_error() {
    let q = make_query(vec![CredentialQuery {
        id: "bad".into(),
        format: CredentialFormat::LdpVc,
        meta: None,
        claims: Some(vec![ClaimQuery {
            id: Some("x".into()),
            path: vec![], // invalid
            values: None,
            filter: None,
        }]),
        claim_sets: None,
        trusted_authorities: None,
        multiple: None,
    }]);
    assert!(translator().translate(&q).is_err());
}

// ─── custom field mapping via TranslationOptions ─────────────────────────────

#[test]
fn custom_field_mapping() {
    use dcql_plus_to_sparql_rs::sparql::translator::TranslationOptions;
    use std::collections::HashMap;

    let mut mappings = HashMap::new();
    mappings.insert(
        "givenName".to_string(),
        "schema:givenName".to_string(),
    );
    let opts = TranslationOptions {
        default_prefix: "ex".into(),
        default_namespace: "https://example.org/vocab#".into(),
        extra_prefixes: vec![("schema".into(), "https://schema.org/".into())],
        field_mappings: mappings,
    };
    let translator = SparqlTranslator::with_options(opts);

    let q = make_query(vec![cred(
        "person",
        vec![claim(
            "gn",
            vec![
                PathElement::Key("credentialSubject".into()),
                PathElement::Key("givenName".into()),
            ],
        )],
    )]);
    let sparql = translator.translate(&q).unwrap();
    assert!(sparql.contains("schema:givenName ?val_person_gn"), "{sparql}");
    assert!(sparql.contains("PREFIX schema: <https://schema.org/>"), "{sparql}");
}
