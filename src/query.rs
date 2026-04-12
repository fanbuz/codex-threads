pub(crate) fn normalize_query_terms(query: &str) -> Vec<String> {
    query
        .trim()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}
