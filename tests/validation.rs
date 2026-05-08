use dcql_plus_to_sparql_rs::{
    DcqlValidate, ErrorCode, ExtendedDcqlQuery, Severity,
};

// helper: parse + validate, assert valid
fn valid(json: &str) -> dcql_plus_to_sparql_rs::ValidationResult {
    let q = ExtendedDcqlQuery::from_json(json).expect("parse failed");
    let r = q.validate();
    assert!(r.is_valid(), "expected valid, got:\n{}", r);
    r
}

// helper: parse + validate, assert invalid, return errors
fn invalid(json: &str) -> Vec<dcql_plus_to_sparql_rs::ValidationError> {
    let q = ExtendedDcqlQuery::from_json(json)
        .unwrap_or_else(|_| serde_json::from_str(json).expect("raw parse failed"));
    let r = q.validate();
    assert!(!r.is_valid(), "expected invalid, but was valid");
    r.errors
}

// helper: parse raw (bypasses from_json validation) for structural tests
fn raw_parse(json: &str) -> ExtendedDcqlQuery {
    serde_json::from_str(json).expect("serde parse failed")
}

// ─── valid queries ────────────────────────────────────────────────────────────

#[test]
fn valid_minimal_query() {
    valid(r#"{
      "credentials": [
        {"id": "x", "format": "ldp_vc",
         "claims": [{"id": "n", "path": ["credentialSubject", "name"]}]}
      ]
    }"#);
}

#[test]
fn valid_with_credential_link() {
    valid(r#"{
      "credentials": [
        {"id": "a", "format": "ldp_vc",
         "claims": [{"id": "iss", "path": ["issuer"]}]},
        {"id": "b", "format": "ldp_vc",
         "claims": [{"id": "sub", "path": ["credentialSubject", "id"]}]}
      ],
      "credential_links": [
        {"left_credential": "a", "left_claim": "iss",
         "right_credential": "b", "right_claim": "sub"}
      ]
    }"#);
}

#[test]
fn valid_aggregate_with_multiple() {
    valid(r#"{
      "credentials": [
        {"id": "bank", "format": "jwt_vc_json", "multiple": true,
         "claims": [{"id": "bal", "path": ["credentialSubject", "balance"]}]}
      ],
      "aggregates": [
        {"id": "total", "credential_id": "bank", "claim_id": "bal",
         "function": "sum", "having": {"op": "gt", "value": 5000}}
      ]
    }"#);
}

#[test]
fn valid_returns_warnings_for_no_claims() {
    let r = valid(r#"{
      "credentials": [{"id": "anycred", "format": "ldp_vc"}]
    }"#);
    assert_eq!(r.warnings.len(), 1);
    assert_eq!(r.warnings[0].code, ErrorCode::NoClaimsSpecified);
    assert_eq!(r.warnings[0].severity, Severity::Warning);
}

// ─── EmptyCredentials ─────────────────────────────────────────────────────────

#[test]
fn error_empty_credentials() {
    let q = raw_parse(r#"{"credentials": []}"#);
    let r = q.validate();
    assert!(!r.is_valid());
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::EmptyCredentials));
}

// ─── InvalidCredentialId ──────────────────────────────────────────────────────

#[test]
fn error_invalid_credential_id_chars() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "bad id!", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ]
    }"#);
    let r = q.validate();
    assert!(!r.is_valid());
    let e = r.errors.iter().find(|e| e.code == ErrorCode::InvalidCredentialId)
        .expect("expected InvalidCredentialId");
    assert!(e.message.contains("bad id!"), "{}", e.message);
    assert!(e.hint.is_some());
}

#[test]
fn error_empty_credential_id() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::InvalidCredentialId));
}

// ─── DuplicateCredentialId ────────────────────────────────────────────────────

#[test]
fn error_duplicate_credential_id() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "dup", "format": "ldp_vc",
         "claims": [{"id": "a", "path": ["issuer"]}]},
        {"id": "dup", "format": "jwt_vc_json",
         "claims": [{"id": "b", "path": ["issuer"]}]}
      ]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::DuplicateCredentialId));
    let e = r.errors.iter().find(|e| e.code == ErrorCode::DuplicateCredentialId).unwrap();
    assert!(e.message.contains("dup"), "{}", e.message);
    assert!(e.location.contains("credentials[1]"), "{}", e.location);
}

// ─── EmptyClaimsArray ────────────────────────────────────────────────────────

#[test]
fn error_empty_claims_array() {
    let q = raw_parse(r#"{
      "credentials": [{"id": "c", "format": "ldp_vc", "claims": []}]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::EmptyClaimsArray));
}

// ─── EmptyPath ────────────────────────────────────────────────────────────────

#[test]
fn error_empty_path() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc",
         "claims": [{"id": "bad", "path": []}]}
      ]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::EmptyPath)
        .expect("expected EmptyPath error");
    assert!(e.location.contains("claims[0].path"), "{}", e.location);
    assert!(e.hint.is_some());
}

// ─── DuplicateClaimId ────────────────────────────────────────────────────────

#[test]
fn error_duplicate_claim_id() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc", "claims": [
          {"id": "same", "path": ["issuer"]},
          {"id": "same", "path": ["credentialSubject", "name"]}
        ]}
      ]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::DuplicateClaimId));
}

// ─── FilterAndValuesBothPresent (warning) ────────────────────────────────────

#[test]
fn warning_filter_and_values_both_present() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc", "claims": [
          {"id": "age", "path": ["credentialSubject", "age"],
           "values": [18],
           "filter": {"op": "ge", "value": 18}}
        ]}
      ]
    }"#);
    let r = q.validate();
    assert!(r.is_valid(), "should be valid (only a warning)");
    assert!(r.warnings.iter().any(|w| w.code == ErrorCode::FilterAndValuesBothPresent));
}

// ─── ClaimSetsWithoutClaims ───────────────────────────────────────────────────

#[test]
fn error_claim_sets_without_claims() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc",
         "claim_sets": [["missing_claim"]]}
      ]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::ClaimSetsWithoutClaims));
}

#[test]
fn error_claim_sets_unknown_claim_ref() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc",
         "claims": [{"id": "real", "path": ["issuer"]}],
         "claim_sets": [["real"], ["ghost"]]}
      ]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::UnknownClaimId)
        .expect("expected UnknownClaimId");
    assert!(e.message.contains("ghost"), "{}", e.message);
}

// ─── credential_sets ─────────────────────────────────────────────────────────

#[test]
fn error_credential_sets_unknown_cred_ref() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "real", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ],
      "credential_sets": [
        {"options": [["real"], ["ghost"]]}
      ]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::UnknownCredentialId)
        .expect("expected UnknownCredentialId");
    assert!(e.message.contains("ghost"), "{}", e.message);
    assert!(e.hint.as_ref().unwrap().contains("real"), "{:?}", e.hint);
}

#[test]
fn error_credential_sets_empty_options() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ],
      "credential_sets": [{"options": []}]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::EmptyOptions));
}

// ─── credential_links ────────────────────────────────────────────────────────

#[test]
fn error_link_unknown_left_credential() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "real", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ],
      "credential_links": [{
        "left_credential": "ghost", "left_claim": "x",
        "right_credential": "real", "right_claim": "x"
      }]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::UnknownCredentialId)
        .expect("expected UnknownCredentialId");
    assert!(e.message.contains("ghost"));
    assert!(e.location.contains("left_credential"), "{}", e.location);
}

#[test]
fn error_link_unknown_left_claim() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "a", "format": "ldp_vc",
         "claims": [{"id": "real_claim", "path": ["issuer"]}]},
        {"id": "b", "format": "ldp_vc",
         "claims": [{"id": "y", "path": ["issuer"]}]}
      ],
      "credential_links": [{
        "left_credential": "a", "left_claim": "ghost_claim",
        "right_credential": "b", "right_claim": "y"
      }]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::UnknownClaimId)
        .expect("expected UnknownClaimId");
    assert!(e.message.contains("ghost_claim"), "{}", e.message);
    assert!(e.hint.as_ref().unwrap().contains("real_claim"), "{:?}", e.hint);
}

#[test]
fn error_link_claim_has_no_id() {
    // claim exists but has no 'id' field → MissingClaimId
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "a", "format": "ldp_vc",
         "claims": [{"path": ["issuer"]}]},
        {"id": "b", "format": "ldp_vc",
         "claims": [{"id": "y", "path": ["issuer"]}]}
      ],
      "credential_links": [{
        "left_credential": "a", "left_claim": "anything",
        "right_credential": "b", "right_claim": "y"
      }]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::MissingClaimId));
}

#[test]
fn warning_self_link() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "a", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]}
      ],
      "credential_links": [{
        "left_credential": "a", "left_claim": "x",
        "right_credential": "a", "right_claim": "x"
      }]
    }"#);
    let r = q.validate();
    assert!(r.is_valid(), "self-link is a warning, not an error");
    assert!(r.warnings.iter().any(|w| w.code == ErrorCode::SelfLinkDetected));
    let w = r.warnings.iter().find(|w| w.code == ErrorCode::SelfLinkDetected).unwrap();
    assert!(w.hint.is_some());
}

// ─── aggregates ───────────────────────────────────────────────────────────────

#[test]
fn error_aggregate_unknown_credential() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "real", "format": "ldp_vc", "multiple": true,
         "claims": [{"id": "bal", "path": ["credentialSubject", "balance"]}]}
      ],
      "aggregates": [{
        "id": "total", "credential_id": "ghost",
        "claim_id": "bal", "function": "sum"
      }]
    }"#);
    let r = q.validate();
    assert!(r.errors.iter().any(|e| e.code == ErrorCode::UnknownCredentialId));
}

#[test]
fn error_aggregate_unknown_claim() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc", "multiple": true,
         "claims": [{"id": "real_claim", "path": ["credentialSubject", "x"]}]}
      ],
      "aggregates": [{
        "id": "a", "credential_id": "c",
        "claim_id": "ghost_claim", "function": "count"
      }]
    }"#);
    let r = q.validate();
    let e = r.errors.iter().find(|e| e.code == ErrorCode::UnknownClaimId).unwrap();
    assert!(e.message.contains("ghost_claim"));
    assert!(e.hint.as_ref().unwrap().contains("real_claim"));
}

#[test]
fn warning_aggregate_without_multiple() {
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "c", "format": "ldp_vc",
         "claims": [{"id": "bal", "path": ["credentialSubject", "balance"]}]}
      ],
      "aggregates": [{
        "id": "total", "credential_id": "c",
        "claim_id": "bal", "function": "sum"
      }]
    }"#);
    let r = q.validate();
    assert!(r.is_valid(), "should be valid (only a warning):\n{}", r);
    let w = r.warnings.iter().find(|w| w.code == ErrorCode::AggregateWithoutMultiple).unwrap();
    assert!(w.hint.as_ref().unwrap().contains("multiple"), "{:?}", w.hint);
}

// ─── multiple errors collected in one pass ────────────────────────────────────

#[test]
fn collects_multiple_errors() {
    // Two errors: duplicate credential id AND unknown claim in link
    let q = raw_parse(r#"{
      "credentials": [
        {"id": "a", "format": "ldp_vc",
         "claims": [{"id": "x", "path": ["issuer"]}]},
        {"id": "a", "format": "ldp_vc",
         "claims": [{"id": "y", "path": ["issuer"]}]}
      ],
      "credential_links": [{
        "left_credential": "a", "left_claim": "x",
        "right_credential": "ghost", "right_claim": "y"
      }]
    }"#);
    let r = q.validate();
    assert!(r.errors.len() >= 2, "expected at least 2 errors, got: {:?}",
        r.errors.iter().map(|e| &e.code).collect::<Vec<_>>());
}

// ─── Display formatting ───────────────────────────────────────────────────────

#[test]
fn validation_error_display_includes_location_and_hint() {
    let q = raw_parse(r#"{"credentials": []}"#);
    let r = q.validate();
    let text = format!("{}", r.errors[0]);
    assert!(text.contains("ERROR"), "{text}");
    assert!(text.contains("credentials"), "{text}");
}

#[test]
fn validation_result_display_summary() {
    let q = raw_parse(r#"{"credentials": []}"#);
    let r = q.validate();
    let text = format!("{}", r);
    assert!(text.contains("invalid"), "{text}");
}

// ─── SPARQL validation (feature-gated) ───────────────────────────────────────

#[cfg(feature = "sparql-validation")]
mod sparql_validation {
    use dcql_plus_to_sparql_rs::{ExtendedDcqlQuery, SparqlTranslator, SparqlValidate};

    #[test]
    fn translated_sparql_parses_cleanly() {
        let json = r#"{
          "credentials": [
            {"id": "id_card", "format": "ldp_vc",
             "claims": [
               {"id": "iss", "path": ["issuer"]},
               {"id": "name", "path": ["credentialSubject", "name"]}
             ]}
          ]
        }"#;
        let q = ExtendedDcqlQuery::from_json(json).unwrap();
        let sparql = SparqlTranslator::new().translate(&q).unwrap();
        sparql.validate_sparql().expect("translated SPARQL should be valid");
    }

    #[test]
    fn invalid_sparql_returns_error() {
        let bad = "SELECT ?x WHERE { NOT VALID SPARQL }";
        let result = bad.validate_sparql();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("SPARQL parse error"), "{msg}");
    }

    #[test]
    fn cross_credential_sparql_parses_cleanly() {
        let json = r#"{
          "credentials": [
            {"id": "root_ca", "format": "ldp_vc",
             "claims": [{"id": "ca_sub", "path": ["credentialSubject", "id"]}]},
            {"id": "employee", "format": "ldp_vc",
             "claims": [{"id": "emp_iss", "path": ["issuer"]}]}
          ],
          "credential_links": [{
            "left_credential": "employee", "left_claim": "emp_iss",
            "right_credential": "root_ca", "right_claim": "ca_sub"
          }]
        }"#;
        let q = ExtendedDcqlQuery::from_json(json).unwrap();
        let sparql = SparqlTranslator::new().translate(&q).unwrap();
        sparql.validate_sparql().expect("trust-chain SPARQL should be valid");
    }
}
