<!-- Thanks to: @batmania52, @yeonsh, @missflash, @CoLuthien, @dev-minsoo -->

<div align="center">

# seCall

AIエージェントとの会話をローカルWikiに整理して検索しましょう。

**Your AI agent conversations, as a searchable local wiki.**

[![Rust](https://img.shields.io/badge/Rust-1.75+-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5-003B57?logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![MCP](https://img.shields.io/badge/MCP-Protocol-5A67D8?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0Ij48Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIgZmlsbD0id2hpdGUiLz48L3N2Zz4=)](https://modelcontextprotocol.io/)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![ONNX Runtime](https://img.shields.io/badge/ONNX-Runtime-007CFF?logo=onnx&logoColor=white)](https://onnxruntime.ai/)
[![Obsidian](https://img.shields.io/badge/Obsidian-Plugin-7C3AED?logo=obsidian&logoColor=white)](https://obsidian.md/)

<br/>

[**`한국어`**](README.md) · [**`English`**](README.en.md) · **`日本語`** · [**`中文`**](README.zh.md)

</div>

---

## 目次

- [seCallとは？](#secallとは)
- [主な機能](#主な機能)
  - [マルチエージェント収集](#マルチエージェント収集)
  - [ハイブリッド検索](#ハイブリッド検索)
  - [ナレッジボールト](#ナレッジボールト)
  - [Knowledge Graph](#knowledge-graph)
  - [Web UI + REST API + Obsidianプラグイン](#web-ui--rest-api--obsidianプラグイン)
  - [MCPサーバー](#mcpサーバー)
  - [マルチデバイスボールト同期](#マルチデバイスボールト同期)
  - [データ整合性](#データ整合性)
- [クイックスタート](#クイックスタート)
  - [前提条件](#前提条件)
  - [Step 1. インストール](#step-1-インストール)
  - [Step 2. 初期化](#step-2-初期化)
  - [Step 3. セッション収集](#step-3-セッション収集)
  - [Step 4. 検索](#step-4-検索)
- [使い方](#使い方)
  - [セッション参照](#セッション参照)
  - [エンベディング生成](#エンベディング生成)
  - [セッション分類](#セッション分類)
  - [Wiki生成](#wiki生成)
  - [作業日記](#作業日記)
  - [Knowledge Graph](#knowledge-graph-1)
- [設定](#設定)
  - [設定キー一覧](#設定キー一覧)
- [CLIリファレンス](#cliリファレンス)
- [MCP連携](#mcp連携)
- [アーキテクチャ](#アーキテクチャ)
- [技術スタック](#技術スタック)
- [出典](#出典)
- [ライセンス](#ライセンス)

---

<div align="center">
<img src="screenshot.png" alt="seCall Obsidian ボールト" width="720" />
<br/><br/>
</div>

## seCallとは？

seCallはAIエージェントの会話に特化したローカルファーストのツールです。**Claude Code**、**Codex CLI**、**Gemini CLI**、**claude.ai**、**ChatGPT** のセッションログを収集し、LLMでObsidian互換の **Wiki** に整理して、BM25＋ベクトルハイブリッド **検索** をCLI / MCPサーバー / REST API / 内蔵Web UI で提供します。

### なぜ必要なのか？

- アーキテクチャの意思決定、デバッグの痕跡、設計メモがエージェントのJSONLファイルに散らばっており、「この前あのupstreamエラーをどうパッチしたっけ？」を再び探すのは面倒です。
- seCallは元のtranscriptをそのまま保存しつつ、その上にLLMが整理したWikiを重ね、両方を検索できます — CLI / Web UI / Obsidian / MCP互換のAIエージェントのどこからでも。

## 主な機能

### マルチエージェント収集

複数のAIコーディングエージェントのセッションを統一フォーマットでパースし、正規化します:

| エージェント | フォーマット | 状態 |
|---|---|---|
| Claude Code | JSONL | ✅ 安定版 |
| Codex CLI | JSONL | ✅ 安定版 |
| Gemini CLI | JSON | ✅ 安定版 |
| claude.ai | JSON (ZIP) | ✅ v0.2 新規 |
| ChatGPT | JSON (ZIP) | ✅ v0.2.3 新規 |

### ハイブリッド検索

- **BM25全文検索**: SQLite FTS5 + 韓国語形態素解析 ([Lindera](https://github.com/lindera/lindera) ko-dic / [Kiwi-rs](https://github.com/bab2min/kiwi) 選択可能)
- **ベクトル意味検索**: [Ollama](https://ollama.com/) BGE-M3エンベディング (1024次元) + **HNSW ANNインデックス** ([usearch](https://github.com/unum-cloud/usearch)) によるO(log n)探索
- **Reciprocal Rank Fusion (RRF)**: BM25/ベクトル独立実行後に結合 (k=60) + **セッション多様性の強制** (1セッションあたり最大2ターン)
- **LLMクエリ拡張**: Claude Codeによる自然言語クエリ拡張

### ナレッジボールト

Obsidian互換マークダウンボールト (2層構造):

```
vault/
├── raw/.sessions/   # 不変セッション原本 (dot-prefix → obsidianで自動非表示、v0.5.0+)
│   └── YYYY-MM-DD/  # 日付別整理
├── wiki/            # AI生成ナレッジページ
│   ├── projects/    # プロジェクト別サマリー
│   ├── topics/      # 技術トピックページ
│   └── decisions/   # アーキテクチャ意思決定記録
└── graph/           # Knowledge Graph出力
    └── graph.json   # ノード/エッジデータ
```

- **Wiki生成**: pluggable LLM backend ベース (`secall wiki update --backend claude|codex|haiku|ollama|lmstudio`)
- **Obsidianバックリンク** (`[[]]`) でセッション ↔ Wikiページを連結
- Dataviewクエリ用のfrontmatterメタデータ (`summary` フィールドでセッション内容を一目で把握)

### Knowledge Graph

セッション間の関係を抽出してナレッジグラフを構築します:

- **ノードタイプ**: session, project, agent, tool — frontmatterから自動抽出
- **ルールベースエッジ**: `belongs_to`, `by_agent`, `uses_tool`, `same_project`, `same_day` (LLM不要)
- **セマンティックエッジ** (Gemini/Ollama/LM Studio): `fixes_bug`, `modifies_file`, `introduces_tech`, `discusses_topic` — LLMがセッション内容を分析して抽出
- **増分ビルド**: 新規セッションのみノード追加、関係エッジは全体再計算で正確性を担保
- **MCPツール**: `graph_query` — AIエージェントがセッション間関係を探索 (BFS、最大3ホップ)

### Web UI + REST API + Obsidianプラグイン

`secall serve` はREST APIとWeb UIを同一ポート (8080) で提供し、Obsidianプラグインとも同じAPIを共有します。

```bash
# REST API + Web UI サーバー起動
secall serve --port 8080
# ブラウザ: http://127.0.0.1:8080
```

**エンドポイント**:
- 読み取り (Phase 0): `/api/recall`, `/api/get`, `/api/status`, `/api/daily`, `/api/graph`, `/api/wiki` (検索)
- Wiki本文 (Phase 1): `GET /api/wiki/{project}`
- セッションメタ (Phase 0): `/api/sessions`, `/api/projects`, `/api/agents`, `PATCH /api/sessions/{id}/{tags,favorite}`
- セッションノート (Phase 2): `PATCH /api/sessions/{id}/notes`
- タグ一覧 (Phase 3): `GET /api/tags?with_counts={true|false}`
  - `true` (デフォルト): `{ "tags": [{ "name": "rust", "count": 12 }, ...] }`
  - `false`: `{ "tags": ["rust", "search", ...] }`
- コマンド (Phase 1): `POST /api/commands/{sync,ingest,wiki-update}`
- グラフ再構築 (P37): `POST /api/commands/graph-rebuild`
  - body: `{ since?, session?, all?, retry_failed? }`
  - レスポンス: `{ job_id, status: "started" }`
  - シングルキューポリシー: 他のmutating jobが実行中であれば `409 Conflict`
- Job管理 (Phase 1): `GET /api/jobs`, `GET /api/jobs/{id}`, `GET /api/jobs/{id}/stream` (SSE)
- Jobキャンセル (P36): `POST /api/jobs/{id}/cancel`
  - 200: `{ "cancelled": true, "job_id": "..." }` — アクティブjobのキャンセル成功 (完了済み/キャンセル済みjobも同じレスポンスでidempotent)
  - 404: `{ "error": "job not found or already evicted" }` — 未登録 / evictされた

**Web UI** (`web/`, P32 Phase 0 + P33 Phase 1):
- ダークモード優先のモダンUI (Tailwind + shadcn/ui + Pretendard/Geist Sans)
- 2ペインレイアウト (左: 検索/リスト、右: 詳細)
- グラフ折りたたみオーバーレイ (ノードクリック → セッションロード + 自動フォールディング)
- タグ / お気に入り編集
- サイドバー **Commands** メニュー — Sync / Ingest / Wiki Update トリガー (Phase 1)
- グローバル進行状況バナー + SSE進行状況ストリーミング + 完了/失敗 toast (Phase 1)

**Obsidianプラグイン** (`obsidian-secall/`):
- **検索ビュー** — キーワード/セマンティックセッション検索
- **デイリービュー** — 日付別作業サマリー、プロジェクト別セッショングルーピング、ノート生成
- **グラフビュー** — ノード関係探索 (depth 1-3、関係フィルター)
- **セッションビュー** — フルマークダウンレンダリング
- **ステータスバー** — セッション数 + エンベディング状態表示 (5分更新)

### MCPサーバー

MCP互換のAIエージェントにセッションインデックスを公開します:

```bash
# stdioモード (Claude Code, Cursor等)
secall mcp

# HTTPモード (Webクライアント)
secall mcp --http 127.0.0.1:8080
```

提供ツール: `recall`, `get`, `status`, `wiki_search`, `graph_query`

### マルチデバイスボールト同期

Git経由で複数のデバイスでナレッジボールトを同期します:

```bash
# 完全同期: git pull → reindex → ingest → wiki → graph → git push
secall sync

# ローカル専用モード (git省略、Claude Code hookに最適)
secall sync --local-only
```

- **MDがソース** — DBは派生キャッシュであり、`secall reindex --from-vault` で完全に復元可能
- **ホスト追跡** — 各セッションがどのデバイスで収集されたかを記録 (frontmatter `host` フィールド)
- **コンフリクトなし** — セッションはデバイス別にユニークなのでgitマージコンフリクトが発生しない

### データ整合性

組み込みのlintルールでインデックス ↔ ボールトの整合性を検証します:

```bash
secall lint
# L001: 欠落したボールトファイル
# L002: 孤立したボールトファイル
# L003: FTSインデックスのギャップ
```

## クイックスタート

### 前提条件

- Rust 1.75+ (ソースビルド時)
- Claude Code, Codex CLI, Gemini CLI のいずれか
- [Ollama](https://ollama.com/) — ベクトル検索用 (オプション、なければBM25のみ)
- **Windows**: MSVC ツールチェーン (Visual Studio Build Tools)

### Step 1. インストール

**ワンライナーインストール (推奨)** — releaseバイナリを自動取得してPATHに配置:

```bash
# macOS
curl -fsSL https://raw.githubusercontent.com/hang-in/seCall/main/install.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/hang-in/seCall/main/install.ps1 | iex
```

> Linuxのprebuiltバイナリはまだありません — 下記のCargoビルドを利用してください。

**手動ダウンロード** — [Releasesページ](https://github.com/hang-in/seCall/releases) からOSに合ったファイルをダウンロード:
- macOS: `secall-aarch64-apple-darwin.tar.gz` / `secall-x86_64-apple-darwin.tar.gz`
- Windows: `secall-x86_64-pc-windows-msvc.zip` (secall.exe + onnxruntime.dll)

**Cargo (開発者向け)**:

```bash
# CLI/MCP/REST APIのみ (Web UI非同梱)
cargo install --path crates/secall --no-default-features

# Web UI 同梱 — Node 22 + pnpm 9 + just の事前インストールが必要
git clone https://github.com/hang-in/seCall.git && cd seCall
just build         # web/dist ビルド → cargo build --release
cp target/release/secall ~/.local/bin/
```

> `cargo install secall` はnpmビルドを自動では実行しません。Web UIを使う場合はReleasesバイナリまたは上記の手動ビルドを利用してください。

**Homebrew** (予定 — tap登録作業進行中):

```bash
brew install hang-in/tap/secall
```

> **Windowsユーザー**: コア機能 (パース、BM25検索、vault、MCP) は同じように動作します。以下の機能はMSVC非対応のため無効化されます:
> - **HNSW ANNインデックス** (`usearch`) — BLOBコサインスキャンへフォールバック
> - **Kiwi-rs形態素解析** — Lindera ko-dic へフォールバック

### Step 2. 初期化

```bash
# 対話式オンボーディング (推奨)
secall init

# または引数を直接指定
secall init --vault ~/Documents/Obsidian\ Vault/seCall
secall init --git git@github.com:you/obsidian-vault.git
```

`secall init` を引数なしで実行すると対話式ウィザードが起動します:
- ボールトパス設定
- Gitリモート (オプション)
- トークナイザー選択 (lindera/kiwi)
- エンベディングバックエンド選択 (ollama/none)
- Ollama インストール確認 + `bge-m3` モデル自動pull

### Step 3. セッション収集

```bash
# Claude Codeセッション自動検出
secall ingest --auto

# Codex CLI / Gemini CLI
secall ingest ~/.codex/sessions
secall ingest ~/.gemini/sessions

# claude.ai / ChatGPT export (ZIP)
secall ingest ~/Downloads/data-export.zip

# または一括同期
secall sync
```

### Step 4. 検索

```bash
# BM25全文検索
secall recall "BM25インデキシング実装"

# プロジェクト、エージェント、日付フィルター
secall recall "エラー処理" --project seCall --agent claude-code --since 2026-04-01

# ベクトル意味検索 (Ollama必要)
secall recall "検索パイプラインの仕組み" --vec

# LLMクエリ拡張
secall recall "検索精度の改善" --expand
```

## Web UI

`secall serve` はREST APIと一緒にWeb UIを同じポートで提供します (シングルエントリーポイント)。

```bash
secall serve --port 8080
# ブラウザで http://127.0.0.1:8080 にアクセス
```

**Phase 0 機能** (P32、読み取り専用):
- 検索 / セッションブラウジング (2ペインレイアウト)
- 日次日記 / Wikiページ閲覧 (フル本文 — Phase 1でWiki本文fetchを追加)
- グラフ探索 (サイドバーのGraphボタン → フルスクリーンオーバーレイ)
- タグ / お気に入り編集

**Phase 1 機能** (P33、コマンドトリガー):
- サイドバー **Commands** メニュー — Sync / Ingest / Wiki Update ボタン + オプションダイアログ
- SSE進行状況ストリーミング — phase別リアルタイム表示
- グローバル進行状況バナー — どのページにいてもアクティブジョブを追跡 (sticky top)
- 完了/失敗/中断の自動 toast 通知
- 部分成功の明示 (例: 「ingestまでOK / push失敗」)
- 一度に1つのmutating作業のみ実行 (シングルキュー)
- タブを閉じて再接続しても進行中の作業を自動復元

**Phase 2 機能** (P34、ビューア強化):
- セマンティック検索モード切り替え (Ollama使用時)
- 検索語ハイライト — リスト + マークダウン本文の両方
- 複数タグ AND フィルタ + 日付クイックレンジ (今日/今週/今月)
- キーボードショートカット — `?` ヘルプ、`j/k` リスト移動、`/` 検索フォーカス、`g d/w/s/c` ルート、`[/]` セッション prev/next、`f` お気に入り、`e` ノート
- 関連セッションパネル — グラフ隣接 + 同一プロジェクト/タグの推薦 (セッション詳細下部)
- グラフ可視化強化 — dagre自動レイアウト + ノードタイプ別の色/アイコン + エッジラベル切り替え + 凡例
- セッションメタ mini-chart — ターンrole分布 (user/assistant/system) + tool使用頻度 top 5
- ユーザーノート編集 — セッション別 markdown ノート (autosave 1s、`PATCH /api/sessions/{id}/notes`)

**Phase 3 機能** (P35、パフォーマンス + 精度):
- `/api/tags` エンドポイント — 全タグ + 使用頻度を正確に公開 (sessions 100件のヒューリスティック撤廃)
- SessionList 無限スクロール — IntersectionObserver ベースの自動ロード (page_size=100)
- Code-split — ルート別 + vendor (react/query/radix/viz) chunk 分離、初期ロードJS ≤ 250 kB (gzip)

**Job Cancellation** (P36、実行中ジョブのキャンセル):
- 実行中の sync / ingest / wiki-update 作業を安全に中断可能
- `tokio_util::sync::CancellationToken` ベース — `JobRegistry` / `JobExecutor` / `BroadcastSink` 統合、`ProgressSink::is_cancelled()` を公開
- アダプタ (sync/ingest/wiki) が安全ポイントでpolling — phase間、file/sessionループ開始時、LLM呼び出し直前
- 部分結果の保存 — 例: ingest 100件中50件処理後にキャンセル → 結果JSONに `ingested=50` がそのまま記録
- キャンセル時の最終SSEイベント: `Failed { error: "cancelled by user", partial_result: None }`、ジョブ状態は `Interrupted` に強制
- REST: `POST /api/jobs/{id}/cancel` — アクティブ200、idempotent 200、未登録/evict 404
- Web UI: `JobBanner` とアクティブな `JobItem` に **キャンセル** ボタン + `window.confirm` ダイアログ (`useCancelJob` mutation hook)

**Graph Sync 自動化** (P37、セマンティックグラフ再構築):
- 既にingest済みのセッションのセマンティックグラフを別途再構築可能 — embeddingのみ完了したセッションのbackfill、モデル/プロンプト差し替え後の一括再処理など
- DBスキーマ v8: `sessions.semantic_extracted_at` カラムでセマンティック抽出状態を追跡 (NULL = 未処理)
- CLI: `secall graph rebuild [--since DATE] [--session ID] [--all] [--retry-failed]`
- REST: `POST /api/commands/graph-rebuild` — P33 Job システム + P36 cancel と統合
- Web UI: Commandsページ4番目のカード "Graph Rebuild" + オプションダイアログ (since / session / all / retry-failed)
- 優先順位: `--session` > `--all` > `--retry-failed` > `--since` (同時指定時は上の順序で適用) — CLI / REST / Web UI 全て同一

### キーボードショートカット (Phase 2)

| キー | 動作 |
|---|---|
| `?` | ショートカットヘルプ |
| `/` | 検索フォーカス |
| `j` / `k` | リスト次/前項目 |
| `[` / `]` | セッション prev/next |
| `g d` | Daily 画面 |
| `g w` | Wiki 画面 |
| `g s` | Sessions 画面 |
| `g c` | Commands 画面 |
| `g g` | グラフオーバーレイ切り替え |
| `f` | 現在のセッションのお気に入りトグル |
| `e` | 現在のセッションのノート編集 |
| `Esc` | ダイアログ/オーバーレイを閉じる |

### コマンドの利用

Web UIでは左サイドバーの **Commands** メニュー → 任意のコマンド + オプション → 開始。

CLI でも同様に利用可能です (Job システムは Web UI 専用):
```bash
secall sync --local-only --dry-run
secall sync --no-graph         # graph 自動増分を無効化 (sync のデフォルトは有効)
secall ingest --auto --auto-graph   # ingest 時に graph 自動増分を有効化 (デフォルトは無効)
secall wiki update --backend claude

# P37 — セマンティックグラフ再構築 (semantic_extracted_at 状態追跡)
secall graph rebuild --retry-failed              # 未処理 (NULL) セッションを一括 backfill
secall graph rebuild --since 2026-04-01          # 特定日以降のセッション
secall graph rebuild --session abc12345          # 単一セッション
secall graph rebuild --all                       # 全体再構築 (既存結果を上書き)
# 優先順位: --session > --all > --retry-failed > --since (同時指定時は上の順序で適用)
```

### Job システム

コマンドトリガー (sync/ingest/wiki update) はバックグラウンドJobとして実行されます:

1. `POST /api/commands/{kind}` → 即座に `{ job_id, status: "started" }` を返却 (HTTP 202)
2. 進行中の状態はメモリに保存され、高速なSSE/ポーリングが可能 (`Arc<RwLock<HashMap>>`)
3. 完了/失敗時は `jobs` テーブルに永続記録
4. **シングルキュー**: 同時に実行できるmutating作業は1つ — 2つ目のリクエストは `409 Conflict` + `{"error":"another mutating job is running","current_kind":"sync|ingest|wiki_update"}`
5. **Read 作業** (検索、セッション参照など) は同時実行無制限
6. サーバー再起動時、`running`/`started` 状態のjobは自動的に `interrupted` に更新
7. 7日以上経過した完了/失敗/中断jobは起動時に自動cleanup
8. **Cancellation サポート** (P36) — `POST /api/jobs/{id}/cancel` でアクティブjobをキャンセル (200 idempotent / 404 unknown)。アダプタが phase 間・ループ・LLM呼び出し直前の安全ポイントで polling し、部分結果を保存、ジョブ状態は `Interrupted` で終了

#### Phase 分離 (sync の例)

```
sync = init → pull → reindex → ingest → wiki_update → graph → push
```

各 phase 完了ごとに SSE イベントを発行 (`type` discriminator: `initial_state`, `phase_start`, `message`, `progress`, `phase_complete`, `done`, `failed`, KeepAlive 15秒)。push 失敗時も ingest までの結果は保存され、結果 JSON に明示されます:

```json
{
  "pulled": 3,
  "reindexed": 5,
  "ingested": 2,
  "wiki_updated": 1,
  "graph_nodes_added": 12,
  "graph_edges_added": 34,
  "pushed": null,
  "partial_failure": "push: <error>"
}
```

### 開発モード

```bash
just dev    # Vite dev server (5173) + axum (8080) を同時起動
```

`just dev` は Vite を 5173 で起動し、axum が 8080 で reverse proxy します。
- **8080 アクセス**: 単一ポートで全て動作 (HMR は再読み込みが必要)
- **5173 直接アクセス**: HMR 動作、`/api/*` は 8080 にプロキシされる

### ビルド

```bash
just build          # web/dist ビルド + cargo build --release
# または手動:
cd web && pnpm install && pnpm build && cd ..
cargo build --release
```

### 前提条件 (開発時)

- Node 22 + pnpm 9 — `corepack enable` または `npm i -g pnpm`
- [just](https://just.systems) — `brew install just` (オプション、コマンド統合用)

## 使い方

### セッション参照

```bash
# サマリー表示
secall get <session-id>

# フルマークダウン
secall get <session-id> --full

# 特定ターン
secall get <session-id>:5
```

### エンベディング生成

セマンティック検索 (`--vec`) を使うにはベクトルインデックスが必要です。Ollamaがインストールされていれば、`secall embed` または `secall sync` 実行時に自動でエンベディングされます。

```bash
# 新規/変更されたセッションのみエンベディング
secall embed

# 全体再エンベディング
secall embed --all

# パフォーマンスオプション (M1 Max 基準推奨値)
secall embed --concurrency 4 --batch-size 32
```

> ONNX Runtime を使うには `secall config set embedding.backend ort` の後、`secall model download` でモデルをダウンロードしてください。

### セッション分類

config で定義した regex ルールで、収集時にセッションを自動タグ付けします:

```toml
[ingest.classification]
default = "interactive"
skip_embed_types = ["automated"]   # このタイプはベクトルエンベディングをスキップ

[[ingest.classification.rules]]
pattern = "^\\[当月 rawdata\\]"
session_type = "automated"

[[ingest.classification.rules]]
pattern = "^# Wiki Incremental Update Prompt"
session_type = "automated"
```

- **収集時に自動分類** — 最初の user turn の内容を rules の順序でマッチング (最初にマッチしたものを適用)
- **エンベディング選択的スキップ** — `skip_embed_types` で指定したタイプはベクトルエンベディングを省略しコスト削減
- **検索フィルター** — `recall` および MCP `recall` ツールはデフォルトで `automated` セッションを除外 (`--include-automated` フラグで含めることが可能)
- **遡及分類** — `secall classify --dry-run` / `secall classify` で既存セッションを一括再分類

### Wiki生成

```bash
# Claude Code で Wiki 更新 (デフォルト)
secall wiki update

# Codex CLI バックエンド
secall wiki update --backend codex

# ローカル LLM バックエンド
secall wiki update --backend ollama
secall wiki update --backend lmstudio

# Anthropic API (haiku — 直接 API 呼び出し)
secall wiki update --backend haiku

# 特定セッションのみ増分更新
secall wiki update --backend lmstudio --session <id>

# オフライン / 手動 sync モード
secall wiki update --no-pull

# Wiki ステータス確認
secall wiki status
```

### Cross-host 同期 (複数マシン vault)

`secall wiki update` は起動時に vault git repo を検出すると、自動で `auto_commit + pull --rebase` を試みます。

| シナリオ | 動作 |
|---|---|
| 同じトピックの wiki が両方のマシンで更新された | `wiki/*.md` 衝突を検出後、両方の `sources` の和集合で該当ページを自動再生成 |
| wiki 以外のファイル (`raw/`, `log/`, `graph/` など) の衝突 | 自動中断後、手動解決を案内 |
| オフライン または手動 sync | `secall wiki update --no-pull` で git 作業をスキップ |
| 同じトピックの再呼び出し | 既存本文を累積せず新本文に置換、`sources` のみ和集合で保持 |

バックエンドは config からも設定できます:

```toml
[wiki]
default_backend = "lmstudio"   # "claude" | "codex" | "haiku" | "ollama" | "lmstudio"

[wiki.backends.lmstudio]
api_url = "http://localhost:1234"
model = "lmstudio-community/gemma-4-e4b-it"
max_tokens = 3000

[wiki.backends.ollama]
api_url = "http://localhost:11434"
model = "gemma3:27b"

[wiki.backends.claude]
model = "sonnet"   # "opus" も可能
```

### Wiki review (複数 backend)

`secall wiki update --review` は review backend を別途選択できます。

| Backend | 認証 | JSON 信頼性 | コスト |
|---|---|---|---|
| `anthropic` | `ANTHROPIC_API_KEY` | 高い | API 課金 |
| `haiku` | `ANTHROPIC_API_KEY` | 高い | API 課金 |
| `claude` | claude CLI | 中 | subscription |
| `codex` | codex CLI | 中 | subscription |
| `ollama` | なし | モデル次第 | ローカル |
| `lmstudio` | なし | モデル次第 | ローカル |

優先順位:
1. CLI `--review-backend`
2. `[wiki].review_backend`
3. `[wiki].default_backend`
4. fallback `"haiku"`

```bash
secall wiki update --review --review-backend ollama
secall config set wiki.review_backend ollama
```

ローカル backend (`ollama`, `lmstudio`) は `docs/prompts/wiki-review-strict-json.md` の strict JSON suffix を自動で付加して再試行します。

### 作業日記

日付別の作業日記を自動生成します:

```bash
# 今日の日記を生成
secall log

# 特定日を指定
secall log 2026-04-15
```

- プロジェクト別にセッションをグループ化し、トピックノードを Knowledge Graph から抽出
- Ollama/Gemini LLM で散文整理 (LLM 未設定時はテンプレート fallback)
- 結果を `vault/log/{date}.md` に保存

### Knowledge Graph

```bash
# 全グラフをビルド
secall graph build

# 統計確認
secall graph stats

# graph.json エクスポート
secall graph export
```

## 設定

`secall config` コマンドで設定を管理します。必要であれば Web UI `/settings` と REST `/api/config` でも同じ設定を確認できます。

```bash
# 現在の設定を確認
secall config show
secall config llm show

# 設定変更
secall config set output.timezone Asia/Tokyo
secall config set search.tokenizer kiwi
secall config set embedding.backend ollama
secall config llm set log.backend haiku

# 設定ファイルパスを確認
secall config path

# Web UI から設定を編集 (デフォルトは read-only)
secall serve --port 8080 --allow-config-edit
```

### 設定キー一覧

| キー | 説明 | デフォルト |
|---|---|---|
| `vault.path` | Obsidian vault パス | `~/obsidian-vault/seCall` |
| `vault.git_remote` | Git remote URL | (なし) |
| `vault.branch` | Git ブランチ名 | `main` |
| `search.tokenizer` | トークナイザー (`lindera` / `kiwi`) | `lindera` |
| `search.default_limit` | 検索結果数 | `10` |
| `embedding.backend` | エンベディングバックエンド (`ollama` / `ort` / `openai` / `openvino` / `ollama_cloud`) | `ollama` |
| `embedding.ollama_model` | Ollama モデル名 | `bge-m3` |
| `embedding.pool_size` | ORT session pool サイズ (未設定 = RAM ベース自動) | `null` |
| `embedding.cloud_host` | Ollama Cloud API ホスト | `https://ollama.com` |
| `embedding.cloud_model` | Ollama Cloud embedding モデル名 | `null` |
| `output.timezone` | タイムゾーン (IANA) | `UTC` |
| `ingest.classification.default` | 分類ルール未マッチ時のデフォルト session_type | `interactive` |
| `ingest.classification.skip_embed_types` | エンベディングをスキップする session_type 一覧 | `[]` |
| `graph.semantic_backend` | セマンティックエッジ抽出バックエンド (`ollama_cloud` / `ollama` / `lmstudio` / `anthropic` / `none`) | `none` |
| `graph.cloud_model` | Ollama Cloud セマンティックモデル | `gemma4:31b-cloud` |
| `graph.cloud_host` | Ollama Cloud API ホスト | `https://ollama.com` |
| `graph.ollama_model` | Ollama/LM Studio セマンティックモデル | `gemma4:e4b` / `gemma-4-e4b-it` |
| `wiki.default_backend` | Wiki 生成バックエンド (`claude` / `codex` / `haiku` / `ollama` / `lmstudio`) | `claude` |
| `wiki.review_backend` | Wiki review バックエンド (`anthropic` / `claude` / `codex` / `haiku` / `ollama` / `lmstudio`) | `wiki.default_backend` フォールバック |
| `wiki.review_model` | Wiki review モデル override | `sonnet` |
| `wiki.backends.<name>.api_url` | バックエンド API エンドポイント | (デフォルト値使用) |
| `wiki.backends.<name>.model` | バックエンドモデル名 | (デフォルト値使用) |
| `wiki.backends.<name>.max_tokens` | 最大生成トークン数 | `4096` |
| `log.backend` | Daily diary バックエンド (`claude` / `codex` / `haiku` / `ollama` / `lmstudio`) | `graph.semantic_backend` フォールバック |
| `log.model` | Daily diary モデル override | backend デフォルト値 |
| `log.api_url` | Daily diary API URL override | backend デフォルト値 |
| `log.max_tokens` | Daily diary 最大生成トークン数 | backend デフォルト値 |

設定ファイルパス:
- **macOS**: `~/Library/Application Support/secall/config.toml`
- **Linux**: `~/.config/secall/config.toml`
- **Windows**: `%APPDATA%\secall\config.toml`

## CLIリファレンス

| コマンド | 説明 |
|---|---|
| `secall init` | 対話式オンボーディング (vault、トークナイザー、エンベディング設定) |
| `secall ingest [path] --auto [--auto-graph]` | エージェントセッションのパース・インデックシング (`--auto-graph` で graph 自動増分を有効化、デフォルトは無効) |
| `secall sync [--local-only] [--no-wiki] [--no-semantic] [--no-graph]` | 完全同期: init → pull → reindex → ingest → wiki_update → graph → push (`--no-graph` で graph フェーズをスキップ) |
| `secall recall <query>` | ハイブリッド検索 (デフォルト: automated セッション除外) |
| `secall recall <query> --include-automated` | automated セッションを含めて検索 |
| `secall get <id> [--full]` | セッション詳細表示 |
| `secall status` | インデックス統計 + 設定サマリー |
| `secall embed [--all]` | ベクトルエンベディング生成 |
| `secall classify [--dry-run]` | config ルールで既存セッションを一括再分類 |
| `secall lint` | インデックス/ボールト整合性検証 |
| `secall mcp [--http <addr>]` | MCP サーバー起動 |
| `secall config show\|set\|path` | 設定の確認/変更 |
| `secall config llm show\|set\|where` | LLM 関連設定のみ参照/変更 |
| `secall graph build\|stats\|export` | Knowledge Graph 管理 |
| `secall graph rebuild [--since <date>\|--session <id>\|--all\|--retry-failed]` | セマンティックグラフ再構築 (P37) — 優先順位: `--session` > `--all` > `--retry-failed` > `--since` |
| `secall wiki update [--backend claude\|codex\|haiku\|ollama\|lmstudio] [--review] [--review-backend <name>]` | Wiki 生成 + optional review |
| `secall wiki status` | Wiki ステータス確認 |
| `secall log [YYYY-MM-DD] [--backend <name>] [--model <name>]` | 日付別作業日記の生成 |
| `secall serve [--port <port>] [--allow-config-edit]` | REST API + Web UI サーバー起動 (`/settings` 保存は flag が必要) |
| `secall model download\|info\|check` | ONNX モデル管理 |
| `secall reindex --from-vault` | ボールトから DB を再構築 |
| `secall migrate summary` | summary frontmatter の一括追加 |

## MCP連携

Claude Code 設定 (`~/.claude/settings.json`) に追加:

```json
{
  "mcpServers": {
    "secall": {
      "command": "secall",
      "args": ["mcp"]
    }
  }
}
```

セッション開始/終了時の自動同期:

```json
{
  "hooks": {
    "SessionStart": [{
      "matcher": "startup|resume",
      "hooks": [{"type": "command", "command": "secall sync --local-only"}]
    }],
    "SessionEnd": [{
      "hooks": [{"type": "command", "command": "secall sync"}]
    }]
  }
}
```

> 詳しい設定方法は [GitHub ボールト同期ガイド](docs/reference/github-vault-sync.md) を参照してください。

## アーキテクチャ

![seCall アーキテクチャ](arch_v0.png)

## 技術スタック

| 分類 | 技術 |
|---|---|
| 言語 | Rust 1.75+ (2021 edition) |
| データベース | SQLite + FTS5 (rusqlite, bundled) |
| 韓国語 NLP | Lindera ko-dic + Kiwi-rs 形態素解析 (macOS/Linux) |
| プラットフォーム | macOS, Windows (x86_64), Linux (CI) |
| エンベディング | Ollama BGE-M3 (1024次元) / ONNX Runtime (オプション) |
| ANN インデックス | usearch HNSW (macOS/Linux) |
| MCP サーバー | rmcp (stdio + Streamable HTTP / axum) |
| ボールト | Obsidian 互換 Markdown |
| REST API | axum (CORS 対応) |
| Wiki エンジン | Claude Code / Codex CLI / Ollama / LM Studio / Gemini (プラグイン方式バックエンド) |
| Obsidian プラグイン | obsidian-secall (TypeScript, esbuild) |

## 出典

本プロジェクトは以下のアイデア・プロジェクトをベースにしています:

- **[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)** (Andrej Karpathy) — LLM を用いて原本ソースから段階的にナレッジベースを構築するパターン。seCall の 2層ボールトアーキテクチャ (原本セッション + AI 生成 Wiki) はこのコンセプトを直接実装したものです。[Tobi Lütke の実装](https://github.com/tobi/llm-wiki) も参考。
- **[qmd](https://github.com/tobi/qmd)** (Tobi Lütke) — マークダウンファイル向けローカル検索エンジン。seCall の検索パイプライン (FTS5 BM25、ベクトルエンベディング、RRF k=60) は qmd のアプローチを参考に設計されています。
- **[graphify](https://github.com/safishamsi/graphify)** (Safi Shamsi) — ファイルフォルダを knowledge graph に変換するツール。seCall P16 の決定論的グラフ抽出と confidence ラベリングはこのプロジェクトに着想を得ています。

本プロジェクトは AI コーディングエージェント (Claude Code, Codex) を [tunaFlow](https://github.com/hang-in/tunaFlow) マルチエージェントワークフロープラットフォームでオーケストレーションして開発されました。

## ライセンス

[AGPL-3.0](LICENSE)

## 更新履歴

> NOTE: git tag (v0.x.x) が SSOT。下表の P34〜P44 は v0.4.0 release の内部 phase、P49〜P56 は v0.5.0 release の phase。

| 日付 | バージョン/Phase | 変更内容 |
|------|------|---------|
| 2026-05-15 | **v0.5.0** | 累積 release (P49〜P56) — TMPDIR/secall-prompt ノイズの ingest 遮断 (P49) + `raw/sessions/` → `raw/.sessions/` rename (obsidian 自動非表示、breaking)、`LlmBackend` trait + 4 バックエンド統合 (P50-B)、wiki/ingest 巨大関数の分解 (P50-C/D/E)、graph/log のデフォルトを `ollama_cloud` に (P51、breaking)、wiki 4 バックエンドの `generate()` に 300s timeout — `kill_on_drop` (P52)、wiki `--since` ターゲット表示の精度向上 (P53)、`secall lint --fix-orphan-vault` (P54)、`ollama_cloud` wiki review/generation バックエンド (P55)、`WikiBackendConfig.cloud_*` フィールド + claude CLI `haiku` alias (P56) |
| 2026-05-10 | P44 (v0.4.0+) | Wiki cross-host merge: `wiki update` 起動時に自動 `auto_commit + pull`、`wiki/*.md` 衝突時は両方の `sources` 和集合ベースで自動再生成、`--no-pull` 追加、`merge_with_existing()` の本文累積を撤廃 |
| 2026-05-09 | P43 (v0.4.0+) | Wiki review backend 拡張: `wiki update --review` が `claude` / `codex` / `haiku` / `ollama` / `lmstudio` / `anthropic` backend をサポート、`[wiki].review_backend` + `--review-backend` 追加、`toml_edit` ベースの config 保存でユーザーコメントを保持、`docs/reference/llm-config.md` 追加 |
| 2026-05-09 | P41 (v0.4.0+) | LLM 設定の統合: `secall log --backend/--model`、新規 `[log]` セクション、ハードコードされたデフォルトモデルの定数化 + warning、`GET /api/config` / `PATCH /api/config/{section}`、Web `/settings`、`secall config llm show\|set\|where` |
| 2026-05-06 | P40 (v0.4.0) | Wiki 検索ハイブリッドモード: `wiki_vectors` テーブル (DB v9、ページレベルエンベディング、bge-m3 + Ollama)、SHA-256 content-hash ベースの idempotent インデキシング + orphan 整理の `WikiIndexer`、`do_wiki_search` に `mode={keyword\|semantic\|hybrid}` パラメータ (デフォルト `keyword` — 互換)、hybrid は RRF (k=60) で結合、Ollama 不可 / エンベディング失敗時は自動で keyword fallback、新規 CLI `secall wiki vectorize [--force] [--model bge-m3] [--ollama-url ...]` で一度きりの backfill、回帰カバレッジ `tests/{db_migrations,wiki_indexer,wiki_search_modes}.rs` |
| 2026-05-05 | P39 (v0.4.0) | wiki パイプライン baseline + sync auto-commit fix + dotenv autoload: `VaultGit::auto_commit` が `git add -A` で SCHEMA.md / graph/ / log/ などを全て stage (`crates/secall-core/src/vault/git.rs:146`、8 回帰 tests `tests/vault_auto_commit.rs`)、`secall` バイナリ起動時に `dotenvy::dotenv()` autoload (`crates/secall/src/main.rs:382` — Gemini/OpenAI キー環境変数の自動注入)、683 セッション sync baseline 測定 (`docs/baseline/p39-wiki-baseline.md` / `p39-wiki-quality.md` / `p39-p40-decision.md`)、`graph rebuild --since 2026-05-05` 28 sessions / 840 edges backfill |
| 2026-05-03 | P38 (v0.4.0) | テストギャップ補強: `tests/rest_routes.rs` (REST 22 エンドポイントのルートレベル回帰、45 tests) + `tests/session_repo_helpers.rs` (P32〜P37 累積 helper 回帰、29 tests) — 計 74 P38 新規 tests を追加、Insight TES-session_repo finding を解消 |
| 2026-05-03 | P37 (v0.4.0) | Graph Sync 自動化: DB スキーマ v8 (`sessions.semantic_extracted_at` カラムでセマンティック抽出状態を追跡)、`secall graph rebuild [--since\|--session\|--all\|--retry-failed]` CLI (`extract_one_session_semantic` helper 分離、優先順位: `--session` > `--all` > `--retry-failed` > `--since`)、`POST /api/commands/graph-rebuild` REST (`JobKind::GraphRebuild`、P33 シングルキュー + P36 cancel と統合)、Web UI Commands ページ 4 番目カード "Graph Rebuild" + オプションダイアログ |
| 2026-05-02 | P36 (v0.4.0) | Job Cancellation: `tokio_util::sync::CancellationToken` 統合 (`JobRegistry`/`JobExecutor`/`BroadcastSink`)、`ProgressSink::is_cancelled()` 追加、sync/ingest/wiki アダプタの safe-point polling (phase 間・file/session ループ・LLM 呼び出し直前)、部分結果の保存、`POST /api/jobs/{id}/cancel` 有効化 (200 idempotent / 404 unknown、最終イベント `Failed { error: "cancelled by user" }` + status=`Interrupted`)、Web UI キャンセルボタン (`JobBanner`/`JobItem`、`useCancelJob` + `window.confirm`) |
| 2026-05-02 | P35 (v0.4.0) | Web UI Phase 3: `/api/tags` エンドポイント (with_counts オプション、100 セッションヒューリスティック撤廃)、SessionList 無限スクロール (IntersectionObserver、page_size=100)、Code-split (vendor react/query/radix/viz + per-route chunk、初期ロード JS ≤ 250 kB gzip) |
| 2026-05-02 | P34 (v0.4.0) | Web UI Phase 2: セマンティック検索モード有効化、検索語ハイライト、複数タグ + 日付クイックレンジ、キーボードショートカット (`?`/`/`/`j`/`k`/`[`/`]`/`g d/w/s/c/g`/`f`/`e`)、関連セッションパネル、グラフ可視化強化 (dagre + ノード色/アイコン + 凡例)、セッションメタ mini-chart、ユーザーノート編集 (`PATCH /api/sessions/{id}/notes`)、DB スキーマ v7 |
| 2026-05-02 | v0.4.0 | Web UI Phase 1 (P33): コマンドトリガー (Sync/Ingest/Wiki Update)、SSE 進行状況ストリーミング (phase 別)、Job システム (シングルキュー + 7 日 cleanup + interrupted 補正)、グローバル進行状況バナー + toast、グラフ自動増分 (`secall ingest --auto-graph`, `secall sync --no-graph`)、Wiki 本文 GET エンドポイント (`/api/wiki/{project}`)、DB v6 (`jobs` テーブル) |
| 2026-04-17 | v0.3.3 | LM Studio (OpenAI 互換) セマンティックバックエンド追加 (`--backend lmstudio`、#35)、`secall sync --no-semantic` フラグ追加 — GPU メモリ競合の回避 (#34)、Gemini Web ZIP ingest サポート (#31)、`graph semantic` CLI バックエンド設定オプション (#30) |
| 2026-04-15 | v0.3.2 | Gemini API バックエンド (セマンティックグラフ + 日記生成)、Codex wiki バックエンド (PR #29)、REST API サーバー (`secall serve`)、Obsidian プラグイン (検索/デイリー/グラフビュー)、作業日記 (`secall log`)、セマンティックエッジ (`fixes_bug`、`modifies_file`、`introduces_tech`、`discusses_topic`)、BM25-only モード時に graph semantic を自動無効化 (#25) |
| 2026-04-12 | v0.3.1 | `secall lint --fix` で stale DB を整理 (#15)、`wiki_search` に created/updated フィールド (#13)、P20 テストカバレッジ強化 (+16 tests) |
| 2026-04-12 | v0.3.0 | セッション分類 (regex ルール、`secall classify`)、Wiki プラグインバックエンド (Ollama、LM Studio)、`--include-automated` フラグ |
| 2026-04-10 | P17 | 対話式オンボーディング (`secall init` ウィザード)、`secall config` CLI、git ブランチ設定 |
| 2026-04-10 | P16 | Knowledge Graph — frontmatter ベースの決定論的グラフ抽出、`secall graph build/stats/export`、MCP `graph_query`、sync Phase 3.7 |
| 2026-04-09 | P15 | Windows ランタイム修正 — Ollama NaN を許容、クロスプラットフォーム `command_exists`、sync 衝突の事前検査 |
| 2026-04-09 | P14 | 検索品質 — 独立ベクトル実行、セッションレベルの結果多様性 |
| 2026-04-09 | P13 | Windows ビルドサポート — `x86_64-pc-windows-msvc` CI/Release、ORT DLL 同梱 |
| 2026-04-09 | v0.2.3 | ChatGPT エクスポートパーサー — `conversations.json` (ZIP)、マッピングツリーの線形化 |
| 2026-04-08 | v0.2.2 | タイムゾーン設定 — IANA タイムゾーン変換でボールトタイムスタンプを現地化 |
| 2026-04-08 | v0.2.1 | `--force` 再収集 + Dataview `::` エスケープ + AGPL-3.0 LICENSE |
| 2026-04-07 | P11 | エンベディング性能 — ORT セッションプール、バッチ推論、並列化 (49h → 約3-4h) |
| 2026-04-07 | P10 | セッション `summary` frontmatter — 最初の user turn から自動生成 |
| 2026-04-06 | P8 | 安定化 + GitHub Actions リリースワークフロー |
| 2026-04-06 | P7 | `--min-turns`、`embed --all`、`wiki_search` MCP ツール、`--no-wiki` |
| 2026-04-05 | v0.2 | claude.ai エクスポートパーサー、ZIP 自動解凍 |
| 2026-04-05 | P6 | ANN インデックス (usearch HNSW) |
| 2026-04-04 | P5 | マルチデバイスボールト Git 同期、`secall sync`、`reindex --from-vault` |
| 2026-03-31 | MVP | 最初のリリース — Claude Code/Codex/Gemini パーサー、BM25+ベクトル検索、MCP サーバー、Obsidian ボールト |

---

<div align="center">

**Contact**: [d9ng@outlook.com](mailto:d9ng@outlook.com)

</div>
