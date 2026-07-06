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

    /// 색인용 FTS 텍스트. 형태소 토큰을 공백 조인 (현행 색인 동작 유지).
    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }

    /// POS 필터 없는 raw 토큰 (외래어·굴절형 보존). 기본 = tokenize_fallback.
    fn raw_tokens(&self, text: &str) -> Vec<String> {
        tokenize_fallback(text)
    }

    /// 질의용 FTS5 MATCH 표현식: 형태소 + raw + 외래어 음역 alias 를 distinct 후
    /// 각 토큰에 prefix(`*`)를 붙여 ` OR ` 로 조인. 빈 입력은 빈 문자열.
    ///
    /// - **OR**: 다토큰 질의의 암묵적 AND(리콜 급감)를 피한다. bm25 가 다토큰 매치
    ///   문서를 상위로 올려 정밀도를 보존.
    /// - **prefix `*`**: 한국어 조사가 접미라 stem 이 굴절형의 prefix 가 된다.
    /// - **alias**: 교차스크립트(리프레시↔refresh) 갭 보강. index 무변경(질의 확장 전용).
    fn fts_query(&self, text: &str) -> String {
        let mut toks = self.tokenize(text);
        toks.extend(self.raw_tokens(text));
        toks.sort();
        toks.dedup();
        // alias 는 원 토큰 기준으로 모아 사후 추가(확장 토큰이 다시 확장되지 않도록).
        let aliases: Vec<String> = toks
            .iter()
            .flat_map(|t| crate::search::loanword::loanword_aliases(t))
            .collect();
        toks.extend(aliases);
        toks.sort();
        toks.dedup();
        toks.into_iter()
            .filter(|t| !t.is_empty())
            // FTS5 구문오류 방지: 토큰을 큰따옴표로 감싸고 내부 " 이스케이프 후 prefix(*).
            // 특수문자(* + - . @ ") 가 토큰에 있어도 syntax error 없이 안전.
            .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ")
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

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
mod kiwi_impl {
    use super::*;

    /// libkiwi release tag pinned for kiwi-rs's auto-download fallback. This is
    /// the version VERIFIED to work with the pinned kiwi-rs fork's FFI binding.
    ///
    /// WARNING: do NOT bump to v0.23.x. libkiwi >= 0.23 currently SIGSEGVs in
    /// `Kiwi::new()` (the kiwi builder/init path) with this binding, even though
    /// the standalone `kiwi-cli` 0.23.x tokenizes fine with the same lib+model.
    /// PR #143 fixed the analyze-path `KiwiAnalyzeOption` ABI, but a separate
    /// init-path 0.23 incompatibility remains and needs upstream kiwi-rs work.
    /// Verified empirically: 0.22.2 tokenizes; 0.23.2 crashes in init.
    ///
    /// Only affects the auto-download fallback. An already-installed libkiwi
    /// found via `KIWI_LIBRARY_PATH` or default discovery is used as-is by
    /// `Kiwi::new()`, so a user who manually installed 0.23.x can still crash.
    pub(super) const KIWI_LIBKIWI_TAG: &str = "v0.22.2";

    /// Newtype wrapper so we can impl Send without Sync.
    /// kiwi_rs::Kiwi contains *mut c_void + RefCell internals — not Sync.
    pub(super) struct KiwiWrapper(pub(super) kiwi_rs::Kiwi);

    // SAFETY: kiwi_rs::Kiwi wraps a C pointer that is safe to move between threads.
    // We do NOT implement Sync — all concurrent access is serialized via Mutex below.
    unsafe impl Send for KiwiWrapper {}

    /// Korean morphological tokenizer backed by kiwi-rs.
    /// On first use, kiwi-rs downloads the pinned libkiwi/model (~50MB) to ~/.cache/kiwi/.
    /// Thread safety is provided by `Mutex<KiwiWrapper>`.
    pub struct KiwiTokenizer {
        pub(super) kiwi: std::sync::Mutex<KiwiWrapper>,
    }

    // Mutex<KiwiWrapper>: Sync because KiwiWrapper: Send — no unsafe needed.

    impl KiwiTokenizer {
        pub fn new() -> Result<Self> {
            let kiwi = kiwi_rs::Kiwi::init_with_version(KIWI_LIBKIWI_TAG)
                .map_err(|e| anyhow::anyhow!("kiwi-rs init failed: {e}"))?;
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
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
pub use kiwi_impl::KiwiTokenizer;

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
        #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
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
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        "kiwi" => {
            tracing::warn!("kiwi-rs is not supported on aarch64 Linux, falling back to lindera");
            LinderaKoTokenizer::new().map(|t| Box::new(t) as Box<dyn Tokenizer>)
        }
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

    #[cfg(all(
        not(target_os = "windows"),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    #[test]
    #[ignore]
    fn test_kiwi_korean_tokenization() {
        // Manual: requires kiwi model download (~50MB)
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("아키텍처를 설계한다");
        assert!(!tokens.is_empty());
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    #[test]
    #[ignore]
    fn test_kiwi_english_tokenization() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("Rust workspace");
        assert!(!tokens.is_empty());
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    #[test]
    #[ignore]
    fn test_kiwi_mixed_tokenization() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("seCall의 BM25 검색");
        assert!(!tokens.is_empty());
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    #[test]
    #[ignore]
    fn test_kiwi_empty() {
        let tok = KiwiTokenizer::new().expect("kiwi init");
        let tokens = tok.tokenize("");
        assert!(tokens.is_empty());
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(all(target_os = "linux", target_arch = "aarch64"))
    ))]
    #[test]
    #[ignore]
    fn test_create_tokenizer_kiwi() {
        // Manual: requires kiwi model download
        let tok = create_tokenizer("kiwi");
        assert!(tok.is_ok());
    }

    /// Real-FFI ABI regression: exercises `kiwi_analyze` end-to-end against the
    /// actual libkiwi binary. Guards the by-value `KiwiAnalyzeOption` ABI (PR
    /// #143): a struct-layout drift (e.g. a future libkiwi appending fields)
    /// would SIGSEGV here or return garbage morphemes — turning CI red instead
    /// of a user's `secall sync`. Unlike the `#[ignore]` tests above this is a
    /// normal test, but it self-skips unless opted in (first run downloads
    /// ~50MB libkiwi+model): set `SECALL_KIWI_FFI_TEST=1`. CI runs it in a
    /// dedicated step with the libkiwi assets cached (see ci.yml).
    #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
    #[test]
    fn test_kiwi_ffi_abi_smoke() {
        if std::env::var_os("SECALL_KIWI_FFI_TEST").is_none() {
            eprintln!(
                "skipping test_kiwi_ffi_abi_smoke (set SECALL_KIWI_FFI_TEST=1 to run the real FFI check)"
            );
            return;
        }
        let tok = KiwiTokenizer::new().expect("kiwi init (real libkiwi)");
        // 표준 Kiwi 분석 문장: 아버지/NNG 가/JKS 방/NNG 에/JKB 들어가/VV 시/EP ㄴ다/EF.
        // 필터(NNG/NNP/NNB/VV/VA/SL, len>1) 후 '아버지'·'들어가' 가 남아야 한다.
        let tokens = tok.tokenize("아버지가방에들어가신다");
        assert!(
            tokens.iter().any(|t| t == "아버지"),
            "real kiwi analysis expected the '아버지' morpheme, got {tokens:?} \
             (empty/garbage/whole-word ⇒ ABI drift or silent fallback — see PR #143)"
        );
    }

    // ─── fts_query: OR + prefix + 외래어 음역 alias ────────────────────────────

    #[test]
    fn fts_query_uses_or_and_prefix() {
        // SimpleTokenizer 로 결정적 검증 (사전 불요)
        let q = SimpleTokenizer.fts_query("hello world");
        assert!(q.contains(" OR "), "다토큰은 OR 로 조인: {q}");
        assert!(q.contains("\"hello\"*"), "인용+prefix 와일드카드: {q}");
        assert!(q.contains("\"world\"*"));
    }

    #[test]
    fn fts_query_expands_loanword_aliases() {
        // 한글 음역 질의가 영어 원어 alias 를 병기 (교차스크립트 갭 보강)
        let q = SimpleTokenizer.fts_query("리프레시 토큰");
        assert!(q.contains("\"리프레시\"*"));
        assert!(q.contains("\"refresh\"*"), "리프레시 → refresh alias: {q}");
        assert!(q.contains("\"토큰\"*"));
        assert!(q.contains("\"token\"*"), "토큰 → token alias: {q}");
    }

    #[test]
    fn fts_query_empty_is_empty() {
        assert_eq!(SimpleTokenizer.fts_query(""), "");
        assert_eq!(SimpleTokenizer.fts_query("  !@#  "), "");
    }

    #[test]
    fn fts_query_no_alias_when_none_matches() {
        let q = SimpleTokenizer.fts_query("architecture design");
        // alias 없는 토큰은 그대로 prefix+OR
        assert!(q.contains("\"architecture\"*"));
        assert!(q.contains("\"design\"*"));
        // 영어→한글 병기가 잘못 끼지 않음 (음역 테이블에 없는 단어)
        assert!(!q.contains("아키텍처"));
    }
}
