use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};

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

        /// Output results as JSON
        #[arg(long)]
        json: bool,
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

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

        /// Output as JSON
        #[arg(long)]
        json: bool,
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

    /// Update searchgrep to the latest version from GitHub
    Update {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,
    },

    /// Remove searchgrep MCP configuration from AI tools
    Remove {
        /// Tool to remove from: claude, opencode, cursor, windsurf, or all
        #[arg(default_value = "interactive")]
        tool: String,
    },

    /// Initialize searchgrep in current directory (index + compile)
    Init {
        /// Fast mode - skip embeddings, BM25 only
        #[arg(long)]
        fast: bool,

        /// Also setup MCP for Claude Code
        #[arg(long)]
        mcp: bool,
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
            json,
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
                json,
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
        Some(Commands::Status { store, files, json }) => {
            status::run(status::StatusOptions { store, files, json }).await?;
        }
        Some(Commands::McpServer) => {
            let mut server = mcp::McpServer::new();
            server.run()?;
        }
        Some(Commands::Clean {
            list,
            all,
            store,
            json,
        }) => {
            clean::run(clean::CleanOptions {
                list,
                all,
                store,
                json,
            })
            .await?;
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
            json,
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
                json,
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
                    println!("Select AI coding tool to configure:");
                    println!();
                    println!("  {}  Claude Code", "1.".bold());
                    println!("  {}  OpenCode", "2.".bold());
                    println!("  {}  Cursor", "3.".bold());
                    println!("  {}  Windsurf", "4.".bold());
                    println!("  {}  Codex (OpenAI)", "5.".bold());
                    println!("  {}  Gemini CLI", "6.".bold());
                    println!("  {}  Cody (Sourcegraph)", "7.".bold());
                    println!("  {}  Continue", "8.".bold());
                    println!("  {}  Aider", "9.".bold());
                    println!("  {} Zed", "10.".bold());
                    println!("  {} ACP (Agent Control Protocol)", "11.".bold());
                    println!("  {} Droid", "12.".bold());
                    println!("  {} Amp", "13.".bold());
                    println!("  {} Roo Code", "14.".bold());
                    println!("  {} Cline", "15.".bold());
                    println!();
                    print!("Enter choice (1-15): ");
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
                        "7" => vec!["cody"],
                        "8" => vec!["continue"],
                        "9" => vec!["aider"],
                        "10" => vec!["zed"],
                        "11" => vec!["acp"],
                        "12" => vec!["droid"],
                        "13" => vec!["amp"],
                        "14" => vec!["roo"],
                        "15" => vec!["cline"],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "claude" => vec!["claude"],
                "opencode" => vec!["opencode"],
                "cursor" => vec!["cursor"],
                "windsurf" => vec!["windsurf"],
                "codex" => vec!["codex"],
                "gemini" => vec!["gemini"],
                "cody" => vec!["cody"],
                "continue" => vec!["continue"],
                "aider" => vec!["aider"],
                "zed" => vec!["zed"],
                "acp" => vec!["acp"],
                "droid" => vec!["droid"],
                "amp" => vec!["amp"],
                "roo" => vec!["roo"],
                "cline" => vec!["cline"],
                _ => {
                    println!("{} Unknown tool: {}", "✗".red(), tool);
                    println!("Available: claude, opencode, cursor, windsurf, codex, gemini, cody, continue, aider, zed, acp, droid, amp, roo, cline");
                    return Ok(());
                }
            };

            for tool_name in tools {
                // Each tool has its own config path and format
                let (config_path, config_type) = match tool_name {
                    "claude" => (home.join(".claude/mcp_servers.json"), "mcpServers"),
                    "cursor" => (home.join(".cursor/mcp.json"), "mcpServers"),
                    "windsurf" => (home.join(".codeium/windsurf/mcp_config.json"), "mcpServers"),
                    "codex" => (home.join(".codex/mcp.json"), "mcpServers"),
                    "opencode" => (home.join(".opencode/mcp.json"), "mcpServers"),
                    "zed" => (home.join(".config/zed/settings.json"), "context_servers"),
                    "continue" => (home.join(".continue/config.json"), "experimental.modelContextProtocolServers"),
                    "cline" => (home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"), "mcpServers"),
                    "roo" => (home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json"), "mcpServers"),
                    "amp" => (home.join(".amp/amp.yaml"), "mcpServers"),
                    "gemini" => (home.join(".gemini/settings.json"), "mcpServers"),
                    "cody" => (home.join("Library/Application Support/Code/User/globalStorage/sourcegraph.cody-ai/cody_mcp_settings.json"), "mcpServers"),
                    "aider" => (home.join(".aider/mcp.json"), "mcpServers"),
                    "droid" => (home.join(".droid/mcp.json"), "mcpServers"),
                    "acp" => (home.join(".acp/mcp.json"), "mcpServers"),
                    _ => continue,
                };

                // Create directory if needed
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                // Zed uses a different config structure
                if config_type == "context_servers" {
                    let zed_config = serde_json::json!({
                        "command": searchgrep_path,
                        "args": ["mcp-server"]
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("context_servers").is_none() {
                        config["context_servers"] = serde_json::json!({});
                    }
                    config["context_servers"]["searchgrep"] = zed_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else if config_type == "experimental.modelContextProtocolServers" {
                    // Continue uses experimental.modelContextProtocolServers array
                    let continue_config = serde_json::json!({
                        "name": "searchgrep",
                        "command": searchgrep_path,
                        "args": ["mcp-server"]
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("experimental").is_none() {
                        config["experimental"] = serde_json::json!({});
                    }
                    if config["experimental"]
                        .get("modelContextProtocolServers")
                        .is_none()
                    {
                        config["experimental"]["modelContextProtocolServers"] =
                            serde_json::json!([]);
                    }

                    // Remove existing searchgrep entry if present
                    if let Some(arr) =
                        config["experimental"]["modelContextProtocolServers"].as_array_mut()
                    {
                        arr.retain(|v| {
                            v.get("name").and_then(|n| n.as_str()) != Some("searchgrep")
                        });
                        arr.push(continue_config);
                    }

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                } else {
                    // Standard mcpServers format (Claude, Cursor, Codex, Cline, etc.)
                    let mcp_config = serde_json::json!({
                        "command": searchgrep_path,
                        "args": ["mcp-server"],
                        "env": {}
                    });

                    let mut config: serde_json::Value = if config_path.exists() {
                        let content = std::fs::read_to_string(&config_path)?;
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if config.get("mcpServers").is_none() {
                        config["mcpServers"] = serde_json::json!({});
                    }
                    config["mcpServers"]["searchgrep"] = mcp_config;

                    let content = serde_json::to_string_pretty(&config)?;
                    std::fs::write(&config_path, content)?;
                }

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
                json: false,
            })
            .await?;
        }
        Some(Commands::Update { check }) => {
            use colored::Colorize;

            let current_version = env!("CARGO_PKG_VERSION");
            println!(
                "{} Checking for updates (current: v{})...",
                "→".cyan(),
                current_version
            );

            // Fetch latest release from GitHub
            let client = reqwest::Client::new();
            let resp = client
                .get("https://api.github.com/repos/RandomsUsernames/Searchgrep/releases/latest")
                .header("User-Agent", "searchgrep")
                .send()
                .await?;

            if !resp.status().is_success() {
                println!("{} Failed to check for updates", "✗".red());
                return Ok(());
            }

            let release: serde_json::Value = resp.json().await?;
            let latest_version = release["tag_name"]
                .as_str()
                .unwrap_or("unknown")
                .trim_start_matches('v');

            if latest_version == current_version {
                println!("{} Already up to date (v{})", "✓".green(), current_version);
                return Ok(());
            }

            println!(
                "{} New version available: v{} → v{}",
                "!".yellow(),
                current_version,
                latest_version
            );

            if check {
                println!("\nRun 'searchgrep update' to install the latest version.");
                return Ok(());
            }

            // Download and install
            println!("{} Downloading latest release...", "→".cyan());

            #[cfg(target_os = "macos")]
            let asset_name = if cfg!(target_arch = "aarch64") {
                "searchgrep-aarch64-apple-darwin.tar.gz"
            } else {
                "searchgrep-x86_64-apple-darwin.tar.gz"
            };

            #[cfg(target_os = "linux")]
            let asset_name = "searchgrep-x86_64-unknown-linux-gnu.tar.gz";

            #[cfg(target_os = "windows")]
            let asset_name = "searchgrep-x86_64-pc-windows-msvc.zip";

            let download_url = format!(
                "https://github.com/RandomsUsernames/Searchgrep/releases/download/v{}/{}",
                latest_version, asset_name
            );

            let resp = client.get(&download_url).send().await?;
            if !resp.status().is_success() {
                println!("{} Failed to download release", "✗".red());
                println!("Try manually: {}", download_url);
                return Ok(());
            }

            let bytes = resp.bytes().await?;

            // Extract and install
            let temp_dir = std::env::temp_dir().join("searchgrep-update");
            std::fs::create_dir_all(&temp_dir)?;

            let archive_path = temp_dir.join(asset_name);
            std::fs::write(&archive_path, &bytes)?;

            println!("{} Installing...", "→".cyan());

            // Extract using tar
            let status = std::process::Command::new("tar")
                .args([
                    "-xzf",
                    &archive_path.to_string_lossy(),
                    "-C",
                    &temp_dir.to_string_lossy(),
                ])
                .status()?;

            if !status.success() {
                println!("{} Failed to extract archive", "✗".red());
                return Ok(());
            }

            // Copy binary to cargo bin
            let cargo_bin = dirs::home_dir()
                .unwrap_or_default()
                .join(".cargo/bin/searchgrep");

            let new_binary = temp_dir.join("searchgrep");
            if new_binary.exists() {
                std::fs::copy(&new_binary, &cargo_bin)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&cargo_bin, std::fs::Permissions::from_mode(0o755))?;
                }
            }

            // Cleanup
            let _ = std::fs::remove_dir_all(&temp_dir);

            println!("{} Updated to v{}!", "✓".green(), latest_version);
        }
        Some(Commands::Remove { tool }) => {
            use colored::Colorize;

            let home =
                dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;

            let tools: Vec<&str> = match tool.as_str() {
                "interactive" => {
                    println!("{}", "searchgrep MCP Removal".cyan().bold());
                    println!();
                    println!("Select AI tool to remove searchgrep from:");
                    println!();
                    println!("  {}  Claude Code", "1.".bold());
                    println!("  {}  OpenCode", "2.".bold());
                    println!("  {}  Cursor", "3.".bold());
                    println!("  {}  Windsurf", "4.".bold());
                    println!("  {}  All configured tools", "5.".bold());
                    println!();
                    print!("Enter choice (1-5): ");
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    match input.trim() {
                        "1" => vec!["claude"],
                        "2" => vec!["opencode"],
                        "3" => vec!["cursor"],
                        "4" => vec!["windsurf"],
                        "5" => vec![
                            "claude", "opencode", "cursor", "windsurf", "codex", "gemini", "cody",
                            "continue", "aider", "zed", "acp", "droid", "amp", "roo", "cline",
                        ],
                        _ => {
                            println!("{} Invalid choice", "✗".red());
                            return Ok(());
                        }
                    }
                }
                "all" => vec![
                    "claude", "opencode", "cursor", "windsurf", "codex", "gemini", "cody",
                    "continue", "aider", "zed", "acp", "droid", "amp", "roo", "cline",
                ],
                other => vec![other],
            };

            for tool_name in tools {
                let (config_path, is_claude) = match tool_name {
                    "claude" => (home.join(".claude.json"), true),
                    "opencode" => (home.join(".opencode/mcp.json"), false),
                    "cursor" => (home.join(".cursor/mcp.json"), false),
                    "windsurf" => (home.join(".windsurf/mcp.json"), false),
                    "codex" => (home.join(".codex/mcp.json"), false),
                    "gemini" => (home.join(".gemini/mcp.json"), false),
                    "cody" => (home.join(".cody/mcp.json"), false),
                    "continue" => (home.join(".continue/mcp.json"), false),
                    "aider" => (home.join(".aider/mcp.json"), false),
                    "zed" => (home.join(".zed/mcp.json"), false),
                    "acp" => (home.join(".acp/mcp.json"), false),
                    "droid" => (home.join(".droid/mcp.json"), false),
                    "amp" => (home.join(".amp/mcp.json"), false),
                    "roo" => (home.join(".roo-code/mcp.json"), false),
                    "cline" => (home.join(".cline/mcp.json"), false),
                    _ => continue,
                };

                if !config_path.exists() {
                    continue;
                }

                let content = std::fs::read_to_string(&config_path)?;
                let mut config: serde_json::Value =
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

                let removed = if is_claude {
                    let home_str = home.to_string_lossy().to_string();
                    if let Some(servers) =
                        config["projects"][&home_str]["mcpServers"].as_object_mut()
                    {
                        servers.remove("searchgrep").is_some()
                    } else {
                        false
                    }
                } else {
                    if let Some(servers) = config["mcpServers"].as_object_mut() {
                        servers.remove("searchgrep").is_some()
                    } else {
                        false
                    }
                };

                if removed {
                    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                    println!("{} Removed searchgrep from {}", "✓".green(), tool_name);
                }
            }
        }
        Some(Commands::Init { fast, mcp }) => {
            use colored::Colorize;

            let current_dir = std::env::current_dir()?;
            println!(
                "{} Initializing searchgrep in {}",
                "→".cyan(),
                current_dir.display()
            );

            // Index the directory
            println!("{} Indexing files...", "→".cyan());

            let speed_mode = if fast {
                crate::core::local_embeddings::SpeedMode::Fast
            } else {
                crate::core::local_embeddings::SpeedMode::Balanced
            };

            crate::commands::watch::sync_files(
                current_dir.to_str().unwrap_or("."),
                None,
                speed_mode,
            )
            .await?;

            println!("{} Indexed successfully", "✓".green());

            // Compile codebase map
            println!("{} Compiling codebase map...", "→".cyan());
            match compile::run(compile::CompileOptions {
                path: Some(current_dir.to_string_lossy().to_string()),
                show: false,
                minimal: false,
            })
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    println!("{} Failed to compile: {}", "✗".yellow(), e);
                }
            }

            // Setup MCP if requested
            if mcp {
                println!("{} Setting up Claude Code MCP...", "→".cyan());
                let home = dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
                let searchgrep_path = which::which("searchgrep")
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "searchgrep".to_string());

                let config_path = home.join(".claude.json");
                let mut config: serde_json::Value = if config_path.exists() {
                    let content = std::fs::read_to_string(&config_path)?;
                    serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                let home_str = home.to_string_lossy().to_string();
                if config.get("projects").is_none() {
                    config["projects"] = serde_json::json!({});
                }
                if config["projects"].get(&home_str).is_none() {
                    config["projects"][&home_str] = serde_json::json!({});
                }
                if config["projects"][&home_str].get("mcpServers").is_none() {
                    config["projects"][&home_str]["mcpServers"] = serde_json::json!({});
                }
                config["projects"][&home_str]["mcpServers"]["searchgrep"] = serde_json::json!({
                    "command": searchgrep_path,
                    "args": ["mcp-server"],
                    "env": {}
                });

                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                println!("{} Claude Code MCP configured", "✓".green());
            }

            println!();
            println!("{}", "Ready! You can now:".green().bold());
            println!("  • Search: searchgrep search \"your query\"");
            println!("  • Ask: searchgrep ask \"how does X work?\"");
            if !mcp {
                println!("  • Setup MCP: searchgrep mcp");
            }
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
                    json: false,
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
