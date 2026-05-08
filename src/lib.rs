pub mod error;
pub mod model;
pub mod sparql;
pub mod matcher;
pub mod validation;

pub use error::{DcqlError, Result};
pub use model::dcql::{
    ClaimFilter, ClaimQuery, ClaimValue, CredentialFormat, CredentialQuery, CredentialSetQuery,
    DcqlQuery, FilterOp, FilterValue, PathElement,
};
pub use model::extended::{
    AggregateFunction, AggregateHaving, AggregateQuery, CredentialLink, ExtendedDcqlQuery,
    LinkRelation,
};
pub use sparql::translator::{SparqlTranslator, TranslationOptions};
pub use validation::dcql::{DcqlValidate, ErrorCode, Severity, ValidationError, ValidationResult};
#[cfg(feature = "sparql-validation")]
pub use validation::sparql::{SparqlValidate, SparqlValidationError};
