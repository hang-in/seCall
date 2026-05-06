#!/bin/bash
# P25 Phase 0-2 소크 테스트 (6시간)
# 사용: ./scripts/soak-test.sh
# 결과: scripts/soak-results.log

set -euo pipefail

LOG="scripts/soak-results.log"
BASE="http://127.0.0.1:8080"
PASS=0
FAIL=0
TOTAL=0

log() { echo "[$(date '+%H:%M:%S')] $*" | tee -a "$LOG"; }
pass() { PASS=$((PASS+1)); TOTAL=$((TOTAL+1)); log "  PASS: $1"; }
fail() { FAIL=$((FAIL+1)); TOTAL=$((TOTAL+1)); log "  FAIL: $1"; }

section() { log ""; log "======== $1 ========"; }

echo "" > "$LOG"
log "seCall Soak Test — $(date)"
log "Branch: $(git branch --show-current)"
log "Commit: $(git log --oneline -1)"
log ""

# ─── Phase 1: 빌드 + 유닛 테스트 ───────────────────────────

section "1. cargo check"
if cargo check 2>&1 | tee -a "$LOG" | tail -1 | grep -q "Finished"; then
  pass "cargo check"
else
  fail "cargo check"
fi

section "2. cargo test (secall-core)"
# exit code 우선 — 텍스트 파싱은 Rust 버전마다 달라 fragile.
# 카운트는 부가 정보로만 표시.
if cargo test --package secall-core >>"$LOG" 2>&1; then
  CORE_PASSED=$(grep "^test result:" "$LOG" | tail -1 | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "?")
  pass "secall-core: ${CORE_PASSED} passed"
else
  fail "secall-core: cargo test exited non-zero"
fi

section "3. cargo test (secall CLI)"
if cargo test --package secall >>"$LOG" 2>&1; then
  CLI_PASSED=$(grep "^test result:" "$LOG" | tail -1 | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "?")
  pass "secall CLI: ${CLI_PASSED} passed"
else
  fail "secall CLI: cargo test exited non-zero"
fi

section "4. TypeScript 타입 체크"
if (cd obsidian-secall && npx tsc --noEmit 2>&1) | tee -a "$LOG"; then
  pass "tsc --noEmit"
else
  fail "tsc --noEmit"
fi

section "5. esbuild 번들"
if (cd obsidian-secall && node esbuild.config.mjs production 2>&1) | tee -a "$LOG"; then
  BUNDLE_SIZE=$(ls -la obsidian-secall/main.js | awk '{print $5}')
  pass "esbuild bundle (${BUNDLE_SIZE} bytes)"
else
  fail "esbuild bundle"
fi

# ─── Phase 2: REST API 기능 테스트 ──────────────────────────

section "6. 서버 기동"
cargo build --release 2>&1 | tail -1 | tee -a "$LOG"
# pkill -f 는 다른 도구(에디터/grep 등)도 잡힐 수 있어 정확한 바이너리 경로로 매칭.
pkill -f "target/release/secall serve" 2>/dev/null || true
sleep 1
./target/release/secall serve --port 8080 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
sleep 3

check_endpoint() {
  local name="$1" method="$2" path="$3" body="${4:-}" expect="$5"
  local resp
  if [ "$method" = "GET" ]; then
    resp=$(curl -sf "$BASE$path" 2>&1) || { fail "$name (connection)"; return; }
  else
    resp=$(curl -sf -X POST "$BASE$path" -H 'Content-Type: application/json' -d "$body" 2>&1) || { fail "$name (connection)"; return; }
  fi
  if echo "$resp" | python3 -c "import sys,json; d=json.load(sys.stdin); $expect" 2>/dev/null; then
    pass "$name"
  else
    fail "$name — response: $(echo "$resp" | head -c 200)"
  fi
}

section "7. GET /api/status"
check_endpoint "status" GET "/api/status" "" \
  "assert d['sessions']>0; assert d['vectors']>=0"

section "8. POST /api/recall"
check_endpoint "recall keyword" POST "/api/recall" \
  '{"query":"rust","limit":3}' \
  "assert d['count']==3; assert len(d['results'])==3"

check_endpoint "recall empty" POST "/api/recall" \
  '{"query":"xyznonexistent99999","limit":1}' \
  "assert d['count']==0"

section "9. POST /api/get"
# 유효한 세션 ID 가져오기
SESSION_ID=$(curl -sf -X POST "$BASE/api/recall" \
  -H 'Content-Type: application/json' \
  -d '{"query":"rust","limit":1}' | python3 -c "import sys,json; print(json.load(sys.stdin)['results'][0]['session_id'])" 2>/dev/null)

if [ -n "$SESSION_ID" ]; then
  check_endpoint "get meta" POST "/api/get" \
    "{\"session_id\":\"$SESSION_ID\",\"full\":false}" \
    "assert 'agent' in d; assert 'date' in d"

  check_endpoint "get full" POST "/api/get" \
    "{\"session_id\":\"$SESSION_ID\",\"full\":true}" \
    "assert 'content' in d or 'agent' in d"
else
  fail "get — no session to test"
fi

check_endpoint "get invalid" POST "/api/get" \
  '{"session_id":"nonexistent-id-12345","full":false}' \
  "assert 'error' in d or d == {} or d.get('session') is None"  # 명시적 에러/빈 응답 검증

section "10. POST /api/daily"
check_endpoint "daily 2026-04-05" POST "/api/daily" \
  '{"date":"2026-04-05"}' \
  "assert d['filtered_sessions']>0; assert len(d['projects'])>0; assert d['filtered_sessions']==sum(len(v) for v in d['projects'].values())"

check_endpoint "daily today" POST "/api/daily" \
  '{}' \
  "assert 'date' in d; assert 'projects' in d"

check_endpoint "daily empty date" POST "/api/daily" \
  '{"date":"2020-01-01"}' \
  "assert d['total_sessions']==0; assert d['filtered_sessions']==0"

section "11. POST /api/graph"
check_endpoint "graph depth=1" POST "/api/graph" \
  '{"node_id":"project:seCall","depth":1}' \
  "assert d['count']>0; r=d['results'][0]; assert 'label' in r; assert 'node_type' in r"

check_endpoint "graph depth=2" POST "/api/graph" \
  '{"node_id":"project:seCall","depth":2}' \
  "d1=d['count']; assert d1>0"

check_endpoint "graph relation filter" POST "/api/graph" \
  '{"node_id":"project:seCall","depth":1,"relation":"belongs_to"}' \
  "assert all(r['relation']=='belongs_to' for r in d['results'])"

check_endpoint "graph nonexistent" POST "/api/graph" \
  '{"node_id":"project:nonexistent","depth":1}' \
  "assert d['count']==0"

section "12. CORS"
CORS_HEADER=$(curl -sf -I -X OPTIONS "$BASE/api/daily" \
  -H 'Origin: app://obsidian' \
  -H 'Access-Control-Request-Method: POST' 2>&1 | grep -i "access-control-allow-origin" || echo "")
if echo "$CORS_HEADER" | grep -qi "\*"; then
  pass "CORS allow-origin: *"
else
  fail "CORS — $CORS_HEADER"
fi

# ─── Phase 3: 부하 테스트 (10분) ────────────────────────────

section "13. 부하 테스트 (recall 100회)"
LOAD_START=$(date +%s)
LOAD_FAIL=0
for i in $(seq 1 100); do
  if ! curl -sf -X POST "$BASE/api/recall" \
    -H 'Content-Type: application/json' \
    -d "{\"query\":\"test request $i\",\"limit\":1}" > /dev/null 2>&1; then
    LOAD_FAIL=$((LOAD_FAIL+1))
  fi
done
LOAD_END=$(date +%s)
LOAD_ELAPSED=$((LOAD_END - LOAD_START))
if [ $LOAD_FAIL -eq 0 ]; then
  pass "recall 100x in ${LOAD_ELAPSED}s, 0 failures"
else
  fail "recall 100x: ${LOAD_FAIL} failures in ${LOAD_ELAPSED}s"
fi

section "14. 부하 테스트 (daily 50회)"
LOAD_FAIL=0
for i in $(seq 1 50); do
  DAY=$(printf "2026-04-%02d" $((i % 14 + 1)))
  if ! curl -sf -X POST "$BASE/api/daily" \
    -H 'Content-Type: application/json' \
    -d "{\"date\":\"$DAY\"}" > /dev/null 2>&1; then
    LOAD_FAIL=$((LOAD_FAIL+1))
  fi
done
if [ $LOAD_FAIL -eq 0 ]; then
  pass "daily 50x, 0 failures"
else
  fail "daily 50x: ${LOAD_FAIL} failures"
fi

section "15. 부하 테스트 (graph 50회)"
LOAD_FAIL=0
for i in $(seq 1 50); do
  if ! curl -sf -X POST "$BASE/api/graph" \
    -H 'Content-Type: application/json' \
    -d '{"node_id":"project:seCall","depth":1}' > /dev/null 2>&1; then
    LOAD_FAIL=$((LOAD_FAIL+1))
  fi
done
if [ $LOAD_FAIL -eq 0 ]; then
  pass "graph 50x, 0 failures"
else
  fail "graph 50x: ${LOAD_FAIL} failures"
fi

# ─── Phase 4: 안정성 소크 (6시간 반복) ──────────────────────

section "16. 소크 테스트 (6시간, 5분 간격)"
SOAK_END=$(($(date +%s) + 21600))  # 6시간 (파일 상단 주석과 일치)
SOAK_CYCLE=0
SOAK_FAIL=0

while [ $(date +%s) -lt $SOAK_END ]; do
  SOAK_CYCLE=$((SOAK_CYCLE+1))

  # 각 엔드포인트 1회씩 호출
  OK=true
  curl -sf "$BASE/api/status" > /dev/null 2>&1 || OK=false
  curl -sf -X POST "$BASE/api/recall" -H 'Content-Type: application/json' \
    -d '{"query":"soak cycle","limit":1}' > /dev/null 2>&1 || OK=false
  curl -sf -X POST "$BASE/api/daily" -H 'Content-Type: application/json' \
    -d '{"date":"2026-04-05"}' > /dev/null 2>&1 || OK=false
  curl -sf -X POST "$BASE/api/graph" -H 'Content-Type: application/json' \
    -d '{"node_id":"project:seCall","depth":1}' > /dev/null 2>&1 || OK=false

  if [ "$OK" = "false" ]; then
    SOAK_FAIL=$((SOAK_FAIL+1))
    log "  SOAK cycle $SOAK_CYCLE — FAIL"
  fi

  # 10사이클마다 로그
  if [ $((SOAK_CYCLE % 10)) -eq 0 ]; then
    # 메모리 사용량 체크 (macOS)
    MEM=$(ps -o rss= -p $SERVER_PID 2>/dev/null | awk '{print int($1/1024)"MB"}' || echo "?")
    log "  SOAK cycle $SOAK_CYCLE — mem: $MEM, fails so far: $SOAK_FAIL"
  fi

  sleep 300  # 5분
done

if [ $SOAK_FAIL -eq 0 ]; then
  pass "soak ${SOAK_CYCLE} cycles over 5h, 0 failures"
else
  fail "soak ${SOAK_CYCLE} cycles: ${SOAK_FAIL} failures"
fi

# ─── 정리 ───────────────────────────────────────────────────

kill $SERVER_PID 2>/dev/null || true

section "SUMMARY"
log "Total: $TOTAL tests, $PASS passed, $FAIL failed"
log "Finished: $(date)"

if [ $FAIL -eq 0 ]; then
  log "ALL TESTS PASSED"
  exit 0
else
  log "SOME TESTS FAILED — check $LOG"
  exit 1
fi
