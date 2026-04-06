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
    },

    /// Verify index and vault integrity
    Lint {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Only show errors (skip warn/info)
        #[arg(long)]
        errors_only: bool,
    },

    /// Start MCP server
    Mcp {
        /// Start HTTP server instead of stdio (e.g., --http 127.0.0.1:8080)
        #[arg(long)]
        http: Option<String>,
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
    },

    /// Rebuild DB index from vault markdown files
    Reindex {
        /// Rebuild from vault markdown files
        #[arg(long)]
        from_vault: bool,
    },

    /// Manage wiki generation via Claude Code meta-agent
    Wiki {
        #[command(subcommand)]
        action: WikiAction,
    },
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
    /// Run wiki update using Claude Code as meta-agent
    Update {
        /// Model: opus or sonnet
        #[arg(long, default_value = "sonnet")]
        model: String,

        /// Only process sessions since this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Incremental mode: update for a specific session
        #[arg(long)]
        session: Option<String>,

        /// Print the prompt without executing Claude Code
        #[arg(long)]
        dry_run: bool,
    },

    /// Show wiki status (page count, last update)
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Commands::Ingest { path, auto, cwd } => {
            commands::ingest::run(path, auto, cwd, &cli.format).await?;
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
        Commands::Embed { all } => {
            commands::embed::run(all).await?;
        }
        Commands::Lint { json, errors_only } => {
            commands::lint::run(json, errors_only)?;
        }
        Commands::Mcp { http } => {
            commands::mcp::run(http).await?;
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
        } => {
            commands::sync::run(local_only, dry_run).await?;
        }
        Commands::Reindex { from_vault } => {
            commands::reindex::run(from_vault)?;
        }
        Commands::Wiki { action } => match action {
            WikiAction::Update {
                model,
                since,
                session,
                dry_run,
            } => {
                commands::wiki::run_update(&model, since.as_deref(), session.as_deref(), dry_run)
                    .await?;
            }
            WikiAction::Status => {
                commands::wiki::run_status()?;
            }
        },
    }

    Ok(())
}
