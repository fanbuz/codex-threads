use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchBackend {
    Fts,
    Like,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchQueryMode {
    Literal,
    Expanded,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchMeta {
    pub backend: SearchBackend,
    pub query_mode: SearchQueryMode,
    pub ranking: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_query: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normalized_terms: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchExplain {
    pub rank: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_fields: Vec<String>,
    pub matched_terms: usize,
    pub literal_match: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchReport<T> {
    pub search: SearchMeta,
    pub results: Vec<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct SearchField<'a> {
    pub name: &'static str,
    pub text: Option<&'a str>,
    pub weight: i64,
}

#[derive(Debug, Clone)]
pub struct SearchMatchAnalysis {
    pub explain: SearchExplain,
    pub fallback_score: i64,
}

pub fn build_search_meta(
    query: &str,
    normalized_terms: &[String],
    backend: SearchBackend,
    query_mode: SearchQueryMode,
    ranking: &'static str,
) -> SearchMeta {
    let trimmed = query.trim();
    let normalized_query = if normalized_terms.is_empty() {
        None
    } else {
        let joined = normalized_terms.join(" ");
        if query_mode == SearchQueryMode::Expanded || joined != trimmed {
            Some(joined)
        } else {
            None
        }
    };

    SearchMeta {
        backend,
        query_mode,
        ranking,
        normalized_query,
        normalized_terms: normalized_terms.to_vec(),
    }
}

pub fn analyze_match(
    query: &str,
    normalized_terms: &[String],
    fields: &[SearchField<'_>],
    rank: usize,
) -> SearchMatchAnalysis {
    let trimmed = query.trim();
    let lower_query = trimmed.to_lowercase();
    let term_count = normalized_terms.len();
    let lower_terms = normalized_terms
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();

    let mut matched_fields = Vec::new();
    let mut matched_terms = 0;
    let mut literal_match = false;
    let mut field_score = 0;

    for field in fields {
        let Some(text) = field.text else {
            continue;
        };
        let lower_text = text.to_lowercase();
        let field_literal_match = !lower_query.is_empty() && lower_text.contains(&lower_query);
        let field_term_matches = lower_terms
            .iter()
            .filter(|term| lower_text.contains(term.as_str()))
            .count();

        if field_literal_match || field_term_matches > 0 {
            matched_fields.push(field.name.to_string());
            field_score += field.weight;
        }

        literal_match |= field_literal_match;
    }

    if term_count > 0 {
        matched_terms = lower_terms
            .iter()
            .filter(|term| {
                fields.iter().any(|field| {
                    field
                        .text
                        .map(|text| text.to_lowercase().contains(term.as_str()))
                        .unwrap_or(false)
                })
            })
            .count();
    } else if literal_match {
        matched_terms = 1;
    }

    let full_term_coverage_bonus = if term_count > 0 && matched_terms == term_count {
        2_000
    } else {
        0
    };
    let literal_bonus = if literal_match { 500 } else { 0 };
    let fallback_score =
        full_term_coverage_bonus + literal_bonus + (matched_terms as i64 * 100) + field_score;

    SearchMatchAnalysis {
        explain: SearchExplain {
            rank,
            matched_fields,
            matched_terms,
            literal_match,
        },
        fallback_score,
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze_match, SearchField};

    #[test]
    fn fallback_scoring_prefers_more_complete_term_coverage() {
        let exact = analyze_match(
            "CLI/search",
            &[String::from("CLI"), String::from("search")],
            &[SearchField {
                name: "text",
                text: Some("Please build a CLI search flow"),
                weight: 30,
            }],
            1,
        );
        let partial = analyze_match(
            "CLI/search",
            &[String::from("CLI"), String::from("search")],
            &[SearchField {
                name: "text",
                text: Some("Please build a CLI flow"),
                weight: 30,
            }],
            1,
        );

        assert!(exact.fallback_score > partial.fallback_score);
        assert_eq!(exact.explain.matched_terms, 2);
        assert_eq!(partial.explain.matched_terms, 1);
    }
}
