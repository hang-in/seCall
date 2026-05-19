mod commands;
mod output;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use output::OutputFormat;

#[derive(Parser)]
#[command(name = "secall", version, about = "Agent session search engine")]
struct Cli {
    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize vault and config
    Init {
        /// Vault path
        #[arg(short, long)]
        vault: Option<PathBuf>,
        /// Git remote URL for vault sync
        #[arg(long)]
        git: Option<String>,
    },

    /// Ingest agent session logs
    Ingest {
        /// Session file path, session ID, or use --auto
        path: Option<String>,

        /// Auto-detect new sessions from ~/.claude/projects/
        #[arg(long)]
        auto: bool,

        /// Filter by project directory
        #[arg(long)]
        cwd: Option<PathBuf>,

        /// Skip sessions with fewer turns than this (0 = no filter)
        #[arg(long, default_value = "0")]
        min_turns: usize,

        /// Re-ingest already-indexed sessions (overwrite vault + DB)
        #[arg(long)]
        force: bool,

        /// Skip semantic edge extraction during ingest
        #[arg(long)]
        no_semantic: bool,

        /// Skip vector embedding (BM25/structure indexing only). Run `secall embed` separately to fill in vectors later.
        #[arg(long)]
        no_embed: bool,

        /// Automatically run graph incremental extraction for new sessions after ingest
        #[arg(long)]
        auto_graph: bool,
    },

    /// Search session history
    Recall {
        /// Search query (multiple words joined)
        query: Vec<String>,

        /// Temporal filter: today, yesterday, last week, since YYYY-MM-DD
        #[arg(long)]
        since: Option<String>,

        /// Filter by project
        #[arg(long, short)]
        project: Option<String>,

        /// Filter by agent
        #[arg(long, short)]
        agent: Option<String>,

        /// Max results
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,

        /// BM25-only (skip vector search)
        #[arg(long)]
        lex: bool,

        /// Vector-only (skip BM25)
        #[arg(long)]
        vec: bool,

        /// Expand query using Claude Code (requires claude CLI)
        #[arg(long)]
        expand: bool,

        /// Include automated sessions in search results (excluded by default)
        #[arg(long)]
        include_automated: bool,

        /// Skip related session graph traversal in output
        #[arg(long)]
        no_related: bool,

        /// Filter by topic node in knowledge graph (e.g., "rust async")
        #[arg(long)]
        topic: Option<String>,

        /// Filter by file node in knowledge graph (e.g., "src/main.rs")
        #[arg(long)]
        file: Option<String>,

        /// Filter by issue node in knowledge graph (e.g., "#42")
        #[arg(long)]
        issue: Option<String>,
    },

    /// Get a specific session or turn
    Get {
        /// Session ID or session_id:turn_index
        id: String,

        /// Show full markdown content
        #[arg(long)]
        full: bool,
    },

    /// Show index status
    Status,

    /// Generate vector embeddings for un-embedded sessions
    Embed {
        /// Re-embed all sessions
        #[arg(long)]
        all: bool,

        /// Embedding batch size (default: 32)
        #[arg(long)]
        batch_size: Option<usize>,

        /// Number of sessions to embed concurrently (default: 4)
        #[arg(long, default_value = "4")]
        concurrency: usize,
    },

    /// Classify sessions using config rules (backfill existing sessions)
    Classify {
        /// Preview changes without writing to DB
        #[arg(long)]
        dry_run: bool,
    },

    /// Verify index and vault integrity
    Lint {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Only show errors (skip warn/info)
        #[arg(long)]
        errors_only: bool,

        /// Auto-fix: delete stale DB records for missing vault files (L001)
        #[arg(long)]
        fix: bool,

        /// Auto-fix orphan vault files (L002) — move vault md to archive if not in DB
        #[arg(long)]
        fix_orphan_vault: bool,

        /// P84 (issue #82): Auto-archive wiki invocation sessions (L011) — codex/claude
        /// sessions whose cwd matches the vault path are wiki self-invocations, archived
        /// to prevent self-ingest loops in legacy data (pre-P83 ingest).
        #[arg(long)]
        fix_wiki_invocations: bool,
    },

    /// Start MCP server
    Mcp {
        /// Start HTTP server instead of stdio (e.g., --http 127.0.0.1:8080)
        #[arg(long)]
        http: Option<String>,
    },

    /// Start REST API server for Obsidian plugin and external clients
    Serve {
        /// Port number (default: 8080)
        #[arg(long, short, default_value = "8080")]
        port: u16,

        /// Allow PATCH /api/config/* writes (local-only, dangerous if externally exposed)
        #[arg(long)]
        allow_config_edit: bool,
    },

    /// Manage ONNX embedding models
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },

    /// Sync vault with remote (git pull -> reindex -> ingest -> git push)
    Sync {
        /// Skip git pull/push (local-only reindex + ingest)
        #[arg(long)]
        local_only: bool,

        /// Dry run — show what would happen without executing
        #[arg(long)]
        dry_run: bool,

        /// Skip incremental wiki generation for new sessions
        #[arg(long)]
        no_wiki: bool,

        /// Skip semantic edge extraction during ingest
        #[arg(long)]
        no_semantic: bool,

        /// Skip graph incremental extraction (default: enabled)
        #[arg(long)]
        no_graph: bool,

        /// Skip vector embedding during ingest phase (BM25/structure indexing only). Run `secall embed` separately to fill in vectors later.
        #[arg(long)]
        no_embed: bool,
    },

    /// Rebuild DB index from vault markdown files
    Reindex {
        /// Rebuild from vault markdown files
        #[arg(long)]
        from_vault: bool,
    },

    /// Manage wiki generation via pluggable LLM backends
    Wiki {
        #[command(subcommand)]
        action: WikiAction,
    },

    /// Run data migrations
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },

    /// Build and query knowledge graph
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },

    /// Generate daily work log from sessions
    Log {
        /// 날짜 (YYYY-MM-DD). 생략 시 오늘
        date: Option<String>,

        /// Backend: claude | codex | haiku | ollama | lmstudio
        #[arg(long)]
        backend: Option<String>,

        /// Model name (backend-dependent)
        #[arg(long)]
        model: Option<String>,
    },

    /// View or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Config key (e.g. search.tokenizer, embedding.backend)
        key: String,
        /// New value
        value: String,
    },
    /// Show config file path
    Path {
        /// Copy the config path to the clipboard when supported
        #[arg(long)]
        copy: bool,
    },
    /// LLM-focused config helpers
    Llm {
        #[command(subcommand)]
        action: LlmAction,
    },
}

#[derive(Subcommand)]
enum LlmAction {
    /// Show only LLM-related configuration
    Show,
    /// Set one LLM-related config value
    Set { key: String, value: String },
    /// Test LLM backend connectivity and credentials
    Test {
        /// Backend: claude | codex | haiku | ollama | lmstudio | gemini
        backend: Option<String>,
        /// Skip outbound network calls and only verify local prerequisites
        #[arg(long)]
        no_network: bool,
    },
    /// Show config file location and LLM entry points
    Where,
}

#[derive(Subcommand)]
enum ModelAction {
    /// Download bge-m3 ONNX model from HuggingFace
    Download {
        #[arg(long)]
        force: bool,
    },
    /// Check for model updates
    Check,
    /// Remove downloaded model
    Remove,
    /// Show model info (path, size, version)
    Info,
}

#[derive(Subcommand)]
enum WikiAction {
    /// Run wiki update using a configurable LLM backend
    Update {
        /// Model name (backend-dependent). Claude defaults to sonnet, Codex defaults to gpt-5.4
        #[arg(long)]
        model: Option<String>,

        /// Backend: claude | codex | haiku | ollama | lmstudio (기본값: config wiki.default_backend)
        #[arg(long)]
        backend: Option<String>,

        /// Only process sessions since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Incremental mode: update for a specific session
        #[arg(long)]
        session: Option<String>,

        /// Print the prompt without executing the selected backend
        #[arg(long)]
        dry_run: bool,

        /// Review generated pages with Sonnet/Opus after generation
        #[arg(long)]
        review: bool,

        /// Review backend: claude | codex | haiku | ollama | lmstudio
        #[arg(long)]
        review_backend: Option<String>,

        /// Review model: sonnet or opus (default: config or sonnet)
        #[arg(long)]
        review_model: Option<String>,

        /// Skip git pull/auto-commit at start (offline / manual sync mode)
        #[arg(long)]
        no_pull: bool,
    },

    /// Show wiki status (page count, last update)
    Status,

    /// Backfill wiki page embeddings for semantic/hybrid search
    Vectorize {
        /// Ignore content hash and reindex every page
        #[arg(long)]
        force: bool,

        /// Embedding model ID
        #[arg(long, default_value = "bge-m3")]
        model: String,

        /// Ollama base URL
        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,
    },
}

#[derive(Subcommand)]
enum MigrateAction {
    /// Backfill summary field for existing sessions
    Summary {
        /// Dry run — show what would be changed without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    /// Re-extract semantic edges (LLM) for all sessions without rebuilding embeddings
    Semantic {
        /// 세션당 요청 사이 대기 시간(초). 소수점 가능 (기본: 2.5)
        #[arg(long, default_value_t = 2.5)]
        delay: f64,
        /// 처리할 최대 세션 수 (기본: 전체)
        #[arg(long)]
        limit: Option<usize>,
        /// LLM 백엔드 오버라이드: "ollama" | "gemini" | "anthropic" | "lmstudio" | "disabled"
        #[arg(long)]
        backend: Option<String>,
        /// API base URL (예: http://localhost:11434, Ollama 전용)
        #[arg(long)]
        api_url: Option<String>,
        /// 모델명 오버라이드 (예: gemma4:e4b, gemini-2.5-flash)
        #[arg(long)]
        model: Option<String>,
        /// API 키 오버라이드 (Gemini 등). 보안상 환경변수 SECALL_GRAPH_API_KEY 사용 권장
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Build graph from vault sessions
    Build {
        /// Only process sessions since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Force rebuild (clear existing graph)
        #[arg(long)]
        force: bool,
    },
    /// Show graph statistics
    Stats,
    /// Export graph to vault/graph/graph.json
    Export,
    /// Rebuild semantic edges for selected sessions (P37)
    Rebuild {
        /// Only process sessions started since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Single session ID (overrides other filters)
        #[arg(long)]
        session: Option<String>,

        /// Process all sessions (overrides --retry-failed and --since)
        #[arg(long)]
        all: bool,

        /// Only process sessions with NULL semantic_extracted_at
        #[arg(long)]
        retry_failed: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // P39 — `.env` 자동 로드 (cwd 또는 부모 디렉토리). secall sync 등 명령이
    // OLLAMA_CLOUD_API_KEY 같은 env var 를 사용하기 전에 로드. 파일 없으면
    // silently skip (운영 환경에서 env var 직접 export 한 경우 문제 없음).
    let _ = dotenvy::dotenv();

    // stderr 전용 — stdout은 MCP 프로토콜 전용
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { vault, git } => {
            commands::init::run(vault, git)?;
        }
        Commands::Ingest {
            path,
            auto,
            cwd,
            min_turns,
            force,
            no_semantic,
            no_embed,
            auto_graph,
        } => {
            commands::ingest::run(
                path,
                auto,
                cwd,
                min_turns,
                force,
                no_semantic,
                no_embed,
                auto_graph,
                &cli.format,
            )
            .await?;
        }
        Commands::Recall {
            query,
            since,
            project,
            agent,
            limit,
            lex,
            vec,
            expand,
            include_automated,
            no_related,
            topic,
            file,
            issue,
        } => {
            commands::recall::run(
                query,
                since,
                project,
                agent,
                limit,
                lex,
                vec,
                expand,
                include_automated,
                no_related,
                topic,
                file,
                issue,
                &cli.format,
            )
            .await?;
        }
        Commands::Get { id, full } => {
            commands::get::run(id, full)?;
        }
        Commands::Status => {
            commands::status::run()?;
        }
        Commands::Embed {
            all,
            batch_size,
            concurrency,
        } => {
            commands::embed::run(all, batch_size, concurrency).await?;
        }
        Commands::Classify { dry_run } => {
            commands::classify::run_backfill(dry_run).await?;
        }
        Commands::Lint {
            json,
            errors_only,
            fix,
            fix_orphan_vault,
            fix_wiki_invocations,
        } => {
            commands::lint::run(
                json,
                errors_only,
                fix,
                fix_orphan_vault,
                fix_wiki_invocations,
            )?;
        }
        Commands::Mcp { http } => {
            commands::mcp::run(http).await?;
        }
        Commands::Serve {
            port,
            allow_config_edit,
        } => {
            commands::serve::run(port, allow_config_edit).await?;
        }
        Commands::Model { action } => match action {
            ModelAction::Download { force } => {
                commands::model::run_download(force).await?;
            }
            ModelAction::Check => {
                commands::model::run_check().await?;
            }
            ModelAction::Remove => {
                commands::model::run_remove()?;
            }
            ModelAction::Info => {
                commands::model::run_info()?;
            }
        },
        Commands::Sync {
            local_only,
            dry_run,
            no_wiki,
            no_semantic,
            no_graph,
            no_embed,
        } => {
            commands::sync::run(
                local_only,
                dry_run,
                no_wiki,
                no_semantic,
                no_graph,
                no_embed,
            )
            .await?;
        }
        Commands::Reindex { from_vault } => {
            commands::reindex::run(from_vault)?;
        }
        Commands::Wiki { action } => match action {
            WikiAction::Update {
                model,
                backend,
                since,
                session,
                dry_run,
                review,
                review_backend,
                review_model,
                no_pull,
            } => {
                commands::wiki::run_update(
                    model.as_deref(),
                    backend.as_deref(),
                    since.as_deref(),
                    session.as_deref(),
                    dry_run,
                    review,
                    review_backend.as_deref(),
                    review_model.as_deref(),
                    no_pull,
                )
                .await?;
            }
            WikiAction::Status => {
                commands::wiki::run_status()?;
            }
            WikiAction::Vectorize {
                force,
                model,
                ollama_url,
            } => {
                commands::wiki::vectorize(force, &model, &ollama_url).await?;
            }
        },
        Commands::Migrate { action } => match action {
            MigrateAction::Summary { dry_run } => {
                commands::migrate::run_summary(dry_run)?;
            }
        },
        Commands::Log {
            date,
            backend,
            model,
        } => {
            commands::log::run(date, backend, model).await?;
        }
        Commands::Graph { action } => match action {
            GraphAction::Semantic {
                delay,
                limit,
                backend,
                api_url,
                model,
                api_key,
            } => {
                commands::graph::run_semantic(delay, limit, backend, api_url, model, api_key)
                    .await?;
            }
            GraphAction::Build { since, force } => {
                commands::graph::run_build(since.as_deref(), force)?;
            }
            GraphAction::Stats => {
                commands::graph::run_stats()?;
            }
            GraphAction::Export => {
                commands::graph::run_export()?;
            }
            GraphAction::Rebuild {
                since,
                session,
                all,
                retry_failed,
            } => {
                commands::graph::run_rebuild_cli(commands::graph::GraphRebuildArgs {
                    since,
                    session,
                    all,
                    retry_failed,
                })
                .await?;
            }
        },
        Commands::Config { action } => match action {
            ConfigAction::Show => {
                commands::config::run_show()?;
            }
            ConfigAction::Set { key, value } => {
                commands::config::run_set(&key, &value)?;
            }
            ConfigAction::Path { copy } => {
                commands::config::run_path(copy)?;
            }
            ConfigAction::Llm { action } => match action {
                LlmAction::Show => {
                    commands::config::run_llm_show()?;
                }
                LlmAction::Set { key, value } => {
                    commands::config::run_set(&key, &value)?;
                }
                LlmAction::Test {
                    backend,
                    no_network,
                } => {
                    commands::config::run_llm_test(backend, no_network).await?;
                }
                LlmAction::Where => {
                    commands::config::run_llm_where()?;
                }
            },
        },
    }

    Ok(())
}
