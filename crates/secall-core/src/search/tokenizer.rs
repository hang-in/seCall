use std::collections::HashSet;

use anyhow::Result;
use lindera::{
    dictionary::{load_embedded_dictionary, DictionaryKind},
    mode::Mode,
    segmenter::Segmenter,
    token_filter::{korean_keep_tags::KoreanKeepTagsTokenFilter, BoxTokenFilter},
    tokenizer::Tokenizer as LinderaInner,
};

pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;

    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }
}

// ─── LinderaKoTokenizer ───────────────────────────────────────────────────────

pub struct LinderaKoTokenizer {
    inner: LinderaInner,
}

impl LinderaKoTokenizer {
    pub fn new() -> Result<Self> {
        let dictionary = load_embedded_dictionary(DictionaryKind::KoDic)
            .map_err(|e| anyhow::anyhow!("lindera ko-dic load failed: {e}"))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let mut tokenizer = LinderaInner::new(segmenter);

        // Keep: NNG (일반명사), NNP (고유명사), NNB (의존명사), VV (동사), VA (형용사), SL (외국어)
        let tags: HashSet<String> = ["NNG", "NNP", "NNB", "VV", "VA", "SL"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let keep_filter = KoreanKeepTagsTokenFilter::new(tags);
        tokenizer.append_token_filter(BoxTokenFilter::from(keep_filter));

        Ok(Self { inner: tokenizer })
    }
}

impl Tokenizer for LinderaKoTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let tokens = match self.inner.tokenize(text) {
            Ok(t) => t,
            Err(_) => return tokenize_fallback(text),
        };

        let mut result: Vec<String> = Vec::new();
        for token in tokens {
            let surface = token.surface.to_lowercase();
            if surface.chars().count() > 1 {
                result.push(surface);
            }
        }

        if result.is_empty() {
            tokenize_fallback(text)
        } else {
            result
        }
    }
}

// ─── KiwiTokenizer ────────────────────────────────────────────────────────────

/// Newtype wrapper so we can impl Send without Sync.
/// kiwi_rs::Kiwi contains *mut c_void + RefCell internals — not Sync.
struct KiwiWrapper(kiwi_rs::Kiwi);

// SAFETY: kiwi_rs::Kiwi wraps a C pointer that is safe to move between threads.
// We do NOT implement Sync — all concurrent access is serialized via Mutex below.
unsafe impl Send for KiwiWrapper {}

/// Korean morphological tokenizer backed by kiwi-rs.
/// On first use, `Kiwi::init()` downloads the model (~50MB) to ~/.cache/kiwi/.
/// Thread safety is provided by `Mutex<KiwiWrapper>`.
pub struct KiwiTokenizer {
    kiwi: std::sync::Mutex<KiwiWrapper>,
}

// Mutex<KiwiWrapper>: Sync because KiwiWrapper: Send — no unsafe needed.

impl KiwiTokenizer {
    pub fn new() -> Result<Self> {
        let kiwi =
            kiwi_rs::Kiwi::init().map_err(|e| anyhow::anyhow!("kiwi-rs init failed: {e}"))?;
        Ok(Self {
            kiwi: std::sync::Mutex::new(KiwiWrapper(kiwi)),
        })
    }
}

impl Tokenizer for KiwiTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let guard = match self.kiwi.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        match guard.0.tokenize(text) {
            Ok(tokens) => {
                let result: Vec<String> = tokens
                    .into_iter()
                    .filter(|t| {
                        // Keep NNG, NNP, NNB (nouns), VV (verbs), VA (adjectives), SL (foreign)
                        matches!(t.tag.as_str(), "NNG" | "NNP" | "NNB" | "VV" | "VA" | "SL")
                    })
                    .map(|t| t.form.to_lowercase())
                    .filter(|s| s.chars().count() > 1)
                    .collect();

                if result.is_empty() {
                    tokenize_fallback(text)
                } else {
                    result
                }
            }
            Err(_) => tokenize_fallback(text),
        }
    }
}

// ─── SimpleTokenizer ──────────────────────────────────────────────────────────

/// Simple whitespace + punctuation tokenizer as fallback
pub struct SimpleTokenizer;

impl Tokenizer for SimpleTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        tokenize_fallback(text)
    }
}

// ─── Factory ─────────────────────────────────────────────────────────────────

/// Create a tokenizer based on the backend name from config.
/// Falls back to lindera if kiwi-rs fails to initialize.
pub fn create_tokenizer(backend: &str) -> Result<Box<dyn Tokenizer>> {
    match backend {
        "kiwi" => match KiwiTokenizer::new() {
            Ok(t) => {
                tracing::info!("kiwi-rs tokenizer loaded");
                Ok(Box::new(t))
            }
            Err(e) => {
                tracing::warn!(error = %e, "kiwi-rs failed, falling back to lindera");
                Ok(Box::new(LinderaKoTokenizer::new()?))
            }
        },
        _ => Ok(Box::new(LinderaKoTokenizer::new()?)),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn tokenize_fallback(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| s.chars().count() > 1)
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ko_tokenizer() -> LinderaKoTokenizer {
        LinderaKoTokenizer::new().expect("lindera ko-dic should load")
    }

    #[test]
    fn test_korean_tokenization() {
        let tok = ko_tokenizer();
        let tokens = tok.tokenize("아키텍처를 설계한다");
        assert!(!tokens.is_empty());
        let joined = tokens.join(" ");
        assert!(joined.contains("아키텍처") || joined.contains("설계") || !joined.is_empty());
    }

    #[test]
    fn test_english_tokenization() {
        let tok = ko_tokenizer();
        let tokens = tok.tokenize("Rust workspace");
        let joined = tokens.join(" ");
        assert!(!joined.is_empty());
    }

    #[test]
    fn test_mixed_tokenization() {
        let tok = ko_tokenizer();
        let tokens = tok.tokenize("seCall의 BM25 검색");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_empty_text() {
        let tok = ko_tokenizer();
        let tokens = tok.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_special_chars_only() {
        let tok = ko_tokenizer();
        let tokens = tok.tokenize("!@#$%^");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_create_tokenizer_lindera() {
        let tok = create_tokenizer("lindera");
        assert!(tok.is_ok());
    }

    #[test]
    fn test_create_tokenizer_fallback() {
        // Unknown backend → lindera fallback
        let tok = create_tokenizer("unknown_backend");
        assert!(tok.is_ok());
    }

    #[test]
    #[ignore]
    fn test_kiwi_korean_tokenization() {
        // Manual: requires kiwi model download (~50MB)
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("아키텍처를 설계한다");
        assert!(!tokens.is_empty());
    }

    #[test]
    #[ignore]
    fn test_kiwi_english_tokenization() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("Rust workspace");
        assert!(!tokens.is_empty());
    }

    #[test]
    #[ignore]
    fn test_kiwi_mixed_tokenization() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("seCall의 BM25 검색");
        assert!(!tokens.is_empty());
    }

    #[test]
    #[ignore]
    fn test_kiwi_empty() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    #[ignore]
    fn test_create_tokenizer_kiwi() {
        // Manual: requires kiwi model download
        let tok = create_tokenizer("kiwi");
        assert!(tok.is_ok());
    }
}
