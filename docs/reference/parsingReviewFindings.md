---
type: reference
status: in_progress
updated_at: 2026-07-06
---

# 파싱 서브시스템 코드리뷰 findings (2026-07-06)

적대적 리뷰 워크플로(7차원 + refute-by-default 검증). raised 25 → **confirmed 23**, uncertain 0. 심각도순.

## [1] HIGH — crates/secall-core/src/ingest/detect.rs:157
*discovery-detection / source-discovery-overmatch*  

**find_claude_sessions (and its codex/gemini siblings) recurse the whole tree collecting every *.jsonl, and detect_parser routes anything under /.claude/projects/ to ClaudeCodeParser, so nested non-session artifacts get ingested.**

- 상세: The path-substring routing is a pure directory heuristic with no content check, so any *.jsonl placed anywhere beneath the agent home dirs is force-routed to that agent's parser.
- 실패 시나리오: With workflow artifacts at ~/.claude/projects/<hash>/<uuid>/subagents/workflows/wf_*/journal.jsonl and agent-*.jsonl (the reported ~889 files), `secall ingest --auto` walks them (detect.rs:157-165 collects all *.jsonl with no skip of subagents/, .git, node_modules, tmp, backups). detect_parser (detect.rs:17) then matches path_str.contains("/.claude/projects/") and returns ClaudeCodeParser for every one of them. can_parse in claude.rs:17-22 has the identical over-match (it is dead in prod — only used in tests — but mirrors the live bug). find_codex_sessions/find_gemini_sessions (detect.rs:187, 219) have the same unfiltered walk for their trees.
- 수정 힌트: Restrict the walker to true session files (e.g. filename == <uuid>.jsonl at the project-dir top level, or exclude subagents/workflows/.git/node_modules), and/or make detect_parser require positive content shape (first line has sessionId + type==user) instead of only a path substring.

## [2] HIGH — crates/secall-core/src/ingest/claude.rs:254
*discovery-detection / silent-junk-ingest*  

**parse_claude_jsonl returns Ok for a file with lines but zero conversation turns (only checks line_count==0, never turns.is_empty()), and derives the session id from the filename stem — unlike codex/gemini/opencode which reject empty-turn sessions.**

- 상세: The filename-stem id fallback also means any two turn-less files with the same basename (all journal.jsonl) collapse to one id, so they overwrite each other rather than coexisting — a second data-integrity symptom of the same root cause.
- 실패 시나리오: A subagents/workflows journal.jsonl contains valid JSON lines that are not type user/assistant (log/event records), so every line hits the `_ => continue` arm and turns stays empty. line_count>0, so the empty check at claude.rs:254 passes and a Session is returned. session_id was never set (no user line with sessionId), so id falls back to path.file_stem() = "journal" (claude.rs:258-263); cwd is None so project is None. In ingest_single_session (commands/ingest.rs:1011) min_turns defaults to 0 so the 0-turn filter is off, is_noise_session returns None (no cwd, no user turn), and the empty session is written to the vault + BM25-indexed. Because end_time is None the row is 'open', so every subsequent journal.jsonl (same stem "journal") hits the fast-dedup open-session branch (commands/ingest.rs:621), deletes the prior "journal" row and re-ingests — hundreds of delete+reinsert cycles leaving one arbitrary junk survivor. Codex (codex.rs:211), gemini (gemini.rs:234) and opencode (opencode.rs:160) all guard with `if turns.is_empty() { return Err }`; claude is the only parser missing this guard.
- 수정 힌트: Add `if turns.is_empty() { return Err(anyhow!("claude session has no parseable turns: {}", path.display())) }` before building the Session, matching the other parsers. This alone neutralizes the workflow-artifact pollution even before the walker is tightened.

## [3] HIGH — crates/secall-core/src/ingest/claude.rs:173
*claude / token-accounting-corruption*  

**Assistant token usage is summed per JSONL line, but real claude-code splits ONE assistant message (same message.id, identical usage object) across many lines, so total_tokens and per-turn tokens are inflated ~3x.**

- 상세: Lines 168-183 read message.usage and do total_tokens.input/output/cached += ... for EVERY assistant line, and also assign that same usage to the per-line Turn.tokens. Real claude-code JSONL emits one assistant response as multiple lines (one per content block: thinking / text / each tool_use), and every one of those lines repeats the SAME usage object (verified: msg_011hEYzyvJbGW6VZbVky9A3z appears on 4 lines all with usage (19012,1234)). Because the parser creates one Turn per line and sums usage per line, both the session total and every fragment turn are multiplied. Measured on a real session (07c233d2, 1698 assistant lines -> 556 distinct message.id): parser computes input=648146 output=3940118, correct is input=202859 output=1203745 (x3.20 / x3.27). These corrupted totals are persisted: markdown.rs:218-219 writes tokens_in/tokens_out to vault frontmatter and session_repo.rs:67-68 writes them to SQLite. cache_creation_input_tokens is also ignored entirely (only cache_read counted).
- 실패 시나리오: Ingest any normal claude-code session that uses thinking + tool calls (the common case). A session that actually consumed ~203K input / ~1.2M output tokens is stored in the vault and DB as ~648K input / ~3.94M output. Any per-turn token display shows the full message usage repeated on the thinking-only, text-only, and each tool_use-only fragment turn.
- 수정 힌트: De-duplicate usage by message.id: only add to total_tokens (and only set Turn.tokens) the first time a given assistant message.id is seen. Better, merge same-message.id lines into a single Turn before token accounting.

## [4] HIGH — crates/secall-core/src/ingest/claude.rs:226
*claude / data-loss-tool-output*  

**For parallel tool calls, pending_tool_uses is overwritten per assistant line (226) and cleared after the first tool_result (133), so all-but-one parallel tool outputs are silently dropped (output_summary stays empty).**

- 상세: When one assistant message issues 2+ parallel tool_use blocks, claude-code logs each tool_use as a SEPARATE JSONL line and returns each tool_result as a SEPARATE user line (verified in real data: 94 message.ids per session split their tool_use across lines; every user line carries exactly 1 tool_result). Line 226 does `pending_tool_uses = new_pending;` on every assistant line, so after processing the 2nd tool_use line only the 2nd id remains pending (the 1st is discarded before any result arrives). Then when tool_result lines are processed, line 133 `pending_tool_uses.clear();` wipes the map after the FIRST result line, so the 2nd result also finds nothing. Traced sequence [18] tool_use Read toolu_A, [19] tool_use Bash toolu_B, [20] result toolu_A (not in pending{toolu_B} -> lost, then clear), [21] result toolu_B (pending empty -> lost): BOTH outputs lost. output_summary is only ever set here (grep confirms no downstream re-pairing), so the empty string is permanently stored in vault markdown and DB.
- 실패 시나리오: A session where the assistant reads a file and runs a bash command in parallel: both tool_result outputs are discarded, and the stored session shows two ToolUse actions (Read, Bash) with empty output_summary. Reindex/search over tool outputs returns nothing for those calls.
- 수정 힌트: Do not overwrite pending across lines of the same assistant message; accumulate. Remove only the matched tool_use_id on each result instead of clear()-ing the whole map, and locate the owning turn by tool_use_id rather than assuming turns.last().

## [5] HIGH — crates/secall-core/src/ingest/codex.rs:175
*codex-opencode / wrong-turn-attribution*  

**Codex `function_call` items are attached to `turns.last_mut()`, which is the USER turn whenever the assistant emits tool calls without a preceding assistant text `message` in that response — so the assistant's tool activity is stored under the user.**

- 실패 시나리오: Real codex rollout format emits, per assistant response: reasoning -> function_call(s) -> function_call_output(s) -> (optionally) an assistant `message` text item at the END. When the model calls tools directly (no assistant preamble text before the calls), the most recent pushed turn is the last user message, so every ToolUse Action is pushed onto the user turn. Verified against the on-disk corpus at ~/.codex/sessions: 428 function_calls across 23 of 302 files attach to a user turn; file rollout-2026-02-20T07-52-35-019c781a-....jsonl has 75 function_calls, 76 reasoning items, and only ONE assistant message (emitted last) — all 75 tool calls attach to the 3rd user turn. In the rendered vault markdown these tools render under `## Turn N — User`, and the single real assistant turn has zero actions. Per-turn/role tool association (used by graph/search and web UI) is wrong for these sessions. The existing test only covers the assistant-message-first ordering, masking the bug.
- 수정 힌트: Model a codex 'assistant response' as spanning reasoning+function_call+message. When a function_call arrives and the last turn is not an Assistant turn, create/attach to an assistant turn instead of turns.last() (e.g., open a synthetic assistant turn on the first reasoning/function_call after a user message, and fold the trailing assistant `message` text into it).

## [6] HIGH — crates/secall-core/src/ingest/gemini_web.rs:167
*gemini / data-loss*  

**Gemini Web ZIP exports containing multiple conversations lose all but the first conversation during ingest.**

- 상세: GeminiWebParser is explicitly a 1:N parser: parse_all() (gemini_web.rs:178) walks every *.json entry in the ZIP and returns a Vec<Session> (the unit test test_parse_all_from_zip puts 2 sessions in and asserts len==2). But the ingest dispatcher in crates/secall/src/commands/ingest.rs only routes AgentKind::ClaudeAi and AgentKind::ChatGpt through parse_all() (ingest.rs:567). AgentKind::GeminiWeb is neither, so it falls into the 1:1 branch at ingest.rs:661 which calls parser.parse(). GeminiWebParser::parse (gemini_web.rs:165-172) calls parse_all() internally and then returns only sessions.into_iter().next() — the FIRST session in ZIP directory order — silently discarding every other conversation. detect_parser (detect.rs:39-40) confirms gemini-web ZIPs reach this parser. ChatGPT (also a ZIP N:1 format) is correctly wired for parse_all; GeminiWeb was omitted.
- 실패 시나리오: User exports their Gemini web history as gemini-export.zip containing 50 conversation JSON files (each with projectHash "gemini-web") and runs `secall ingest gemini-export.zip`. detect_parser selects GeminiWebParser; ingest takes the 1:1 path and calls parse(), which returns only the first conversation. Only 1 of 50 conversations is stored in the vault/DB. No error and no warning is emitted — the other 49 are silently lost.
- 수정 힌트: Add AgentKind::GeminiWeb to the parse_all() routing branch in ingest.rs (alongside ClaudeAi/ChatGpt), so all sessions in the ZIP are ingested.

## [7] HIGH — crates/secall-core/src/ingest/markdown.rs:194
*markdown / correctness*  

**Frontmatter string fields (project, cwd, model, host, session_type) are emitted completely unquoted, and the only escaped field (summary) uses escape_yaml_string which handles just backslash/quote — so any value with a YAML metacharacter or control char produces invalid YAML that parse_session_frontmatter (serde_yaml) rejects, silently dropping the whole session on reindex/graph/sync/migrate.**

- 상세: render_session builds YAML by hand. `project: {}` (l.193-195), `cwd: {}` (l.196-198), `model: {}` (l.190-192), `host: {host}` (l.243-245), `session_type: {}` (l.251) are written raw with no quoting/escaping. Only `summary` is quoted, and escape_yaml_string (l.424-426) escapes only `\` and `"` — not C0 control chars, which are illegal in a YAML double-quoted scalar. project for claude-code is the raw cwd basename (claude.rs:266, no sanitization); for the round-trip these values flow back through serde_yaml in parse_session_frontmatter (l.44-48). On error every consumer just logs warn and `continue`s (reindex.rs:52-59, graph.rs:111, sync.rs:490, migrate.rs:38), so the session is omitted from the rebuilt index/graph while its vault file looks fine.
- 실패 시나리오: A claude-code session whose project directory is named `[archive]` (legal on Windows) renders `project: [archive]`; on `secall reindex --from-vault` serde_yaml parses `[archive]` as a flow sequence, fails to deserialize into Option<String>, parse_session_frontmatter returns Err, and reindex logs 'failed to parse frontmatter' + skips it — the session never enters the DB. Same for a dir named `@work` (leading reserved indicator → scanner error) or a first user line containing a raw ANSI ESC byte that survives into `summary: "...\x1b[31m..."` (unescaped control char → invalid double-quoted scalar). A leading `#` dir name (`#notes`) instead mis-parses: `project: #notes` treats `#notes` as a comment, storing project as empty.
- 수정 힌트: Emit all string frontmatter values through a proper YAML string serializer (serde_yaml::to_string on a struct, or always quote + escape control chars) instead of hand-formatting; extend escape_yaml_string to cover control characters, or reuse serde_yaml for the whole frontmatter block.

## [8] MEDIUM — crates/secall/src/commands/ingest.rs:613
*discovery-detection / dedup-attribution*  

**The 1:1 fast duplicate/open-session check keys on the raw filename stem, but CodexParser strips the 'rollout-' prefix (and prefers session_meta.id), so the stem never equals the stored session id for codex — the fast-path is permanently defeated for every codex session.**

- 상세: Claude works only by luck: its filename stem equals the uuid equals the stored id. Any parser whose stored id differs from the stem (codex, and potentially others) silently loses the fast-path/open-refresh behavior.
- 실패 시나리오: Codex file ~/.codex/sessions/2026/01/01/rollout-2026-01-01T10-00-00-<uuid>.jsonl has file_stem "rollout-2026-01-01T10-00-00-<uuid>". ingest_path (commands/ingest.rs:613) calls db.session_exists("rollout-...") which is always false because parse_codex_jsonl stores id = session_meta.id (or the stem with 'rollout-' stripped) = "<uuid>" (codex.rs:78-82, 219). So the fast dedup and its is_session_open re-ingest branch (ingest.rs:621) are skipped for all codex files; every run fully re-parses each codex session. The real by-id dedup in ingest_single_session (ingest.rs:1034) then catches finished sessions, but that branch only re-ingests on the turn-count-growth heuristic (ingest.rs:1045) and never consults is_session_open — so an ongoing/open codex session that grew by only a few turns is skipped instead of refreshed, unlike claude where the stem hint matches and the open-session refresh fires.
- 수정 힌트: Either derive the dedup hint the same way the parser derives the id (strip 'rollout-' for codex), or drop the stem-based fast path and rely on the real session.id check plus an explicit is_session_open re-ingest branch in ingest_single_session.

## [9] MEDIUM — crates/secall-core/src/ingest/codex.rs:137
*codex-opencode / mis-parsing*  

**Only `role == "developer"` is skipped; codex records injected context (AGENTS.md instructions, `<environment_context>`, `<user_instructions>`, `<permissions>`) as `role: user` messages, so the FIRST user turn is boilerplate, not the real prompt.**

- 실패 시나리오: In the real corpus, 280/301 codex files have `# AGENTS.md instructions for <path>` (or `<environment_context>` / `<user_instructions>` / `<permissions instructions>`) as the first non-empty user turn; the actual user question is the 3rd user message. Consequence 1: markdown.rs `extract_summary` takes the first user turn's first non-empty line, so ~93% of codex sessions get a vault frontmatter `summary` of "# AGENTS.md instructions for …" (truncated to 80 chars) instead of the user's question — polluting session listings and search. Consequence 2: `is_noise_session` (ingest/mod.rs) inspects only the first user turn for the wiki marker / summary-prompt prefix; since codex is now the default wiki backend and its self-invoked sessions carry AGENTS.md as the first user turn, the marker lands in a later turn and the self-ingest guard is defeated (unless the cwd happens to match the tmpdir check — wiki/codex.rs runs in the repo cwd, so it does not).
- 수정 힌트: Skip codex-injected context user messages (detect AGENTS.md-instructions / `<environment_context>` / `<user_instructions>` / `<permissions` prefixes) the same way `developer` is skipped, or flag them so summary/noise logic can find the real first prompt.

## [10] MEDIUM — crates/secall-core/src/ingest/opencode.rs:131
*codex-opencode / tool-mapping-dataloss*  

**OpenCode parser keeps only `type == "text"` parts and drops every tool call/result; assistant messages consisting solely of tool parts are skipped entirely via the `content.is_empty()` continue.**

- 실패 시나리오: An opencode assistant message with parts [tool-call bash, tool-result output] and no text part yields an empty joined content and is dropped at line 136 (`if content.is_empty() { continue; }`), so that turn disappears and turn count/index no longer reflect the conversation. Even when text is present, all tool inputs/outputs are discarded: `actions` is always empty, so the `tools_used` frontmatter is always `[]` and no command/tool output from opencode sessions is ever written to the vault or indexed into FTS/semantic search — unlike the codex and claude parsers which map tools. Any query for a command run in an opencode session returns nothing.
- 수정 힌트: Map opencode tool parts (real export uses `type:"tool"` with nested `state.input`/`state.output`) into Action::ToolUse, and treat tool-only assistant messages as turns rather than skipping them.

## [11] MEDIUM — crates/secall-core/src/ingest/gemini_web.rs:44
*gemini / silent-failure*  

**A single message with a non-text content part (image/thought/function part) or a missing required field causes the entire conversation to be silently dropped.**

- 상세: GeminiWebContent is an untagged enum of Text(String) | Parts(Vec<GeminiWebPart>), and GeminiWebPart requires a non-optional `text: String` (gemini_web.rs:44-47). Additionally GeminiWebMessage requires id/timestamp/type/content with no serde defaults (gemini_web.rs:26-35). When content is an array where any element lacks a string `text` field (e.g. an inline image part, a thought/thinking part, a grounding/citation part, or a function-call part), the Parts variant fails to deserialize; Text also fails (it's an array), so the whole untagged enum fails, which fails GeminiWebMessage, which fails GeminiWebExport. In parse_archive (gemini_web.rs:145-150) that serde error is caught, a warn! is logged, and the ENTIRE session is skipped via `continue` — not just the offending message. Same whole-session drop occurs if any message is missing content/timestamp/id/type.
- 실패 시나리오: A Gemini web conversation in the export ZIP contains one message with a non-text part, e.g. content = [{"text":"see this"},{"inlineData":{"mimeType":"image/png",...}}]. The second part has no `text` field, so serde_json::from_str::<GeminiWebExport> returns Err for that file. parse_archive logs a warning and skips the whole conversation. The entire multi-turn conversation is absent from the vault, with only a log line as evidence.
- 수정 힌트: Make GeminiWebPart.text optional (default) and filter empty/None, or make the parts collection lenient (deserialize to serde_json::Value and extract text best-effort) so unknown part shapes degrade to empty text instead of dropping the whole session.

## [12] MEDIUM — crates/secall-core/src/ingest/gemini.rs:246
*gemini / wrong-attribution*  

**Gemini CLI session with a missing/unparseable startTime is dated to ingest time (Utc::now()), corrupting session date attribution.**

- 상세: start_time is Option with serde default (gemini.rs:37-38), and the parser falls back to Utc::now() when it is absent or not RFC3339 (gemini.rs:241-246). This start_time is the session's canonical date used for daily-note grouping and frontmatter. When startTime is missing, the code does NOT fall back to the first turn's timestamp (which is available in msg.timestamp), so the session is attributed to the moment of ingestion rather than when the conversation actually occurred.
- 실패 시나리오: A gemini CLI session file lacks a top-level startTime (only per-message timestamps present, e.g. an older or partial session-*.json). Ingesting it in 2026-07 files the conversation under 2026-07-06 (ingest day) even though the messages are timestamped weeks earlier, placing it in the wrong daily note and skewing any date-range queries.
- 수정 힌트: Fall back to the earliest turn timestamp (first message's ts) before defaulting to Utc::now().

## [13] MEDIUM — crates/secall-core/src/ingest/chatgpt.rs:182
*chatgpt-claudeai / mis-parsing*  

**ChatGPT `code` content_type is routed to the `parts` extractor, but `code` messages store their payload in a `text` field (no `parts` key), so the content extracts as empty and the turn is dropped entirely.**

- 상세: `execution_output` is already handled with `.get("text")`; `code` has the same shape but was grouped with the `parts`-based types instead. The default `_` arm (lines 239-259) already falls back to `text`/`content`, so simply removing `"code"` from the parts arm, or adding a dedicated `"code" => .get("text")` arm, fixes it.
- 실패 시나리오: A real ChatGPT conversations.json where the assistant used the code interpreter / data analysis contains messages with content `{"content_type":"code","language":"python","text":"import pandas as pd..."}` (no `parts` array). `extract_message_content` matches the `"text" | "code" | "multimodal_text"` arm, calls `.get("parts")` which is None, and returns `String::new()`. Back in `conversation_to_session` (lines 306-309), `content.is_empty() && thinking.is_none()` is true, so the message is `continue`d and never becomes a turn. Result: every code block the assistant wrote/executed is silently missing from the vault markdown, while the tool outputs (author `tool`, content_type `execution_output`) are still stored — so the archived transcript shows results with no visible code, with no error or warning.
- 수정 힌트: Remove `"code"` from the `"text" | "code" | "multimodal_text"` arm (let it hit the `_` fallback that reads `text`), or add an explicit `"code" => message.content.get("text").and_then(|v| v.as_str()).unwrap_or_default().to_string()` arm.

## [14] MEDIUM — crates/secall-core/src/ingest/markdown.rs:125
*markdown / mis-parsing*  

**parse_session_turns treats any main-content line beginning with `## Turn N — Role` or `### Turn N` as a real turn header, but render_session writes turn.content unescaped (only collapse_blank_lines + escape_dataview_fields), so content that literally contains such a line at column 0 is re-parsed as a phantom turn with wrong index/role and split content.**

- 상세: The code fence toggle (l.111-123) only guards ```-fenced lines; a bare `## Turn 5 — Assistant` in prose is not fenced. render_session's main-content path (l.328-333) applies no heading escaping. On reparse, strip_prefix("## Turn ")/parse_turn_heading_h2 (l.125-138, 163-173) matches it and flush()es a new Turn, so the real turn is truncated and a phantom turn (index=N-1, role from the injected header) is created, and following lines are misattributed to it.
- 실패 시나리오: An assistant turn that quotes/generates vault format — e.g. content containing a line `## Turn 2 — Assistant` or `### Turn 3` at line start (common in this project's own meta sessions about markdown.rs) — round-trips through render then reindex/heal_session: parse_session_turns emits an extra turn, the turn count no longer matches frontmatter `turns:`, and the assistant's real text is split across a spurious turn in the turns/turns_fts tables.
- 수정 힌트: On render, escape content lines that would collide with the turn-header grammar (e.g. prefix with a zero-width space or fence them), or on parse require the header to be preceded by a blank line/only accept headers when not mid-content; at minimum add a regression test with `## Turn N — Role` inside turn content.

## [15] MEDIUM — crates/secall-core/src/ingest/markdown.rs:111
*markdown / mis-parsing*  

**An unbalanced (odd number of) ``` fence inside a turn's content leaves parse_session_turns stuck with in_code_block=true, causing every subsequent `## Turn`/`### Turn` header to be swallowed as code and the following turns to be merged into the current one, undercounting turns on reparse.**

- 상세: in_code_block toggles on each ```-prefixed line (l.111-117) and while true all header detection is skipped (l.118-123). render_session writes turn.content verbatim (l.328-333), so if one turn's content contains a lone/unclosed ``` (e.g. an assistant response cut at a tool_use boundary while a code fence is open, or a user paste with a stray triple backtick), the fence never closes and all later turn headers are absorbed as code content of that turn until another ``` appears.
- 실패 시나리오: Assistant turn content ends with an opened but unclosed ```rust block (claude-code splits one assistant message into multiple Turns at tool boundaries). Rendered MD has ```rust ... then blank lines then `### Turn N+1`. parse_session_turns stays in_code_block and appends `### Turn N+1`, its content, and every later header as code of turn N. heal_session/reindex then stores far fewer turns than the frontmatter `turns:` count, with content merged into the wrong turn.
- 수정 힌트: Track fence open/close per turn and force-close the fence at each real header, or only honor a fence toggle when it also has a matching closer within the same turn; add a test with an unclosed ``` inside a turn followed by more turns.

## [16] MEDIUM — crates/secall-core/src/ingest/markdown.rs:417
*robustness / panic-multibyte-slice*  

**`&session.id[..8]` byte-slices the session id at a fixed byte offset guarded only by a byte-length check (`session.id.len() >= 8`), not a char boundary; a non-ASCII id panics and aborts the entire ingest run.**

- 상세: In-scope parser file markdown.rs is the earliest and most certain trigger because it runs for every session during vault write, before the ingest.rs:723 progress loop is ever reached.
- 실패 시나리오: A claude-code session file whose in-content `sessionId` is absent falls back to the file stem (claude.rs:261). A user backs up/renames a session to a multibyte name, e.g. `세션백업.jsonl`, so session.id = "세션백업" (12 bytes — the `>= 8` guard passes). `session_filename` evaluates `&session.id[..8]`, which lands inside the 3rd Korean char → `byte index 8 is not a char boundary` panic. This is on the write hot path: session_filename -> session_vault_path -> Vault::write_session, reached for every ingested session. The ingest driver loops over paths sequentially with no catch_unwind (secall/src/commands/ingest.rs ingest_path), so one bad id aborts the whole `secall ingest` batch and every not-yet-processed session is lost for that run. Any parser that takes its id verbatim from JSON (gemini `sessionId`, codex meta `id`, opencode `info.id`) is equally exposed if that field is non-ASCII.
- 수정 힌트: Take a char-aware prefix instead of a byte slice, e.g. `session.id.chars().take(8).collect::<String>()`. The same unguarded pattern recurs at secall/src/commands/ingest.rs:723, 730, 773 and 840 (`&session.id[..8.min(len)]` / `&session_id[..8.min(len)]` — the `.min(len)` only covers len<8, not the multibyte boundary at byte 8), so the embedding and semantic-extraction loops share the identical crash and should be fixed together.

## [17] LOW — crates/secall-core/src/ingest/claude.rs:134
*claude / data-loss-user-text*  

**A user message whose content array contains a tool_result AND a text block drops the user's text entirely.**

- 상세: Lines 107-135: if any item in the user content array is a tool_result, the code attaches results then `continue`s at 134, never calling extract_user_text. So a user line like [{type:tool_result,...},{type:text,text:"stop, do X instead"}] loses the typed text and creates no user turn. Not observed in the sampled session (0 mixed lines there) but is a valid claude-code shape (e.g. user text bundled with a tool_result, or image-bearing results with accompanying text).
- 실패 시나리오: User cancels/redirects while a tool result is being submitted so the client bundles text with the tool_result; the user's instruction is silently absent from the stored session and from search.
- 수정 힌트: After attaching tool_results, still extract any text blocks and, if non-empty, push a User turn instead of unconditionally continuing.

## [18] LOW — crates/secall-core/src/ingest/claude.rs:313
*claude / raw-ansi-stored*  

**Tool output is stored raw with no ANSI escape stripping (confirmed known issue).**

- 상세: extract_tool_result_content (313-331) returns tool_result text verbatim, then truncate_str (line 120) only length-limits it. There is no ANSI/control-sequence stripping anywhere on the path, so ESC[..m color codes and cursor sequences from tools (e.g. colored cargo/git output) are written raw into output_summary -> vault markdown and DB/FTS index. truncate_str itself is UTF-8 safe (operates on chars, not bytes) so no panic there.
- 실패 시나리오: A Bash tool whose command emits ANSI-colored output stores literal escape sequences in the session markdown and search index, degrading readability and search matching.
- 수정 힌트: Strip ANSI/control sequences in extract_tool_result_content before truncation.

## [19] LOW — crates/secall-core/src/ingest/claude.rs:61
*claude / silent-empty-session*  

**A file consisting only of malformed/typeless lines yields a valid empty Session instead of the intended 'empty session file' error.**

- 상세: line_count is incremented at line 61 for every non-blank line BEFORE JSON parsing and type checks. The empty-file guard at line 254 (`if line_count == 0`) therefore only fires when the file has zero non-blank lines. A file full of invalid JSON (or valid JSON lacking a usable type) increments line_count, skips every line, and returns Ok(Session) with 0 turns and start_time defaulted to Utc::now(). This is written to the vault as a real but contentless session.
- 실패 시나리오: A truncated/corrupted .jsonl (all lines unparseable) is ingested as a legitimate empty session dated 'now' rather than being rejected, polluting the index.
- 수정 힌트: Track successfully parsed conversation lines (or turns.is_empty()) for the empty-session check instead of raw non-blank line count.

## [20] LOW — crates/secall-core/src/ingest/codex.rs:81
*codex-opencode / session-id-derivation*  

**Filename-fallback session_id assumes `rollout-<uuid>.jsonl` but real files are `rollout-<timestamp>-<uuid>.jsonl`, so the fallback id includes the timestamp prefix.**

- 실패 시나리오: Real filenames are e.g. `rollout-2026-02-15T09-20-58-019c5eac-081b-7011-a172-049b28bae864.jsonl`; `strip_prefix("rollout-")` yields `2026-02-15T09-20-58-019c5eac-...` (the whole timestamp+uuid), not the uuid. This is normally masked because session_meta.id (present in all 302 real files) wins, but if the first line is ever missing/corrupt (interrupted or truncated write), the stored session_id becomes the timestamp string and markdown.rs `session_filename` takes its first 8 chars = `2026-02-1`, so two same-day fallback sessions collide on the vault filename prefix. Latent (0/302 observed) but the comment's `rollout-<uuid>` assumption is factually wrong.
- 수정 힌트: Parse the uuid out of the filename (last 5 hyphen-joined groups) for the fallback, or keep the full stem but do not assume it is a bare uuid.

## [21] LOW — crates/secall-core/src/ingest/gemini.rs:265
*gemini / wrong-metadata*  

**Gemini CLI sessions store total_tokens as 0 despite per-turn token data being parsed.**

- 상세: The parser reads per-message token usage into Turn.tokens (gemini.rs:178-182) but sets Session.total_tokens: Default::default() (gemini.rs:265), i.e. 0/0/0. ingest_single_session does not recompute total_tokens from turns, and total_tokens is written verbatim to the vault frontmatter tokens_in/tokens_out (markdown.rs:218-219) and to the DB sessions row (session_repo.rs:67-68). By contrast claude.rs accumulates per-turn tokens into total_tokens (claude.rs:173-175, 286). So Gemini CLI sessions report zero total tokens even though the data was available.
- 실패 시나리오: Ingest a gemini CLI session whose messages carry tokens (input:100,output:50). The vault markdown frontmatter shows tokens_in: 0 / tokens_out: 0 and the DB sessions.tokens_in/out are 0, so any token-based reporting/aggregation under-counts Gemini CLI usage to zero.
- 수정 힌트: Accumulate per-turn tokens into total_tokens while building turns, mirroring claude.rs.

## [22] LOW — crates/secall-core/src/ingest/chatgpt.rs:342
*chatgpt-claudeai / wrong-attribution*  

**When `conv.create_time` is null, `start_time` falls back to `Utc::now()` (import time) instead of the earliest message timestamp, mis-dating the session.**

- 상세: The chain already contains per-message `create_time`s; the earliest present message timestamp is a strictly better fallback than `Utc::now()`. The claude_ai parser has the same shape but `created_at` is a required RFC3339 field there, so the null path is far less realistic.
- 실패 시나리오: `create_time` is declared `Option<f64>` and is null in some ChatGPT exports (e.g. project/archived conversations). For such a conversation `epoch_to_datetime(conv.create_time)` is None, so `start_time` becomes `Utc::now()` — the moment of import — even though the messages have real timestamps (and `end_time` is correctly derived from them). The session is then dated to today, lands in today's daily note, and sorts as most-recent, despite its messages being months old; `start_time` can also end up later than `end_time`.
- 수정 힌트: Fall back to the first available message timestamp before `Utc::now()`, e.g. `epoch_to_datetime(conv.create_time).or_else(|| chain.iter().find_map(|m| epoch_to_datetime(m.create_time))).unwrap_or_else(Utc::now)`.

## [23] LOW — crates/secall-core/src/ingest/chatgpt.rs:182
*robustness / mis-parse-content-loss*  

**content_type "code" is routed to the parts-only extraction arm, but ChatGPT exports store "code" content under a `text` string field (not `parts`), so the code extracts to an empty string and the whole turn is dropped.**

- 실패 시나리오: A ChatGPT `conversations.json` conversation containing a code-interpreter/code message such as `{"content_type":"code","language":"python","text":"print(1)"}`. extract_message_content matches the `"text" | "code" | "multimodal_text"` arm which reads only `content.parts`; a "code" block has no `parts` field, so `unwrap_or_default()` yields "". Back in conversation_to_session the turn has empty content and no thinking, so it hits the `content.is_empty() && thinking.is_none()` guard (chatgpt.rs:307) and is skipped entirely. The code the assistant authored is silently missing from the stored session markdown and is not searchable/embeddable. Contrast with `execution_output`, which correctly reads `text`.
- 수정 힌트: Handle "code" like "execution_output" (read the `text` string), or move "code" out of the parts-only arm into the fallback `_` arm which already tries parts -> text -> content and would recover it.
