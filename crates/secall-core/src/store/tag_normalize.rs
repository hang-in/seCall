//! 태그 정규화 — REST PATCH 핸들러와 insert 경로에서 공유.

const MAX_TAG_LEN: usize = 32;

pub fn normalize_tag(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    let replaced: String = lower
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    replaced.chars().take(MAX_TAG_LEN).collect()
}

pub fn normalize_tags(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|s| normalize_tag(s))
        .filter(|s| !s.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase() {
        assert_eq!(normalize_tag("Rust"), "rust");
    }

    #[test]
    fn whitespace_to_dash() {
        assert_eq!(normalize_tag("hello world"), "hello-world");
    }

    #[test]
    fn truncates_to_32() {
        let long = "a".repeat(50);
        assert_eq!(normalize_tag(&long).len(), 32);
    }

    #[test]
    fn strips_illegal_chars() {
        assert_eq!(normalize_tag("rust!@#$"), "rust");
    }

    #[test]
    fn deduplicates() {
        let tags = vec!["rust".into(), "Rust".into(), "RUST".into()];
        assert_eq!(normalize_tags(&tags), vec!["rust"]);
    }

    #[test]
    fn empty_filtered_out() {
        let tags = vec!["".into(), "  ".into(), "rust".into()];
        assert_eq!(normalize_tags(&tags), vec!["rust"]);
    }

    #[test]
    fn underscore_and_dash_kept() {
        assert_eq!(normalize_tag("foo_bar-baz"), "foo_bar-baz");
    }
}
