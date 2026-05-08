use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::model::dcql::{ClaimQuery, CredentialQuery, DcqlQuery};
use crate::model::extended::ExtendedDcqlQuery;

// ─── error types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARN"),
        }
    }
}

/// Machine-readable classification of a validation finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    // ── structural ──────────────────────────────────────────────────────────
    /// `credentials` array is absent or empty.
    EmptyCredentials,
    /// A `claims` array is present but contains zero elements.
    EmptyClaimsArray,
    /// A claim's `path` array is empty.
    EmptyPath,
    /// A `credential_sets.options` array or one of its inner arrays is empty.
    EmptyOptions,

    // ── identity ────────────────────────────────────────────────────────────
    /// Two credentials share the same `id`.
    DuplicateCredentialId,
    /// Two claims within the same credential share the same `id`.
    DuplicateClaimId,
    /// A credential `id` contains characters outside `[a-zA-Z0-9_-]`.
    InvalidCredentialId,
    /// A claim referenced by `credential_links`, `claim_sets`, or `aggregates`
    /// has no `id` field.
    MissingClaimId,

    // ── cross-reference ─────────────────────────────────────────────────────
    /// A referenced credential `id` does not exist in `credentials`.
    UnknownCredentialId,
    /// A referenced claim `id` does not exist in the credential's `claims`.
    UnknownClaimId,
    /// `claim_sets` is present but `claims` is absent.
    ClaimSetsWithoutClaims,

    // ── logic warnings ──────────────────────────────────────────────────────
    /// A credential specifies no `claims` (all claims will be requested).
    NoClaimsSpecified,
    /// An aggregate references a credential without `multiple: true`.
    AggregateWithoutMultiple,
    /// A claim has both `filter` and `values` (the semantics may be surprising).
    FilterAndValuesBothPresent,
    /// A `credential_link` references the same claim on both sides.
    SelfLinkDetected,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A single validation finding (error or warning).
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub code: ErrorCode,
    pub severity: Severity,
    /// Human-readable description of what is wrong.
    pub message: String,
    /// Dot-notation path to the offending field, e.g. `credentials[0].claims[1].path`.
    pub location: String,
    /// Actionable suggestion for fixing the problem.
    pub hint: Option<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} at '{}': {}", self.severity, self.code, self.location, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {}", hint)?;
        }
        Ok(())
    }
}

/// The full result of validating a DCQL+ query.
///
/// A result is **valid** when `errors` is empty. Warnings do not block translation.
#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// All findings (errors first, then warnings) in one flat iterator.
    pub fn all(&self) -> impl Iterator<Item = &ValidationError> {
        self.errors.iter().chain(self.warnings.iter())
    }

    /// Converts to `Ok(warnings)` if valid, `Err(errors)` otherwise.
    pub fn into_result(self) -> Result<Vec<ValidationError>, Vec<ValidationError>> {
        if self.errors.is_empty() {
            Ok(self.warnings)
        } else {
            Err(self.errors)
        }
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for e in self.all() {
            writeln!(f, "{}", e)?;
        }
        if self.errors.is_empty() {
            write!(f, "valid ({} warning(s))", self.warnings.len())
        } else {
            write!(f, "invalid: {} error(s), {} warning(s)", self.errors.len(), self.warnings.len())
        }
    }
}

// ─── trait ──────────────────────────────────────────────────────────────────

/// Validates a DCQL or DCQL+ query, collecting all findings in one pass.
pub trait DcqlValidate {
    fn validate(&self) -> ValidationResult;
}

impl DcqlValidate for ExtendedDcqlQuery {
    fn validate(&self) -> ValidationResult {
        validate_extended(self)
    }
}

impl DcqlValidate for DcqlQuery {
    fn validate(&self) -> ValidationResult {
        let extended = ExtendedDcqlQuery::from(self.clone());
        validate_extended(&extended)
    }
}

// ─── implementation ──────────────────────────────────────────────────────────

fn err(
    code: ErrorCode,
    message: impl Into<String>,
    location: impl Into<String>,
    hint: Option<String>,
) -> ValidationError {
    ValidationError {
        code,
        severity: Severity::Error,
        message: message.into(),
        location: location.into(),
        hint,
    }
}

fn warn(
    code: ErrorCode,
    message: impl Into<String>,
    location: impl Into<String>,
    hint: Option<String>,
) -> ValidationError {
    ValidationError {
        code,
        severity: Severity::Warning,
        message: message.into(),
        location: location.into(),
        hint,
    }
}

fn validate_extended(query: &ExtendedDcqlQuery) -> ValidationResult {
    let mut errors: Vec<ValidationError> = Vec::new();
    let mut warnings: Vec<ValidationError> = Vec::new();

    // ── 1. credentials must be non-empty ────────────────────────────────────
    if query.credentials.is_empty() {
        errors.push(err(
            ErrorCode::EmptyCredentials,
            "The 'credentials' array must contain at least one CredentialQuery",
            "credentials",
            Some("Add at least one credential entry with a unique 'id' and 'format'".into()),
        ));
        return ValidationResult { errors, warnings };
    }

    // ── 2. Per-credential validation ─────────────────────────────────────────
    // Build lookup tables while validating structure.
    // cred_id -> &CredentialQuery
    let mut cred_map: HashMap<&str, &CredentialQuery> = HashMap::new();
    // cred_id -> claim_id -> &ClaimQuery
    let mut claim_map: HashMap<&str, HashMap<&str, &ClaimQuery>> = HashMap::new();

    for (ci, cred) in query.credentials.iter().enumerate() {
        let cred_loc = format!("credentials[{}]", ci);

        // ── ID validation ──────────────────────────────────────────────────
        if cred.id.is_empty() {
            errors.push(err(
                ErrorCode::InvalidCredentialId,
                "Credential 'id' must not be empty",
                format!("{}.id", cred_loc),
                Some("Use alphanumeric characters, hyphens, or underscores (e.g. \"identity\")".into()),
            ));
        } else {
            let bad: Vec<char> = cred.id
                .chars()
                .filter(|c| !c.is_alphanumeric() && *c != '_' && *c != '-')
                .collect();
            if !bad.is_empty() {
                errors.push(err(
                    ErrorCode::InvalidCredentialId,
                    format!(
                        "Credential id '{}' contains invalid character(s): {}",
                        cred.id,
                        bad.iter().map(|c| format!("'{}'", c)).collect::<Vec<_>>().join(", ")
                    ),
                    format!("{}.id", cred_loc),
                    Some("Credential IDs must match [a-zA-Z0-9_-]+".into()),
                ));
            }

            if cred_map.contains_key(cred.id.as_str()) {
                errors.push(err(
                    ErrorCode::DuplicateCredentialId,
                    format!("Credential id '{}' is defined more than once", cred.id),
                    format!("{}.id", cred_loc),
                    Some("Each credential must have a unique 'id' within the query".into()),
                ));
            } else {
                cred_map.insert(&cred.id, cred);
            }
        }

        // ── claims validation ──────────────────────────────────────────────
        match &cred.claims {
            None => {
                warnings.push(warn(
                    ErrorCode::NoClaimsSpecified,
                    format!(
                        "Credential '{}' has no 'claims' — all claims will be requested from the wallet",
                        cred.id
                    ),
                    format!("{}.claims", cred_loc),
                    Some("Add a 'claims' array to request only the fields your verifier needs".into()),
                ));
            }
            Some(claims) if claims.is_empty() => {
                errors.push(err(
                    ErrorCode::EmptyClaimsArray,
                    format!("Credential '{}' has an empty 'claims' array", cred.id),
                    format!("{}.claims", cred_loc),
                    Some("Either omit 'claims' to request all, or add at least one ClaimQuery".into()),
                ));
            }
            Some(claims) => {
                let mut seen_claim_ids: HashSet<&str> = HashSet::new();
                let mut this_claim_map: HashMap<&str, &ClaimQuery> = HashMap::new();

                for (qi, claim) in claims.iter().enumerate() {
                    let claim_loc = format!("{}.claims[{}]", cred_loc, qi);

                    // path must be non-empty
                    if claim.path.is_empty() {
                        errors.push(err(
                            ErrorCode::EmptyPath,
                            "Claim 'path' must contain at least one element",
                            format!("{}.path", claim_loc),
                            Some(
                                "Example: [\"credentialSubject\", \"name\"] selects the name field"
                                    .into(),
                            ),
                        ));
                    }

                    // duplicate claim id
                    if let Some(id) = &claim.id {
                        if !seen_claim_ids.insert(id.as_str()) {
                            errors.push(err(
                                ErrorCode::DuplicateClaimId,
                                format!(
                                    "Claim id '{}' appears more than once in credential '{}'",
                                    id, cred.id
                                ),
                                format!("{}.id", claim_loc),
                                Some(
                                    "Each claim within a credential must have a unique 'id'"
                                        .into(),
                                ),
                            ));
                        } else {
                            this_claim_map.insert(id.as_str(), claim);
                        }
                    }

                    // filter + values both present
                    if claim.filter.is_some() && claim.values.is_some() {
                        warnings.push(warn(
                            ErrorCode::FilterAndValuesBothPresent,
                            format!(
                                "Claim at {} has both 'filter' (SPARQL predicate) and 'values' \
                                 (wallet pre-filter hint). 'values' will be passed to the wallet \
                                 for pre-filtering while 'filter' generates the SPARQL FILTER.",
                                claim_loc
                            ),
                            claim_loc,
                            Some(
                                "This is allowed but may cause confusion. Use 'filter' for \
                                 verifier-enforced predicates and 'values' for wallet UI hints."
                                    .into(),
                            ),
                        ));
                    }
                }
                claim_map.insert(&cred.id, this_claim_map);
            }
        }

        // ── claim_sets validation ──────────────────────────────────────────
        if let Some(claim_sets) = &cred.claim_sets {
            match &cred.claims {
                None => {
                    errors.push(err(
                        ErrorCode::ClaimSetsWithoutClaims,
                        format!(
                            "Credential '{}' has 'claim_sets' but no 'claims'",
                            cred.id
                        ),
                        format!("{}.claim_sets", cred_loc),
                        Some(
                            "'claim_sets' selects subsets of 'claims'; \
                             both must be present together"
                                .into(),
                        ),
                    ));
                }
                Some(claims) => {
                    let ids_with_id: HashSet<&str> =
                        claims.iter().filter_map(|c| c.id.as_deref()).collect();

                    for (si, set) in claim_sets.iter().enumerate() {
                        for (ri, ref_id) in set.iter().enumerate() {
                            if !ids_with_id.contains(ref_id.as_str()) {
                                let available: Vec<&&str> = ids_with_id.iter().collect();
                                errors.push(err(
                                    ErrorCode::UnknownClaimId,
                                    format!(
                                        "claim_sets[{}][{}] references claim id '{}' which \
                                         does not exist in credential '{}'",
                                        si, ri, ref_id, cred.id
                                    ),
                                    format!("{}.claim_sets[{}][{}]", cred_loc, si, ri),
                                    Some(format!(
                                        "Available claim ids with 'id' set: {:?}",
                                        available
                                    )),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 3. credential_sets ───────────────────────────────────────────────────
    if let Some(cred_sets) = &query.credential_sets {
        for (si, set) in cred_sets.iter().enumerate() {
            let set_loc = format!("credential_sets[{}]", si);

            if set.options.is_empty() {
                errors.push(err(
                    ErrorCode::EmptyOptions,
                    format!("{}.options must not be empty", set_loc),
                    format!("{}.options", set_loc),
                    Some("Each CredentialSetQuery must have at least one option array".into()),
                ));
                continue;
            }

            for (oi, option) in set.options.iter().enumerate() {
                if option.is_empty() {
                    errors.push(err(
                        ErrorCode::EmptyOptions,
                        format!("{}.options[{}] must not be empty", set_loc, oi),
                        format!("{}.options[{}]", set_loc, oi),
                        Some("An option must list at least one credential id".into()),
                    ));
                }
                for (ri, cred_ref) in option.iter().enumerate() {
                    if !cred_map.contains_key(cred_ref.as_str()) {
                        let known: Vec<&&str> = cred_map.keys().collect();
                        errors.push(err(
                            ErrorCode::UnknownCredentialId,
                            format!(
                                "{}.options[{}][{}] references unknown credential id '{}'",
                                set_loc, oi, ri, cred_ref
                            ),
                            format!("{}.options[{}][{}]", set_loc, oi, ri),
                            Some(format!("Known credential ids: {:?}", known)),
                        ));
                    }
                }
            }
        }
    }

    // ── 4. credential_links ──────────────────────────────────────────────────
    if let Some(links) = &query.credential_links {
        for (li, link) in links.iter().enumerate() {
            let link_loc = format!("credential_links[{}]", li);

            for (side, cred_id, claim_id) in [
                ("left", &link.left_credential, &link.left_claim),
                ("right", &link.right_credential, &link.right_claim),
            ] {
                if !cred_map.contains_key(cred_id.as_str()) {
                    let known: Vec<&&str> = cred_map.keys().collect();
                    errors.push(err(
                        ErrorCode::UnknownCredentialId,
                        format!(
                            "{}.{}_credential '{}' does not match any credential id",
                            link_loc, side, cred_id
                        ),
                        format!("{}.{}_credential", link_loc, side),
                        Some(format!("Known credential ids: {:?}", known)),
                    ));
                } else {
                    match claim_map.get(cred_id.as_str()) {
                        None => {
                            errors.push(err(
                                ErrorCode::MissingClaimId,
                                format!(
                                    "{}: credential '{}' has no claims with 'id' fields",
                                    link_loc, cred_id
                                ),
                                format!("{}.{}_claim", link_loc, side),
                                Some(
                                    "Claims used in credential_links must have an 'id' field"
                                        .into(),
                                ),
                            ));
                        }
                        Some(claims) if claims.is_empty() => {
                            errors.push(err(
                                ErrorCode::MissingClaimId,
                                format!(
                                    "{}: credential '{}' has no claims with 'id' fields",
                                    link_loc, cred_id
                                ),
                                format!("{}.{}_claim", link_loc, side),
                                Some(
                                    "Claims used in credential_links must have an 'id' field"
                                        .into(),
                                ),
                            ));
                        }
                        Some(claims) if !claims.contains_key(claim_id.as_str()) => {
                            let available: Vec<&&str> = claims.keys().collect();
                            errors.push(err(
                                ErrorCode::UnknownClaimId,
                                format!(
                                    "{}.{}_claim '{}' does not exist in credential '{}'",
                                    link_loc, side, claim_id, cred_id
                                ),
                                format!("{}.{}_claim", link_loc, side),
                                Some(format!(
                                    "Available claim ids in '{}': {:?}",
                                    cred_id, available
                                )),
                            ));
                        }
                        _ => {}
                    }
                }
            }

            // self-link tautology warning
            if link.left_credential == link.right_credential
                && link.left_claim == link.right_claim
            {
                warnings.push(warn(
                    ErrorCode::SelfLinkDetected,
                    format!(
                        "{}: both sides reference the same claim '{}' in credential '{}' — \
                         this condition is always true",
                        link_loc, link.left_claim, link.left_credential
                    ),
                    link_loc,
                    Some(
                        "A self-link with relation 'equal' is a tautology. \
                         Did you mean to join two different credentials?"
                            .into(),
                    ),
                ));
            }
        }
    }

    // ── 5. aggregates ────────────────────────────────────────────────────────
    if let Some(aggregates) = &query.aggregates {
        for (ai, agg) in aggregates.iter().enumerate() {
            let agg_loc = format!("aggregates[{}]", ai);

            if !cred_map.contains_key(agg.credential_id.as_str()) {
                let known: Vec<&&str> = cred_map.keys().collect();
                errors.push(err(
                    ErrorCode::UnknownCredentialId,
                    format!(
                        "{}.credential_id '{}' does not match any credential id",
                        agg_loc, agg.credential_id
                    ),
                    format!("{}.credential_id", agg_loc),
                    Some(format!("Known credential ids: {:?}", known)),
                ));
                continue;
            }

            // recommend multiple: true
            if let Some(cred) = cred_map.get(agg.credential_id.as_str()) {
                if cred.multiple != Some(true) {
                    warnings.push(warn(
                        ErrorCode::AggregateWithoutMultiple,
                        format!(
                            "{}: credential '{}' is used in an aggregate but does not have \
                             'multiple: true' — the aggregate will operate on at most one instance",
                            agg_loc, agg.credential_id
                        ),
                        format!("{}.credential_id", agg_loc),
                        Some(format!(
                            "Set '\"multiple\": true' on credential '{}' to collect values \
                             across all matching credential instances",
                            agg.credential_id
                        )),
                    ));
                }
            }

            match claim_map.get(agg.credential_id.as_str()) {
                None => {
                    errors.push(err(
                        ErrorCode::MissingClaimId,
                        format!(
                            "{}: credential '{}' has no claims with 'id' fields",
                            agg_loc, agg.credential_id
                        ),
                        format!("{}.claim_id", agg_loc),
                        Some("Claims used in aggregates must have an 'id' field".into()),
                    ));
                }
                Some(claims) if !claims.contains_key(agg.claim_id.as_str()) => {
                    let available: Vec<&&str> = claims.keys().collect();
                    errors.push(err(
                        ErrorCode::UnknownClaimId,
                        format!(
                            "{}.claim_id '{}' does not exist in credential '{}'",
                            agg_loc, agg.claim_id, agg.credential_id
                        ),
                        format!("{}.claim_id", agg_loc),
                        Some(format!(
                            "Available claim ids in '{}': {:?}",
                            agg.credential_id, available
                        )),
                    ));
                }
                _ => {}
            }
        }
    }

    ValidationResult { errors, warnings }
}
