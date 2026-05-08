use thiserror::Error;

#[derive(Debug, Error)]
pub enum DcqlError {
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("translation error: {0}")]
    Translation(String),
    #[error("unknown credential id '{0}' referenced in credential_links")]
    UnknownCredentialId(String),
    #[error("unknown claim id '{0}' in credential '{1}'")]
    UnknownClaimId(String, String),
    #[error("claim has no id but is referenced by credential_links or claim_sets")]
    MissingClaimId,
    #[error("empty path is not allowed")]
    EmptyPath,
}

pub type Result<T> = std::result::Result<T, DcqlError>;
