const EDGE_PUNCTUATION: &[char] = &[
    '"', '\'', '`', '(', ')', '[', ']', '{', '}', '<', '>', ',', '，', ';', '；', ':', '：', '!',
    '！', '?', '？', '.', '。',
];

pub(crate) fn normalize_query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .flat_map(|chunk| chunk.split([',', '，', ';', '；']))
        .filter_map(normalize_query_segment)
        .collect()
}

fn normalize_query_segment(segment: &str) -> Option<String> {
    let trimmed = segment.trim_matches(|ch| EDGE_PUNCTUATION.contains(&ch));
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
