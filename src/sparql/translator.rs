use std::collections::HashMap;

use crate::model::dcql::{ClaimFilter, ClaimValue, FilterOp, FilterValue, PathElement};
use crate::model::extended::{AggregateHaving, ExtendedDcqlQuery};
use crate::sparql::builder::SparqlQueryBuilder;

// ---------------------------------------------------------------------------
// TranslationOptions
// ---------------------------------------------------------------------------

/// Options controlling SPARQL output generation.
#[derive(Debug, Clone)]
pub struct TranslationOptions {
    /// Default namespace prefix name (without colon). Default: "ex".
    pub default_prefix: String,
    /// Default namespace IRI. Default: "https://example.org/vocab#".
    pub default_namespace: String,
    /// Extra prefix declarations to include (prefix, IRI pairs).
    pub extra_prefixes: Vec<(String, String)>,
    /// Custom field-name to IRI mappings for credentialSubject fields.
    pub field_mappings: HashMap<String, String>,
}

impl Default for TranslationOptions {
    fn default() -> Self {
        Self {
            default_prefix: "ex".to_string(),
            default_namespace: "https://example.org/vocab#".to_string(),
            extra_prefixes: Vec::new(),
            field_mappings: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// NamespaceResolver
// ---------------------------------------------------------------------------

struct NamespaceResolver {
    vc_fields: HashMap<String, String>,
    custom: HashMap<String, String>,
    default_prefix: String,
}

impl NamespaceResolver {
    fn new(default_prefix: String, custom: HashMap<String, String>) -> Self {
        let mut vc_fields = HashMap::new();
        vc_fields.insert("issuer".to_string(), "cred:issuer".to_string());
        vc_fields.insert("validFrom".to_string(), "cred:validFrom".to_string());
        vc_fields.insert("validUntil".to_string(), "cred:validUntil".to_string());
        vc_fields.insert("type".to_string(), "rdf:type".to_string());
        vc_fields.insert("@type".to_string(), "rdf:type".to_string());
        vc_fields.insert("holder".to_string(), "cred:holder".to_string());
        vc_fields.insert(
            "credentialStatus".to_string(),
            "cred:credentialStatus".to_string(),
        );
        vc_fields.insert(
            "credentialSchema".to_string(),
            "cred:credentialSchema".to_string(),
        );
        Self {
            vc_fields,
            custom,
            default_prefix,
        }
    }

    fn resolve_vc_field(&self, name: &str) -> Option<&str> {
        self.vc_fields.get(name).map(|s| s.as_str())
    }

    fn resolve_subject_field(&self, name: &str) -> String {
        if let Some(iri) = self.custom.get(name) {
            return iri.clone();
        }
        format!("{}:{}", self.default_prefix, name)
    }
}

// ---------------------------------------------------------------------------
// SparqlTranslator
// ---------------------------------------------------------------------------

/// Translates an ExtendedDcqlQuery into a SPARQL SELECT query string.
pub struct SparqlTranslator {
    resolver: NamespaceResolver,
    options: TranslationOptions,
}

impl SparqlTranslator {
    pub fn new() -> Self {
        Self::with_options(TranslationOptions::default())
    }

    pub fn with_options(opts: TranslationOptions) -> Self {
        let resolver =
            NamespaceResolver::new(opts.default_prefix.clone(), opts.field_mappings.clone());
        Self {
            resolver,
            options: opts,
        }
    }

    /// Translate an ExtendedDcqlQuery to a SPARQL SELECT query string.
    pub fn translate(&self, query: &ExtendedDcqlQuery) -> crate::Result<String> {
        // Step 1: Build query with standard prefixes
        let mut builder = SparqlQueryBuilder::new()
            .prefix("cred", "https://www.w3.org/2018/credentials#")
            .prefix("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#")
            .prefix("rdfs", "http://www.w3.org/2000/01/rdf-schema#")
            .prefix("xsd", "http://www.w3.org/2001/XMLSchema#")
            .prefix(
                &self.options.default_prefix,
                &self.options.default_namespace,
            );

        for (pfx, iri) in &self.options.extra_prefixes {
            builder = builder.prefix(pfx, iri);
        }

        // Step 2: Build claim_var_index: (cred_id, claim_key) -> sparql_var_name
        // Also collect (cred_id, claim_idx) -> claim_key for path lookup
        let mut claim_var_index: HashMap<(String, String), String> = HashMap::new();

        for cred in &query.credentials {
            let safe_cred = sanitize_id(&cred.id);
            if let Some(claims) = &cred.claims {
                for (idx, claim) in claims.iter().enumerate() {
                    let claim_key = claim
                        .id
                        .clone()
                        .unwrap_or_else(|| format!("{}", idx));
                    let val_var =
                        format!("?val_{}_{}", safe_cred, sanitize_id(&claim_key));
                    claim_var_index.insert((cred.id.clone(), claim_key), val_var);
                }
            }
        }

        // Determine if any aggregates exist (affects GROUP BY)
        let has_aggregates = query
            .aggregates
            .as_ref()
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        // Track vc_vars for GROUP BY
        let mut vc_vars: Vec<String> = Vec::new();

        // Step 3: For each CredentialQuery build GRAPH block
        let mut all_filters: Vec<String> = Vec::new();

        for cred in &query.credentials {
            let safe_cred = sanitize_id(&cred.id);
            let vc_var = format!("?vc_{}", safe_cred);
            let subject_var = format!("?subject_{}", safe_cred);

            builder = builder.select(&vc_var);
            vc_vars.push(vc_var.clone());

            let mut graph_body: Vec<String> = Vec::new();
            graph_body.push(format!("{} a cred:VerifiableCredential .", vc_var));

            let mut subject_declared = false;

            if let Some(claims) = &cred.claims {
                for (idx, claim) in claims.iter().enumerate() {
                    let claim_key = claim
                        .id
                        .clone()
                        .unwrap_or_else(|| format!("{}", idx));
                    let val_var = claim_var_index
                        .get(&(cred.id.clone(), claim_key.clone()))
                        .cloned()
                        .unwrap_or_else(|| {
                            format!("?val_{}_{}", safe_cred, sanitize_id(&claim_key))
                        });

                    builder = builder.select(&val_var);

                    if claim.path.is_empty() {
                        return Err(crate::error::DcqlError::EmptyPath);
                    }

                    let path0 = &claim.path[0];
                    let mut mid_counter: usize = 0;

                    match path0 {
                        PathElement::Key(k) if is_vc_level_field(k) => {
                            let field = k.as_str();
                            // Handle id/@id: BIND the vc_var itself
                            if field == "id" || field == "@id" {
                                graph_body.push(format!(
                                    "BIND({} AS {})",
                                    vc_var, val_var
                                ));
                            } else {
                                let pred = self
                                    .resolver
                                    .resolve_vc_field(field)
                                    .unwrap_or(field)
                                    .to_string();
                                let rest = &claim.path[1..];
                                if rest.is_empty() {
                                    graph_body.push(format!(
                                        "{} {} {} .",
                                        vc_var, pred, val_var
                                    ));
                                } else {
                                    // Nested inside VC-level object — chain through intermediates
                                    let mid = format!(
                                        "?mid_{}_{}_{}_vc",
                                        safe_cred,
                                        sanitize_id(&claim_key),
                                        mid_counter
                                    );
                                    mid_counter += 1;
                                    graph_body
                                        .push(format!("{} {} {} .", vc_var, pred, mid));
                                    let nested = translate_path(
                                        &mid,
                                        rest,
                                        &val_var,
                                        &safe_cred,
                                        idx,
                                        &mut mid_counter,
                                        &self.resolver,
                                    );
                                    graph_body.extend(nested);
                                }
                            }
                        }
                        PathElement::Key(k) if k == "credentialSubject" => {
                            if !subject_declared {
                                graph_body.push(format!(
                                    "{} cred:credentialSubject {} .",
                                    vc_var, subject_var
                                ));
                                subject_declared = true;
                            }
                            let remaining = &claim.path[1..];
                            let patterns = translate_path(
                                &subject_var,
                                remaining,
                                &val_var,
                                &safe_cred,
                                idx,
                                &mut mid_counter,
                                &self.resolver,
                            );
                            graph_body.extend(patterns);
                        }
                        _ => {
                            // Treat unknown top-level as credentialSubject field
                            if !subject_declared {
                                graph_body.push(format!(
                                    "{} cred:credentialSubject {} .",
                                    vc_var, subject_var
                                ));
                                subject_declared = true;
                            }
                            let patterns = translate_path(
                                &subject_var,
                                &claim.path,
                                &val_var,
                                &safe_cred,
                                idx,
                                &mut mid_counter,
                                &self.resolver,
                            );
                            graph_body.extend(patterns);
                        }
                    }

                    // Values filter
                    if let Some(values) = &claim.values {
                        if !values.is_empty() {
                            all_filters.push(translate_values_filter(&val_var, values));
                        }
                    }

                    // Extended filter
                    if let Some(f) = &claim.filter {
                        all_filters.push(translate_claim_filter(&val_var, f));
                    }
                }
            }

            // Wrap in GRAPH block
            let mut graph_block = format!("GRAPH {} {{\n", vc_var);
            for line in &graph_body {
                graph_block.push_str("  ");
                graph_block.push_str(line);
                graph_block.push('\n');
            }
            graph_block.push('}');

            builder = builder.r#where(graph_block);
        }

        // Add collected filters
        for f in all_filters {
            builder = builder.filter(f);
        }

        // Step 4: CredentialLinks -> FILTER
        if let Some(links) = &query.credential_links {
            for link in links {
                let left_key = (link.left_credential.clone(), link.left_claim.clone());
                let right_key = (link.right_credential.clone(), link.right_claim.clone());
                let left_var = claim_var_index
                    .get(&left_key)
                    .ok_or_else(|| {
                        crate::error::DcqlError::UnknownClaimId(
                            link.left_claim.clone(),
                            link.left_credential.clone(),
                        )
                    })?;
                let right_var = claim_var_index
                    .get(&right_key)
                    .ok_or_else(|| {
                        crate::error::DcqlError::UnknownClaimId(
                            link.right_claim.clone(),
                            link.right_credential.clone(),
                        )
                    })?;
                let op = link.relation.to_sparql_op();
                builder = builder.filter(format!("{} {} {}", left_var, op, right_var));
            }
        }

        // Step 5: Aggregates
        if let Some(aggregates) = &query.aggregates {
            if !aggregates.is_empty() {
                // Add GROUP BY for all vc_vars
                for vc_var in &vc_vars {
                    builder = builder.group_by(vc_var);
                }

                for agg in aggregates {
                    let source_key = (agg.credential_id.clone(), agg.claim_id.clone());
                    let source_var = claim_var_index.get(&source_key).ok_or_else(|| {
                        crate::error::DcqlError::UnknownClaimId(
                            agg.claim_id.clone(),
                            agg.credential_id.clone(),
                        )
                    })?;
                    let agg_expr = format!(
                        "({fn}({src}) AS ?agg_{id})",
                        fn = agg.function.to_sparql(),
                        src = source_var,
                        id = sanitize_id(&agg.id),
                    );
                    builder = builder.select(&agg_expr);

                    if let Some(having) = &agg.having {
                        let having_expr =
                            translate_having(having, &format!("?agg_{}", sanitize_id(&agg.id)));
                        builder = builder.having(having_expr);
                    }
                }

                // Also add aggregate result vars to GROUP BY select projection
                // (already added via select above)
                // Add non-aggregate claim vars to GROUP BY
                for cred in &query.credentials {
                    if let Some(claims) = &cred.claims {
                        for (idx, claim) in claims.iter().enumerate() {
                            let claim_key = claim
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("{}", idx));
                            // Only group by vars that are NOT the aggregate source
                            let is_agg_source = aggregates.iter().any(|a| {
                                a.credential_id == cred.id && a.claim_id == claim_key
                            });
                            if !is_agg_source {
                                let safe_cred = sanitize_id(&cred.id);
                                let val_var = format!(
                                    "?val_{}_{}",
                                    safe_cred,
                                    sanitize_id(&claim_key)
                                );
                                builder = builder.group_by(&val_var);
                            }
                        }
                    }
                }
            }
        }

        let _ = has_aggregates; // used implicitly above

        Ok(builder.build())
    }
}

impl Default for SparqlTranslator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

fn is_vc_level_field(field: &str) -> bool {
    matches!(
        field,
        "issuer"
            | "validFrom"
            | "validUntil"
            | "type"
            | "@type"
            | "id"
            | "@id"
            | "holder"
            | "credentialStatus"
            | "credentialSchema"
    )
}

fn translate_path(
    current_var: &str,
    path: &[PathElement],
    final_var: &str,
    cred_id: &str,
    claim_idx: usize,
    mid_counter: &mut usize,
    resolver: &NamespaceResolver,
) -> Vec<String> {
    if path.is_empty() {
        // Value IS current_var — emit a BIND to unify
        return vec![format!("BIND({} AS {})", current_var, final_var)];
    }

    match &path[0] {
        PathElement::Key(k) => {
            let pred = resolver.resolve_subject_field(k);
            let rest = &path[1..];
            if rest.is_empty() {
                vec![format!("{} {} {} .", current_var, pred, final_var)]
            } else {
                let mid = format!(
                    "?mid_{}_{}_{}",
                    cred_id, claim_idx, mid_counter
                );
                *mid_counter += 1;
                let mut lines = vec![format!("{} {} {} .", current_var, pred, mid)];
                lines.extend(translate_path(
                    &mid,
                    rest,
                    final_var,
                    cred_id,
                    claim_idx,
                    mid_counter,
                    resolver,
                ));
                lines
            }
        }
        PathElement::Wildcard => {
            let rest = &path[1..];
            let apred = format!("?apred_{}_{}", cred_id, claim_idx);
            if rest.is_empty() {
                vec![format!("{} {} {} .", current_var, apred, final_var)]
            } else {
                let item = format!(
                    "?item_{}_{}_{}",
                    cred_id, claim_idx, mid_counter
                );
                *mid_counter += 1;
                let mut lines = vec![format!("{} {} {} .", current_var, apred, item)];
                lines.extend(translate_path(
                    &item,
                    rest,
                    final_var,
                    cred_id,
                    claim_idx,
                    mid_counter,
                    resolver,
                ));
                lines
            }
        }
        PathElement::Index(i) => {
            let rest = &path[1..];
            let apred = format!("?apred_{}_{}", cred_id, claim_idx);
            if rest.is_empty() {
                vec![
                    format!("# array index {}", i),
                    format!("{} {} {} .", current_var, apred, final_var),
                ]
            } else {
                let item = format!(
                    "?item_{}_{}_{}",
                    cred_id, claim_idx, mid_counter
                );
                *mid_counter += 1;
                let mut lines = vec![
                    format!("# array index {}", i),
                    format!("{} {} {} .", current_var, apred, item),
                ];
                lines.extend(translate_path(
                    &item,
                    rest,
                    final_var,
                    cred_id,
                    claim_idx,
                    mid_counter,
                    resolver,
                ));
                lines
            }
        }
    }
}

fn translate_values_filter(val_var: &str, values: &[ClaimValue]) -> String {
    let parts: Vec<String> = values
        .iter()
        .map(|v| match v {
            ClaimValue::Text(s) => {
                format!("{} = \"{}\"", val_var, s.replace('"', "\\\""))
            }
            ClaimValue::Integer(i) => format!("{} = {}", val_var, i),
            ClaimValue::Bool(b) => format!("{} = {}", val_var, b),
        })
        .collect();
    if parts.len() == 1 {
        parts[0].clone()
    } else {
        format!("({})", parts.join(" || "))
    }
}

fn translate_claim_filter(val_var: &str, filter: &ClaimFilter) -> String {
    let val = match &filter.value {
        FilterValue::Text(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        FilterValue::Integer(i) => i.to_string(),
        FilterValue::Float(f) => f.to_string(),
        FilterValue::Bool(b) => b.to_string(),
    };
    let op = match filter.op {
        FilterOp::Eq => "=",
        FilterOp::Ne => "!=",
        FilterOp::Lt => "<",
        FilterOp::Le => "<=",
        FilterOp::Gt => ">",
        FilterOp::Ge => ">=",
        FilterOp::Regex => {
            return format!("REGEX({}, \"{}\")", val_var, val.trim_matches('"'));
        }
        FilterOp::LangMatches => {
            return format!(
                "LANGMATCHES(LANG({}), \"{}\")",
                val_var,
                val.trim_matches('"')
            );
        }
    };
    format!("{} {} {}", val_var, op, val)
}

fn translate_having(having: &AggregateHaving, agg_var: &str) -> String {
    let val = match &having.value {
        FilterValue::Text(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        FilterValue::Integer(i) => i.to_string(),
        FilterValue::Float(f) => f.to_string(),
        FilterValue::Bool(b) => b.to_string(),
    };
    let op = match having.op {
        FilterOp::Eq => "=",
        FilterOp::Ne => "!=",
        FilterOp::Lt => "<",
        FilterOp::Le => "<=",
        FilterOp::Gt => ">",
        FilterOp::Ge => ">=",
        FilterOp::Regex => {
            return format!("REGEX({}, \"{}\")", agg_var, val.trim_matches('"'));
        }
        FilterOp::LangMatches => {
            return format!(
                "LANGMATCHES(LANG({}), \"{}\")",
                agg_var,
                val.trim_matches('"')
            );
        }
    };
    format!("{} {} {}", agg_var, op, val)
}
