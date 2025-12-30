use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod core;
mod mcp;
pub mod ui;

use commands::{clean, compile, config, index, search, status, watch};

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

    /// Fast parallel indexing with multiple optimization strategies
    #[command(alias = "i")]
    Index {
        /// Path to index (defaults to current directory)
        path: Option<String>,

        /// Use alternative store name
        #[arg(long)]
        store: Option<String>,

        /// Fast mode - BM25 only, instant indexing (no embeddings)
        #[arg(long)]
        fast: bool,

        /// Balanced mode - BM25 + embeddings (default)
        #[arg(long)]
        balanced: bool,

        /// Quality mode - best embeddings, slower
        #[arg(long)]
        quality: bool,

        /// Force re-index all files (ignore cache)
        #[arg(short, long)]
        force: bool,

        /// Number of threads (0 = auto-detect)
        #[arg(short, long, default_value = "0")]
        threads: usize,

        /// Batch size for embedding requests
        #[arg(short, long, default_value = "50")]
        batch_size: usize,
    },

    /// Build and install searchgrep to ~/.cargo/bin
    Install,

    /// Setup MCP integration for AI coding tools
    #[command(alias = "mcp")]
    Setup {
        /// Tool to configure: claude, opencode, cursor, windsurf, or all
        #[arg(default_value = "interactive")]
        tool: String,
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
        Some(Commands::Index {
            path,
            store,
            fast,
            balanced,
            quality,
            force,
            threads,
            batch_size,
        }) => {
            index::run(index::IndexOptions {
                path,
                store,
                fast,
                balanced,
                quality,
                force,
                threads,
                batch_size,
            })
            .await?;
        }
        Some(Commands::Install) => {
            use anyhow::Context;
            use colored::Colorize;
            use std::process::Command;

            // Known source location
            let home = dirs::home_dir().context("Could not find home directory")?;
            let known_path = home.join("extras/stuff/searchgrep-rs");

            // Try to find project directory
            let project_dir = if known_path.join("Cargo.toml").exists() {
                known_path
            } else {
                // Try current directory
                let cwd = std::env::current_dir()?;
                if cwd.join("Cargo.toml").exists() {
                    cwd
                } else {
                    // Walk up from current dir
                    let mut dir = cwd.clone();
                    let mut found = None;
                    for _ in 0..5 {
                        if dir.join("Cargo.toml").exists() {
                            found = Some(dir.clone());
                            break;
                        }
                        if let Some(parent) = dir.parent() {
                            dir = parent.to_path_buf();
                        } else {
                            break;
                        }
                    }
                    found.ok_or_else(|| anyhow::anyhow!(
                        "Could not find searchgrep source. Expected at: {}\nOr run from the searchgrep-rs directory.",
                        known_path.display()
                    ))?
                }
            };

            println!(
                "{} Building release from {}...",
                "⚙".cyan(),
                project_dir.display()
            );

            let status = Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&project_dir)
                .status()?;

            if !status.success() {
                anyhow::bail!("Build failed");
            }

            let target_binary = project_dir.join("target/release/searchgrep");
            let home = dirs::home_dir().context("Could not find home directory")?;
            let install_path = home.join(".cargo/bin/searchgrep");

            std::fs::copy(&target_binary, &install_path)?;

            println!(
                "{} Installed to {}",
                "✓".green().bold(),
                install_path.display()
            );
            println!("   Run {} to verify", "searchgrep --version".cyan());
        }
        Some(Commands::Setup { tool }) => {
            use anyhow::Context;
            use colored::Colorize;
            use std::io::{self, Write};

            let home = dirs::home_dir().context("Could not find home directory")?;

            // Find searchgrep binary path
            let searchgrep_path = which::which("searchgrep")
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "searchgrep".to_string());

            let tools: Vec<&str> = match tool.as_str() {
                "interactive" => {
                    println!("{}", "searchgrep MCP Setup".cyan().bold());
                    println!();
                    println!("Select AI tool to configure:");
                    println!("  {} Claude Code", "1.".bold());
                    println!("  {} OpenCode", "2.".bold());
                    println!("  {} Cursor", "3.".bold());
                    println!("  {} Windsurf", "4.".bold());
                    println!("  {} Codex (OpenAI)", "5.".bold());
                    println!("  {} Gemini CLI", "6.".bold());
                    println!();
                    print!("Enter choice (1-6): ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    match input.trim() {
                        "1" => vec!["claude"],
                        "2" => vec!["opencode"],
                        "3" => vec!["cursor"],
                        "4" => vec!["windsurf"],
                        "5" => vec!["codex"],
                        "6" => vec!["gemini"],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "all" => vec![
                    "claude", "opencode", "cursor", "windsurf", "codex", "gemini",
                ],
                "claude" => vec!["claude"],
                "opencode" => vec!["opencode"],
                "cursor" => vec!["cursor"],
                "windsurf" => vec!["windsurf"],
                "codex" => vec!["codex"],
                "gemini" => vec!["gemini"],
                _ => {
                    println!("{} Unknown tool: {}", "✗".red(), tool);
                    println!("Available: claude, opencode, cursor, windsurf, codex, gemini, all");
                    return Ok(());
                }
            };

            let mcp_config = serde_json::json!({
                "command": searchgrep_path,
                "args": ["mcp-server"],
                "env": {}
            });

            for tool_name in tools {
                let config_path = match tool_name {
                    "claude" => home.join(".claude/mcp_servers.json"),
                    "opencode" => home.join(".opencode/mcp.json"),
                    "cursor" => home.join(".cursor/mcp.json"),
                    "windsurf" => home.join(".windsurf/mcp.json"),
                    "codex" => home.join(".codex/mcp.json"),
                    "gemini" => home.join(".gemini/mcp.json"),
                    _ => continue,
                };

                // Create directory if needed
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // Load or create config
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // Add searchgrep to mcpServers
                if config.get("mcpServers").is_none() {
                    config["mcpServers"] = serde_json::json!({});
                }
                config["mcpServers"]["searchgrep"] = mcp_config.clone();

                // Write config
                let content = serde_json::to_string_pretty(&config)?;
                std::fs::write(&config_path, content)?;

                println!(
                    "{} Configured {} at {}",
                    "✓".green().bold(),
                    tool_name.cyan(),
                    config_path.display()
                );
            }

            println!();
            println!("{}", "Restart your AI tool to use searchgrep.".yellow());
            println!(
                "Available MCP tools: {}, {}",
                "semantic_search".cyan(),
                "index_directory".cyan()
            );
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
