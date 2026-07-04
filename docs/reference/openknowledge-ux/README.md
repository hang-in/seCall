# OpenKnowledge UI/UX 참고 명세 — 인덱스

> **목적**: OpenKnowledge(OK) v0.24.0의 웹 UI/UX 중 seCall 웹UI에 차용할 만한 패턴을 정리. seCall(AGPL)에서 **이 명세만 보고 독립 구현**하기 위한 참고.
> **작성 방식(클린룸)**: OK를 로컬 실행해 **동작하는 UI를 직접 관찰** + Inkeep 자체 기능 문서 근거. **미니파이 소스 비복제** — 기능·레이아웃·인터랙션(비저작권 대상)만 기술. (OK=GPL-3.0, seCall=AGPL-3.0; UI 패턴·아이디어는 저작권 대상 아님)
> **관찰일**: 2026-07-04.

## 문서 분류

| # | 파일 | 내용 |
|---|------|------|
| 01 | [layout-navigation](01-layout-navigation.md) | 3-pane 레이아웃 · 좌측 파일트리 · 탭 · ⌘K 커맨드 팔레트 |
| 02 | [editor](02-editor.md) | WYSIWYG 블록 에디터 · 슬래시 메뉴 · `[[위키링크]]` 자동완성 · Markdown 소스 모드 · 상태바 |
| 03 | [rich-rendering-artifacts](03-rich-rendering-artifacts.md) | **★최대 하이라이트** — `html preview` 라이브 아티팩트 · Callout · Mermaid · KaTeX · embed |
| 04 | [context-panel](04-context-panel.md) | 우측 패널: Outline · Links(백링크) · Graph · Timeline |
| 05 | [folders-templates](05-folders-templates.md) | 폴더 뷰 · 폴더 속성 · 템플릿 · 활동 |
| 06 | [ai-integration](06-ai-integration.md) | Create/Open with Claude → **Claude Code 핸드오프(MCP)** |
| 07 | [collab-versioning-sharing](07-collab-versioning-sharing.md) | CRDT · 버전 타임라인 · Publish to GitHub |
| 08 | [settings](08-settings.md) | 설정 구조(USER / THIS PROJECT) |

## 한 줄 요약

OK = **"파일트리 + 3-pane WYSIWYG 에디터 + 우측 컨텍스트 패널 + ⌘K 팔레트 + 리치 렌더링(아티팩트) + Claude Code 통합"**. seCall이 "검색+세션 브라우징" 중심이라면, OK 강점은 **문서를 편집·연결·시각화하는 워크스페이스 + LLM이 만든 위키를 아티팩트로 렌더**하는 경험임.

## seCall 적용 우선순위

| 우선순위 | 항목 | 문서 | 이유 |
|---|---|---|---|
| **P0★** | 리치 렌더링 / 라이브 아티팩트(`html preview` iframe+테마토큰·Callout·Mermaid·KaTeX) | 03 | 사용자가 "미려하다"고 한 그것. LLM 생성 위키의 최대 임팩트 |
| **P0** | WYSIWYG 블록 에디터 + `[[wikilink]]` 실시간 렌더·자동완성 | 02 | 웹UI 대비 최대 격차 |
| **P0** | 우측 **Links(백링크)** 패널 | 04 | 지식 연결 가시화 |
| **P1** | ⌘K 통합 팔레트(파일·명령·태그·AI) | 01 | seCall 검색 강점 재활용, ROI 높음 |
| **P1** | 3-pane 레이아웃 + 탭 | 01 | 위 기능들의 그릇 |
| **P1** | 로컬 그래프(우패널) | 04 | seCall 그래프 재사용 |
| **P2** | 폴더 속성/템플릿/활동 | 05 | 구조화 강력, 범위 큼 |
| **P2** | AI 생성 진입점 (Claude Code) | 06 | seCall AI·세션 자원과 시너지 |
| **P2** | 아웃라인 · 토큰카운트 상태바 | 02,04 | 저비용 개선 |
| **P3** | Timeline 버전 UI · Publish/Share | 07 | git/동기화 위 UI |
| 보류 | Skills 트리 · CRDT 실시간 동시편집 | — | seCall 맥락에 과할 수 있음 |

## 관찰한 화면 (근거)
홈/생성 · 폴더뷰 · WYSIWYG에디터 · 슬래시메뉴 · `[[`자동완성 · Markdown소스모드 · 아티팩트렌더(콜아웃/Mermaid/html preview/KaTeX) · Outline/Links/Graph/Timeline · ⌘K팔레트 · 설정 · Share(Publish to GitHub).

## 아직 미관찰
이미지/자산 붙여넣기 실제 플로우 · Tabs/Accordion 렌더 · palette 컴포넌트 스키마 브라우저(UI 존재 불명, MCP 도구로는 존재) · Create-with-Claude 생성 결과물(사용자 확인: Claude Code 로딩됨).
