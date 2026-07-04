# 08. 설정 구조

설정 모달은 좌측 나비 2계층(USER / THIS PROJECT).

## 8.1 USER
- **Preferences**:
  - **Theme**: Light / Dark / System.
  - **Word wrap**(Markdown 소스 에디터 줄바꿈) 토글.
  - **Open preview when agent edits**: 에이전트 편집 시 프리뷰 자동 갱신 토글. (자체 프리뷰 창 관리 시 끔.)
  - **Attachments**: 새 첨부(붙여넣기/드롭) 저장 위치 — "Same folder as current file" 등.
- **Hotkeys**: 단축키 커스텀.
- **Account**.

## 8.2 THIS PROJECT
- **Sync**(git 동기화) · **Search** · **Templates** · **Skills** · **Ignore patterns**(`.okignore`) · **Config sharing**(OK 설정을 콘텐츠와 함께 커밋할지 = shared, 아니면 `.git/info/exclude`로 로컬 전용).

## 8.3 seCall 적용
- 설정을 **유저(테마·단축키·프리뷰)** vs **프로젝트(동기화·검색·ignore·템플릿)** 2계층으로 분리.
- `.okignore` 식 **콘텐츠 스코프 제외 패턴** + Config sharing(커밋 vs 로컬) 개념 참고.
