use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use std::fmt;

// ---------------------------------------------------------------------------
// CredentialFormat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CredentialFormat {
    #[serde(rename = "dc+sd-jwt")]
    SdJwtVc,
    #[serde(rename = "jwt_vc_json")]
    JwtVcJson,
    #[serde(rename = "ldp_vc")]
    LdpVc,
    #[serde(rename = "mso_mdoc")]
    MsoMdoc,
}

// ---------------------------------------------------------------------------
// PathElement
// ---------------------------------------------------------------------------

/// A single element of a claim path.
/// - `Key(String)` → JSON string
/// - `Wildcard`    → JSON null
/// - `Index(u64)`  → JSON number
#[derive(Debug, Clone, PartialEq)]
pub enum PathElement {
    Key(String),
    Wildcard,
    Index(u64),
}

impl Serialize for PathElement {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            PathElement::Key(s) => serializer.serialize_str(s),
            PathElement::Wildcard => serializer.serialize_none(),
            PathElement::Index(n) => serializer.serialize_u64(*n),
        }
    }
}

struct PathElementVisitor;

impl<'de> Visitor<'de> for PathElementVisitor {
    type Value = PathElement;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string, null, or non-negative integer")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
        Ok(PathElement::Key(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Self::Value, E> {
        Ok(PathElement::Key(v))
    }

    fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(PathElement::Wildcard)
    }

    fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(PathElement::Wildcard)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
        Ok(PathElement::Index(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
        if v < 0 {
            Err(E::custom("negative index is not allowed"))
        } else {
            Ok(PathElement::Index(v as u64))
        }
    }
}

impl<'de> Deserialize<'de> for PathElement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(PathElementVisitor)
    }
}

// ---------------------------------------------------------------------------
// ClaimValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimValue {
    Text(String),
    Integer(i64),
    Bool(bool),
}

// ---------------------------------------------------------------------------
// FilterOp / FilterValue / ClaimFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Regex,
    LangMatches,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimFilter {
    pub op: FilterOp,
    pub value: FilterValue,
}

// ---------------------------------------------------------------------------
// ClaimQuery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub path: Vec<PathElement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<ClaimValue>>,
    /// Extension field: structured filter predicate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ClaimFilter>,
}

// ---------------------------------------------------------------------------
// CredentialSetQuery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSetQuery {
    pub options: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// CredentialQuery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialQuery {
    pub id: String,
    pub format: CredentialFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<Vec<ClaimQuery>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_sets: Option<Vec<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_authorities: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
}

// ---------------------------------------------------------------------------
// DcqlQuery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcqlQuery {
    pub credentials: Vec<CredentialQuery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_sets: Option<Vec<CredentialSetQuery>>,
}

impl DcqlQuery {
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
        for cred in &self.credentials {
            validate_id(&cred.id)?;
        }
        Ok(())
    }
}

fn validate_id(id: &str) -> crate::Result<()> {
    use crate::error::DcqlError;
    if id.is_empty() {
        return Err(DcqlError::Validation("credential id must not be empty".into()));
    }
    for ch in id.chars() {
        if !ch.is_alphanumeric() && ch != '_' && ch != '-' {
            return Err(DcqlError::Validation(format!(
                "credential id '{}' contains invalid character '{}'",
                id, ch
            )));
        }
    }
    Ok(())
}
