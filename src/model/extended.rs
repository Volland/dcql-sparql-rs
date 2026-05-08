use serde::{Deserialize, Serialize};
use crate::model::dcql::{CredentialQuery, CredentialSetQuery};

/// Extended DCQL query with cross-credential matching capabilities.
///
/// Fully backward compatible with standard DCQL: an ExtendedDcqlQuery
/// with no credential_links or aggregates is equivalent to a DcqlQuery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedDcqlQuery {
    pub credentials: Vec<CredentialQuery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_sets: Option<Vec<CredentialSetQuery>>,
    /// Cross-credential join conditions (extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_links: Option<Vec<CredentialLink>>,
    /// Aggregation queries across multiple matching credentials (extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregates: Option<Vec<AggregateQuery>>,
}

impl ExtendedDcqlQuery {
    pub fn from_json(s: &str) -> crate::Result<Self> {
        let q: Self = serde_json::from_str(s)?;
        q.validate()?;
        Ok(q)
    }

    fn validate(&self) -> crate::Result<()> {
        use crate::error::DcqlError;
        if self.credentials.is_empty() {
            return Err(DcqlError::Validation("credentials must not be empty".into()));
        }
        // Validate credential ids are unique
        let mut seen = std::collections::HashSet::new();
        for cred in &self.credentials {
            if !seen.insert(&cred.id) {
                return Err(DcqlError::Validation(format!(
                    "duplicate credential id: {}",
                    cred.id
                )));
            }
        }
        // Validate credential_links reference known ids
        if let Some(links) = &self.credential_links {
            let cred_map: std::collections::HashMap<&str, &CredentialQuery> =
                self.credentials.iter().map(|c| (c.id.as_str(), c)).collect();
            for link in links {
                let left = cred_map
                    .get(link.left_credential.as_str())
                    .ok_or_else(|| DcqlError::UnknownCredentialId(link.left_credential.clone()))?;
                let right = cred_map
                    .get(link.right_credential.as_str())
                    .ok_or_else(|| {
                        DcqlError::UnknownCredentialId(link.right_credential.clone())
                    })?;
                // Validate claim ids exist
                Self::find_claim(left, &link.left_claim)?;
                Self::find_claim(right, &link.right_claim)?;
            }
        }
        Ok(())
    }

    fn find_claim<'a>(
        cred: &'a CredentialQuery,
        claim_id: &str,
    ) -> crate::Result<&'a crate::model::dcql::ClaimQuery> {
        cred.claims
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .find(|c| c.id.as_deref() == Some(claim_id))
            .ok_or_else(|| {
                crate::error::DcqlError::UnknownClaimId(
                    claim_id.to_string(),
                    cred.id.clone(),
                )
            })
    }
}

impl From<crate::model::dcql::DcqlQuery> for ExtendedDcqlQuery {
    fn from(q: crate::model::dcql::DcqlQuery) -> Self {
        Self {
            credentials: q.credentials,
            credential_sets: q.credential_sets,
            credential_links: None,
            aggregates: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialLink {
    /// The id of the left-hand CredentialQuery.
    pub left_credential: String,
    /// The id of the ClaimQuery within the left credential.
    pub left_claim: String,
    /// The id of the right-hand CredentialQuery.
    pub right_credential: String,
    /// The id of the ClaimQuery within the right credential.
    pub right_claim: String,
    /// The relation between the two claim values.
    #[serde(default)]
    pub relation: LinkRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LinkRelation {
    #[default]
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

impl LinkRelation {
    pub fn to_sparql_op(&self) -> &'static str {
        match self {
            LinkRelation::Equal => "=",
            LinkRelation::NotEqual => "!=",
            LinkRelation::LessThan => "<",
            LinkRelation::LessThanOrEqual => "<=",
            LinkRelation::GreaterThan => ">",
            LinkRelation::GreaterThanOrEqual => ">=",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateQuery {
    /// Unique id for this aggregate result.
    pub id: String,
    /// The CredentialQuery id to aggregate over (must have `multiple: true`).
    pub credential_id: String,
    /// The ClaimQuery id within that credential to aggregate.
    pub claim_id: String,
    pub function: AggregateFunction,
    /// HAVING filter on the aggregate result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub having: Option<AggregateHaving>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateFunction {
    Sum,
    Count,
    Min,
    Max,
    Avg,
}

impl AggregateFunction {
    pub fn to_sparql(&self) -> &'static str {
        match self {
            AggregateFunction::Sum => "SUM",
            AggregateFunction::Count => "COUNT",
            AggregateFunction::Min => "MIN",
            AggregateFunction::Max => "MAX",
            AggregateFunction::Avg => "AVG",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateHaving {
    pub op: crate::model::dcql::FilterOp,
    pub value: crate::model::dcql::FilterValue,
}
