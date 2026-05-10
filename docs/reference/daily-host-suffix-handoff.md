---
type: reference
status: in_progress
updated_at: 2026-05-09
---

# Daily Log Host Suffix — Mac 핸드오프

## 배경

`secall log <DATE>` 명령은 vault `log/<DATE>.md` 형식으로 일별 작업 일지를 생성했다. 그런데 vault git이 멀티 머신 동기화 대상이라 **Mac과 Windows에서 같은 날짜의 daily 를 만들면 동일 파일명 → conflict 100% 발생**. session 파일은 host별로 unique 하지만 daily 는 그렇지 않았음.

## 패치 (Windows에서 완료, 2026-05-09)

`crates/secall/src/commands/log.rs:170` — 파일명에 host suffix 추가:

```rust
let host = gethostname::gethostname()
    .to_string_lossy()
    .split('.')
    .next()
    .unwrap_or("unknown")
    .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
let log_path = log_dir.join(format!("{}--{}.md", target_date, host));
```

새 형식: `log/2026-04-22--<host>.md`
- Windows 기준: `log/2026-04-22--사자.md`
- Mac 기준: `log/2026-04-22--dong-guui-MacBookAir.md`

split('.').next()로 도메인 suffix(`.local` 등) 제거. 윈도우 파일시스템 금지문자 sanitize.

## Windows에서 한 작업

1. ✅ 코드 패치 + cargo install
2. ✅ vault에 65일치 daily backfill (Windows host suffix 적용)
3. ✅ vault git push
4. ✅ secall repo push (코드 + 본 문서)

생성된 daily는 Windows host 기준 통합본 — Mac+Windows 양쪽 세션 모두 reindex된 상태에서 정리됨. 따라서 Mac에서 같은 날짜를 다시 만들면 **거의 같은 입력에 대한 다른 LLM 텍스트**가 됨.

## Mac에서 해야 할 일

### 1. secall pull + 빌드

```bash
cd ~/path/to/seCall
git pull origin main           # 본 패치 가져옴
cargo install --path crates/secall --force
secall --version              # 0.4.x 또는 새 버전 확인
```

### 2. vault pull + legacy 파일 정리

```bash
cd ~/path/to/obsidian-vault
git pull origin main
ls log/                       # Windows host suffix 적용된 daily 확인
```

기존 legacy `log/2026-04-13.md` 는 Windows에서 삭제 commit 했음 (host suffix 없는 형식 정리). Mac에서 다시 필요하면 `secall log 2026-04-13` 실행 → `log/2026-04-13--<mac-host>.md` 생성.

### 3. (선택) Mac 측 daily 생성

Windows 백필이 양 호스트 세션 모두 반영한 통합본이라 **굳이 Mac에서 또 만들 필요는 없음**. 다만 LLM 비결정성으로 다른 시각의 정리를 원하면:

```bash
secall log 2026-04-22         # 단일 일자
# 또는 일괄 backfill 스크립트 (Windows 측 daily_backfill.ps1 참고)
```

생성 결과는 `log/2026-04-22--dong-guui-MacBookAir.md` 형태. Windows 결과와 공존.

### 4. config 확인

`secall log` 는 config의 `[graph]` ollama 모델을 공유한다. Windows 기준 검증된 권장:

```toml
[graph]
semantic_backend = "ollama"
ollama_model = "gemma4:31b-cloud"      # 작업 정리 품질 좋음, 30s/일 (cloud)
```

`ministral-3:14b-cloud` 도 빠르지만 (1분/일) 결과가 표면적. 31b 권장.

## 잡다 메모

- 패치 전에 만들어진 legacy `log/<DATE>.md` 형식은 새 형식과 공존하지 않도록 정리 (Windows에서 commit으로 처리)
- vault git이 머신간 동기화되니 코드 패치는 Windows/Mac 양쪽에 동일하게 빌드 적용해야 함 — 한쪽만 패치하면 옛 동작이 또 충돌 유발
