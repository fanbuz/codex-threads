use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};
use std::time::Duration;

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

    pub fn with_duration(mut self, duration: Duration) -> Self {
        let duration_ms = duration.as_millis() as u64;
        let json = std::mem::take(&mut self.json);
        let mut payload = match json {
            Value::Object(map) => map,
            other => {
                let mut map = Map::new();
                map.insert("data".to_string(), other);
                map
            }
        };
        payload.insert("duration_ms".to_string(), Value::from(duration_ms));
        self.json = Value::Object(payload);

        if self.text.is_empty() {
            self.text = format!("耗时: {}", format_duration(duration));
        } else {
            self.text.push('\n');
            self.text
                .push_str(&format!("耗时: {}", format_duration(duration)));
        }

        self
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

fn format_duration(duration: Duration) -> String {
    let duration_ms = duration.as_millis();
    if duration_ms < 1_000 {
        return format!("{}ms", duration_ms);
    }

    let seconds = duration.as_secs_f64();
    let mut formatted = format!("{seconds:.2}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    format!("{formatted}s")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::format_duration;

    #[test]
    fn formats_subsecond_duration_as_milliseconds() {
        assert_eq!(format_duration(Duration::from_millis(250)), "250ms");
    }

    #[test]
    fn formats_second_scale_duration_as_trimmed_seconds() {
        assert_eq!(format_duration(Duration::from_millis(1_000)), "1s");
        assert_eq!(format_duration(Duration::from_millis(1_230)), "1.23s");
        assert_eq!(format_duration(Duration::from_millis(12_500)), "12.5s");
    }
}
