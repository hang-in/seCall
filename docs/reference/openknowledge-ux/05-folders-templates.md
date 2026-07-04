# 05. 폴더 · 템플릿

폴더를 탭으로 열면 단순 디렉터리가 아니라 **메타데이터 + 템플릿 + 활동 + 문서 테이블**을 가진 컨테이너로 표시됨.

## 5.1 폴더 뷰 구성
- **폴더 헤더** + `+ NEW`(문서 생성).
- **FOLDER PROPERTIES**: 폴더 frontmatter(title/description/tags 등) — `+ Add a property`. 경로 `<folder>/.ok/frontmatter.yml`. *self-only, 하위로 cascade 안 함.*
- **TEMPLATES AVAILABLE(n)**: `+ NEW TEMPLATE` — **폴더별 문서 템플릿**(이 폴더의 새 문서가 이 템플릿으로 시작).
- **ACTIVITY**: 최근 변경 피드.
- **문서 테이블**: `NAME ↑` / `MODIFIED ↕` 정렬.

## 5.2 스타터 팩 (seed)
- `ok seed --pack <name>`으로 폴더+템플릿 구조를 스캐폴드. 팩 예: knowledge-base(신뢰 아티클) · software-lifecycle(제안·결정·스펙) · codebase-wiki · plain-notes · writing-pipeline · entity-vault(개인 CRM) 등.
- **시사점**: 도메인별 "폴더+템플릿 프리셋"으로 빈 화면 공포 제거.

## 5.3 seCall 적용 (P2)
- 폴더 = frontmatter + 템플릿 목록 + 문서 테이블(정렬) + 활동.
- seCall이 "프로젝트/에이전트/토픽별 폴더"에 속성·템플릿을 얹으면 구조화 강력해짐. 단 범위 크므로 P2.
