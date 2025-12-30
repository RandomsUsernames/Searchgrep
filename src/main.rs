use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod mcp;
pub mod ui;

use commands::{clean, compile, config, search, status, watch};

#[derive(Parser)]
#[command(name = "searchgrep")]
#[command(about = "Semantic grep for the AI era - natural language code search")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search pattern (if no command specified)
    #[arg(value_name = "PATTERN")]
    pattern: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search files using natural language
    #[command(alias = "s")]
    Search {
        /// Natural language search query
        pattern: String,

        /// Path to search in (defaults to current directory)
        path: Option<String>,

        /// Maximum number of results
        #[arg(short = 'm', long, default_value = "10")]
        max_count: usize,

        /// Show file content snippets
        #[arg(short = 'c', long)]
        content: bool,

        /// Generate AI answer from search results
        #[arg(short = 'a', long)]
        answer: bool,

        /// Sync files before searching
        #[arg(short = 's', long)]
        sync: bool,

        /// Disable result reranking
        #[arg(long)]
        no_rerank: bool,

        /// Use ColBERT token-level matching (local only)
        #[arg(long)]
        colbert: bool,

        /// Filter by file type (e.g., ts, py, js)
        #[arg(short = 't', long = "type", value_name = "EXT")]
        file_types: Vec<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Code mode - use CodeRankEmbed optimized for code search
        #[arg(long)]
        code: bool,

        /// Hybrid mode - use BGE + CodeRankEmbed fusion for best quality
        #[arg(long)]
        hybrid: bool,
    },

    /// Index files and watch for changes
    #[command(alias = "w")]
    Watch {
        /// Path to watch (defaults to current directory)
        path: Option<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Index files once without watching
        #[arg(long)]
        once: bool,

        /// Fast mode - use smaller model (MiniLM) for 2-3x faster indexing
        #[arg(long)]
        fast: bool,

        /// Quality mode - use F32 precision for highest accuracy
        #[arg(long)]
        quality: bool,

        /// Code mode - use CodeRankEmbed optimized for code search
        #[arg(long)]
        code: bool,
    },

    /// Configure searchgrep settings
    #[command(alias = "c")]
    Config {
        /// Set OpenAI API key
        #[arg(long)]
        api_key: Option<String>,

        /// Set embedding model
        #[arg(long)]
        model: Option<String>,

        /// Set custom API base URL
        #[arg(long)]
        base_url: Option<String>,

        /// Set embedding provider (openai, local, or c2llm)
        #[arg(long)]
        provider: Option<String>,

        /// Set local embedding server URL
        #[arg(long)]
        local_url: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Clear all indexed files
        #[arg(long)]
        clear: bool,

        /// Download C2LLM-0.5B model for local embeddings
        #[arg(long)]
        download_model: bool,
    },

    /// Show index status and statistics
    #[command(alias = "st")]
    Status {
        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// List indexed files
        #[arg(long)]
        files: bool,
    },

    /// Run as MCP server for Claude Code integration
    #[command(name = "mcp-server")]
    McpServer,

    /// Remove stored indexes for privacy/storage management
    Clean {
        /// List all indexes and their sizes
        #[arg(short, long)]
        list: bool,

        /// Remove ALL indexes
        #[arg(short, long)]
        all: bool,

        /// Remove a specific index by name
        #[arg(short, long)]
        store: Option<String>,
    },

    /// Compile codebase map for LLM consumption (90% token reduction)
    Compile {
        /// Path to compile (defaults to current directory)
        path: Option<String>,

        /// Show the compiled map
        #[arg(short, long)]
        show: bool,

        /// Show minimal overview (just function names)
        #[arg(short, long)]
        minimal: bool,
    },

    /// Ask a question about your codebase
    #[command(alias = "a")]
    Ask {
        /// Question to ask about the code
        question: String,

        /// Path to search in
        path: Option<String>,

        /// Number of context files to use
        #[arg(short = 'm', long, default_value = "5")]
        max_count: usize,

        /// Sync files before asking
        #[arg(short = 's', long)]
        sync: bool,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Search {
            pattern,
            path,
            max_count,
            content,
            answer,
            sync,
            no_rerank,
            colbert,
            file_types,
            store,
            code,
            hybrid,
        }) => {
            search::run(search::SearchOptions {
                pattern,
                path,
                max_count,
                content,
                answer,
                sync,
                rerank: !no_rerank,
                colbert,
                file_types: if file_types.is_empty() {
                    None
                } else {
                    Some(file_types)
                },
                store,
                code,
                hybrid,
            })
            .await?;
        }
        Some(Commands::Watch {
            path,
            store,
            once,
            fast,
            quality,
            code,
        }) => {
            watch::run(watch::WatchOptions {
                path,
                store,
                once,
                fast,
                quality,
                code,
            })
            .await?;
        }
        Some(Commands::Config {
            api_key,
            model,
            base_url,
            provider,
            local_url,
            show,
            clear,
            download_model,
        }) => {
            config::run(config::ConfigOptions {
                api_key,
                model,
                base_url,
                provider,
                local_url,
                show,
                clear,
                download_model,
            })
            .await?;
        }
        Some(Commands::Status { store, files }) => {
            status::run(status::StatusOptions { store, files }).await?;
        }
        Some(Commands::McpServer) => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Some(Commands::Clean { list, all, store }) => {
            clean::run(clean::CleanOptions { list, all, store }).await?;
        }
        Some(Commands::Compile {
            path,
            show,
            minimal,
        }) => {
            compile::run(compile::CompileOptions {
                path,
                show,
                minimal,
            })
            .await?;
        }
        Some(Commands::Ask {
            question,
            path,
            max_count,
            sync,
            store,
        }) => {
            search::run(search::SearchOptions {
                pattern: question,
                path,
                max_count,
                content: false,
                answer: true,
                sync,
                rerank: true,
                colbert: false,
                file_types: None,
                store,
                code: false,
                hybrid: false,
            })
            .await?;
        }
        None => {
            if let Some(pattern) = cli.pattern {
                search::run(search::SearchOptions {
                    pattern,
                    path: None,
                    max_count: 10,
                    content: false,
                    answer: false,
                    sync: false,
                    rerank: true,
                    colbert: false,
                    file_types: None,
                    store: None,
                    code: false,
                    hybrid: false,
                })
                .await?;
            } else {
                // Show help
                use clap::CommandFactory;
                Cli::command().print_help()?;
            }
        }
    }

    Ok(())
}
