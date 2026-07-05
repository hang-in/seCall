# Prompts

실행 에이전트(Claude / Codex 등)에게 넘기는 프롬프트 인덱스. 새 프롬프트를 추가하면 여기 등록한다.

> 문서 규약(파일명, frontmatter, index 갱신)은 [../reference/docConventions.md](../reference/docConventions.md) 참고.
> 프롬프트는 원칙적으로 대응하는 [plans](../plans/index.md) 문서와 함께 읽는다.

## 런타임 파이프라인 프롬프트 (Wiki 생성/리뷰)

`secall` wiki 파이프라인이 LLM 백엔드에 주입하는 프롬프트. 코드에서 참조하므로 파일명을 임의로 바꾸지 않는다.

- [wiki-haiku-system.md](wiki-haiku-system.md) — Wiki 생성 시스템 프롬프트 (출력 규칙)
- [wiki-update.md](wiki-update.md) — Wiki 전체 업데이트 프롬프트
- [wiki-incremental.md](wiki-incremental.md) — Wiki 증분 업데이트 프롬프트 (세션 단위)
- [wiki-review.md](wiki-review.md) — Wiki 리뷰 검수 기준
- [wiki-review-strict-json.md](wiki-review-strict-json.md) — Wiki 리뷰 (strict JSON 출력)

## 작업 프롬프트 (날짜별)

일회성 작업 지시문. 위치 규칙: `prompts/YYYY-MM-DD/<short_name>.md`.

- [2026-05-06/web-redesign.md](2026-05-06/web-redesign.md) — secall-web UI/UX 재설계 의뢰 프롬프트 (self-contained) · 상태: ready
