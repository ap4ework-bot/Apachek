use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub category: String,
    pub tags: Vec<String>,
    pub connections: usize,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Edge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub weight: f32,
}

#[derive(Debug, Serialize)]
pub struct SpaceData {
    pub nodes: Vec<Node>,
    pub links: Vec<Edge>,
}

#[derive(Debug, Serialize)]
pub struct Space {
    pub name: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub colors: HashMap<String, String>,
    pub data: SpaceData,
}

pub fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || "-_:/.".contains(c) { c } else { '_' })
        .collect()
}

pub fn dna_prefix(dna: &str) -> String {
    sanitize_id(&dna.chars().take(30).collect::<String>())
}

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max { return s; }
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_id_keeps_allowed_chars() {
        assert_eq!(sanitize_id("kei-fork::v1_2/test.md"), "kei-fork::v1_2/test.md");
    }

    #[test]
    fn sanitize_id_replaces_disallowed_chars() {
        assert_eq!(sanitize_id("hello world!"), "hello_world_");
        assert_eq!(sanitize_id("a@b#c"), "a_b_c");
    }

    #[test]
    fn sanitize_id_empty_string_is_empty() {
        assert_eq!(sanitize_id(""), "");
    }

    #[test]
    fn dna_prefix_truncates_then_sanitizes() {
        let long = "a".repeat(40);
        assert_eq!(dna_prefix(&long), "a".repeat(30));
    }

    #[test]
    fn dna_prefix_sanitizes_after_truncating() {
        // 30-char take happens first, then disallowed chars get replaced.
        let s = format!("{}!!!!!", "x".repeat(30));
        assert_eq!(dna_prefix(&s), "x".repeat(30));
    }

    #[test]
    fn truncate_chars_shorter_than_max_is_unchanged() {
        assert_eq!(truncate_chars("hi", 10), "hi");
    }

    #[test]
    fn truncate_chars_exact_length_is_unchanged() {
        assert_eq!(truncate_chars("hello", 5), "hello");
    }

    #[test]
    fn truncate_chars_cuts_at_char_boundary_not_byte() {
        // Each of these is a multi-byte UTF-8 char; a naive byte-slice
        // truncation (&s[..max]) would panic mid-codepoint here.
        let s = "日本語テスト";
        assert_eq!(truncate_chars(s, 3), "日本語");
        assert_eq!(s.chars().count(), 6);
    }

    #[test]
    fn truncate_chars_max_zero_is_empty() {
        assert_eq!(truncate_chars("anything", 0), "");
    }
}
