use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Rendered {
    pub text: String,
    pub json: Value,
}

impl Rendered {
    pub fn new<T>(text: String, value: &T) -> Result<Self>
    where
        T: Serialize,
    {
        Ok(Self {
            text,
            json: serde_json::to_value(value)?,
        })
    }
}

pub fn excerpt(text: &str, query: &str, width: usize) -> String {
    let normalized = text.trim();
    if normalized.is_empty() {
        return String::new();
    }

    let lower_text = normalized.to_lowercase();
    let query_terms = query
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();

    let start = query_terms
        .iter()
        .find_map(|term| lower_text.find(term))
        .unwrap_or(0);
    let end = start.saturating_add(width).min(normalized.len());
    let slice = normalized.get(start..end).unwrap_or(normalized);

    if start > 0 || end < normalized.len() {
        format!("...{}...", slice)
    } else {
        slice.to_string()
    }
}
