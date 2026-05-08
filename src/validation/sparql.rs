use std::fmt;

/// Error returned when a SPARQL string fails to parse.
#[derive(Debug)]
pub struct SparqlValidationError {
    /// Human-readable parse error from spargebra.
    pub message: String,
}

impl fmt::Display for SparqlValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SPARQL parse error: {}", self.message)
    }
}

impl std::error::Error for SparqlValidationError {}

/// Validates a SPARQL 1.1 query string by parsing it with spargebra.
///
/// Implemented on `str` and `String`. The translated output of
/// [`SparqlTranslator::translate`] can be validated immediately:
///
/// ```rust,ignore
/// use dcql_plus_to_sparql_rs::{ExtendedDcqlQuery, SparqlTranslator};
/// use dcql_plus_to_sparql_rs::validation::SparqlValidate;
///
/// let sparql = SparqlTranslator::new().translate(&query)?;
/// sparql.validate_sparql()?;
/// ```
pub trait SparqlValidate {
    fn validate_sparql(&self) -> Result<(), SparqlValidationError>;
}

impl SparqlValidate for str {
    fn validate_sparql(&self) -> Result<(), SparqlValidationError> {
        spargebra::Query::parse(self, None)
            .map(|_| ())
            .map_err(|e| SparqlValidationError {
                message: e.to_string(),
            })
    }
}

impl SparqlValidate for String {
    fn validate_sparql(&self) -> Result<(), SparqlValidationError> {
        self.as_str().validate_sparql()
    }
}
