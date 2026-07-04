# 07. 협업 · 버전 · 공유

## 7.1 CRDT 협업
- OK는 **markdown-CRDT** 기반 → 다중 클라이언트 동시편집 + 충돌 병합. (에이전트 편집도 CRDT/shadow-repo로 attribution 남김.)
- 네이티브 파일 편집(외부 에디터/직접 write)도 파일와처가 CRDT로 흡수.

## 7.2 버전 (Timeline)
- 우측 **Timeline** 패널 = 문서별 버전 히스토리. 체크포인트(named version) + restore(롤백).
- **구현**: seCall이 git 동기화라 히스토리 존재 → 문서별 타임라인 UI + 체크포인트/복원.

## 7.3 공유 = Publish to GitHub
- 우상단 **Share** → "Publish to GitHub" 다이얼로그:
  - Owner(GitHub 계정) · **Repository name**(프로젝트명 프리필) · **Visibility(Private/Public)** · Description(optional) · PUBLISH.
- 문서 공유엔 GitHub repo가 필요 → repo 생성 후 git-substrate 기반 공유 링크(`share_link`).
- **주의(운영)**: PUBLISH는 실제 GitHub repo를 생성·발행하는 **외부 side-effect**. 데모/탐색 중엔 CANCEL.

## 7.4 seCall 적용 (P3)
- seCall은 이미 git 동기화 → **버전 타임라인 UI + 공유 링크**만 얹으면 됨. CRDT 실시간 동시편집은 단독 사용 위주면 보류.
