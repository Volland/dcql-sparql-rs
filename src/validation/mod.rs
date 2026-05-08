pub mod dcql;
#[cfg(feature = "sparql-validation")]
pub mod sparql;

pub use dcql::{DcqlValidate, ErrorCode, Severity, ValidationError, ValidationResult};
#[cfg(feature = "sparql-validation")]
pub use sparql::{SparqlValidate, SparqlValidationError};
