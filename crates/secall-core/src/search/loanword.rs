//! 외래어 음역 병기(loanword transliteration alias) — FTS 질의 확장 전용.
//!
//! 배경: 질의는 한국어 음역("리프레시")인데 관련 발언은 영어 원어("refresh")면,
//! 형태소 FTS 가 두 토큰을 무관하게 취급해 매칭이 0 이 된다. 다국어 임베딩도
//! 이 교차스크립트 갭은 잘 못 잇는다(동의어/의역은 잇지만 음역+코드믹싱은 실패).
//! → 어휘층에서 양방향 alias 를 병기해 해소한다.
//!
//! 원칙:
//! - **음역(transliteration)만** 병기한다. 의역/번역(검색↔search)은 임베딩(벡터 검색)이
//!   담당하는 영역이고, FTS 에 넣으면 noise 가 커진다.
//! - 모호 단음절(풀=pull/pool/grass, 락=lock/rock, 큐=queue/cue)은 오탐 위험이라 제외.
//! - **index 무변경, 질의 확장 전용**(재색인 불요, 모든 토크나이저 백엔드가 공유).
//!
//! (형제 프로젝트 tunaRound 에서 실측·검증 후 seCall 로 역이식. 실코퍼스 FTS
//!  R@5 0.878→0.944, 타깃 질의 0.0→1.0. 대가는 OR 확장에 따른 MRR 소폭 하락.)

/// 개발·설계 도메인 외래어 음역 병기 그룹. 한 그룹 안 토큰은 서로 alias(양방향).
/// 소문자·형태소 표면형 기준.
const LOANWORD_GROUPS: &[&[&str]] = &[
    &["refresh", "리프레시"],
    &["token", "토큰"],
    &["embedding", "임베딩"],
    &["cache", "캐시"],
    &["index", "인덱스"],
    &["commit", "커밋"],
    &["session", "세션"],
    &["branch", "브랜치"],
    &["sandbox", "샌드박스"],
    &["prompt", "프롬프트"],
    &["vector", "벡터"],
    &["rerank", "리랭크", "리랭커"],
    &["bearer", "베어러"],
    &["redis", "레디스"],
    &["cosine", "코사인"],
    &["snapshot", "스냅샷"],
    &["rollback", "롤백"],
    &["endpoint", "엔드포인트"],
    &["timeout", "타임아웃"],
    &["thread", "스레드"],
    &["mutex", "뮤텍스"],
    &["buffer", "버퍼"],
    &["schema", "스키마"],
    &["migration", "마이그레이션"],
    &["pointer", "포인터"],
    &["context", "컨텍스트"],
    &["tokenizer", "토크나이저"],
    &["cursor", "커서"],
    &["keychain", "키체인"],
    &["cookie", "쿠키"],
    &["rotation", "로테이션"],
];

/// 토큰의 외래어 음역 alias 들을 반환한다(자기 자신 제외). 소문자 표면형 기준,
/// 정확 일치만(prefix/부분일치 아님). 없으면 빈 Vec.
pub fn loanword_aliases(token: &str) -> Vec<String> {
    for group in LOANWORD_GROUPS {
        if group.contains(&token) {
            return group
                .iter()
                .filter(|&&t| t != token)
                .map(|s| s.to_string())
                .collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::loanword_aliases;

    #[test]
    fn aliases_are_bidirectional() {
        assert_eq!(loanword_aliases("리프레시"), vec!["refresh".to_string()]);
        assert_eq!(loanword_aliases("refresh"), vec!["리프레시".to_string()]);
    }

    #[test]
    fn multi_member_group_returns_all_others() {
        let mut a = loanword_aliases("rerank");
        a.sort();
        assert_eq!(a, vec!["리랭커".to_string(), "리랭크".to_string()]);
    }

    #[test]
    fn translation_is_excluded() {
        // 번역/의역은 alias 아님 (임베딩 담당)
        assert!(loanword_aliases("검색").is_empty());
        assert!(loanword_aliases("데이터베이스").is_empty());
    }

    #[test]
    fn unknown_token_has_no_alias() {
        assert!(loanword_aliases("asdfqwer").is_empty());
    }
}
