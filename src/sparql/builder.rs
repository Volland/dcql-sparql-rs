/// A minimal SPARQL 1.1 SELECT query builder.
pub struct SparqlQueryBuilder {
    prefixes: Vec<(String, String)>,
    select_vars: Vec<String>,
    where_clauses: Vec<String>,
    filters: Vec<String>,
    group_by: Vec<String>,
    having: Vec<String>,
    limit: Option<usize>,
    select_all: bool,
}

impl Default for SparqlQueryBuilder {
    fn default() -> Self {
        Self {
            prefixes: Vec::new(),
            select_vars: Vec::new(),
            where_clauses: Vec::new(),
            filters: Vec::new(),
            group_by: Vec::new(),
            having: Vec::new(),
            limit: None,
            select_all: false,
        }
    }
}

impl SparqlQueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a PREFIX declaration. prefix should not include trailing colon.
    pub fn prefix(mut self, prefix: &str, iri: &str) -> Self {
        self.prefixes.push((prefix.to_string(), format!("<{}>", iri)));
        self
    }

    /// Add a SELECT variable. var should include the ? sigil.
    pub fn select(mut self, var: &str) -> Self {
        self.select_vars.push(var.to_string());
        self
    }

    /// Use SELECT * instead of named variables.
    pub fn select_all(mut self) -> Self {
        self.select_all = true;
        self
    }

    /// Add a WHERE clause triple or pattern (raw SPARQL fragment).
    pub fn r#where(mut self, pattern: impl Into<String>) -> Self {
        self.where_clauses.push(pattern.into());
        self
    }

    /// Add a FILTER expression (will be wrapped in FILTER(...)).
    pub fn filter(mut self, expr: impl Into<String>) -> Self {
        self.filters.push(expr.into());
        self
    }

    /// Add a GROUP BY variable.
    pub fn group_by(mut self, var: &str) -> Self {
        self.group_by.push(var.to_string());
        self
    }

    /// Add a HAVING expression.
    pub fn having(mut self, expr: impl Into<String>) -> Self {
        self.having.push(expr.into());
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Build the final SPARQL query string.
    pub fn build(self) -> String {
        let mut out = String::new();

        // Prefixes
        for (prefix, iri) in &self.prefixes {
            out.push_str(&format!("PREFIX {}: {}\n", prefix, iri));
        }
        if !self.prefixes.is_empty() {
            out.push('\n');
        }

        // SELECT
        if self.select_all {
            out.push_str("SELECT *\n");
        } else if self.select_vars.is_empty() {
            out.push_str("SELECT *\n");
        } else {
            out.push_str("SELECT ");
            out.push_str(&self.select_vars.join(" "));
            out.push('\n');
        }

        // WHERE
        out.push_str("WHERE {\n");
        for clause in &self.where_clauses {
            // Indent each line of the clause
            for line in clause.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
        for filter in &self.filters {
            out.push_str(&format!("  FILTER({})\n", filter));
        }
        out.push_str("}\n");

        // GROUP BY
        if !self.group_by.is_empty() {
            out.push_str("GROUP BY ");
            out.push_str(&self.group_by.join(" "));
            out.push('\n');
        }

        // HAVING
        if !self.having.is_empty() {
            out.push_str("HAVING (");
            out.push_str(&self.having.join(" && "));
            out.push_str(")\n");
        }

        // LIMIT
        if let Some(l) = self.limit {
            out.push_str(&format!("LIMIT {}\n", l));
        }

        out
    }
}
