# searchgrep

**Semantic grep for the AI era** - natural language code search powered by Rust.

searchgrep brings grep into 2025 by combining traditional file search with AI-powered semantic understanding. Search your codebase using natural language queries like "where are authentication errors handled" or "database connection pooling logic".

## Features

- **Semantic Search**: Find code by meaning, not just keywords
- **Natural Language Queries**: Ask questions like you would ask a colleague
- **Local Embeddings**: Works offline with no API key required (BGE-base, CodeRankEmbed)
- **Hybrid Mode**: Combines multiple models for best quality
- **MCP Server**: Integrates directly with Claude Code
- **AI Answers**: Get synthesized answers about your codebase
- **File Watching**: Keep your index up-to-date automatically
- **Git-Aware**: Respects .gitignore patterns
- **Blazing Fast**: Native Rust with Apple Silicon acceleration

## Installation

### Homebrew (macOS)

```bash
brew tap RandomsUsernames/searchgrep
brew install searchgrep
```

### From Source (Recommended)

```bash
# Requires Rust: https://rustup.rs
git clone https://github.com/RandomsUsernames/Searchgrep.git
cd Searchgrep
git checkout rust-rewrite
cargo install --path .
```

### Quick Install Script

```bash
curl -fsSL https://raw.githubusercontent.com/RandomsUsernames/Searchgrep/rust-rewrite/install.sh | bash
```

## Quick Start

```bash
# Index your codebase
searchgrep watch --once

# Or watch for changes
searchgrep watch

# Search
searchgrep search "authentication middleware"
searchgrep search "where are errors handled" --content
searchgrep ask "how does the login flow work"
```

## Claude Code Integration (MCP)

searchgrep includes an MCP server for direct Claude Code integration.

### Automatic Setup

The installer configures Claude Code automatically. Or manually add to `~/.claude/mcp_servers.json`:

```json
{
  "mcpServers": {
    "searchgrep": {
      "command": "searchgrep",
      "args": ["mcp-server"],
      "env": {}
    }
  }
}
```

### MCP Tools

After restarting Claude Code, these tools are available:

| Tool | Description |
|------|-------------|
| `semantic_search` | Search code using natural language queries |
| `index_directory` | Index a directory for semantic search |

## Commands

### `searchgrep search <pattern> [path]`

Search files using natural language.

```bash
searchgrep search "database queries"
searchgrep search "API error handling" --content
searchgrep search "user authentication" --answer
searchgrep search "config loading" --sync
```

**Options:**
- `-m, --max-count <n>` - Maximum results (default: 10)
- `-c, --content` - Show file content snippets
- `-a, --answer` - Generate AI answer from results
- `-s, --sync` - Sync files before searching
- `--code` - Use CodeRankEmbed (optimized for code)
- `--hybrid` - Use BGE + CodeRankEmbed fusion (best quality)
- `--store <name>` - Use alternative store

### `searchgrep watch [path]`

Index files and watch for changes.

```bash
searchgrep watch           # Watch current directory
searchgrep watch ./src     # Watch specific path
searchgrep watch --once    # Index once, don't watch
searchgrep watch --fast    # Fast mode (MiniLM, 2-3x faster)
searchgrep watch --code    # Code-optimized (CodeRankEmbed)
```

### `searchgrep mcp-server`

Run as MCP server for Claude Code integration.

```bash
searchgrep mcp-server  # Runs JSON-RPC over stdio
```

### `searchgrep ask <question>`

Ask a question and get an AI-generated answer.

```bash
searchgrep ask "how does error handling work"
searchgrep ask "explain the authentication flow"
```

### `searchgrep config`

Configure settings.

```bash
searchgrep config --api-key sk-...   # Set OpenAI API key (for answers)
searchgrep config --show             # Show current config
searchgrep config --clear            # Clear indexed files
```

### `searchgrep status`

Show index status and statistics.

```bash
searchgrep status          # Show overview
searchgrep status --files  # List indexed files
```

## Search Modes

| Mode | Flag | Model | Best For |
|------|------|-------|----------|
| Balanced | (default) | BGE-base | General text search |
| Fast | `--fast` | MiniLM | Quick indexing |
| Code | `--code` | CodeRankEmbed | Code-specific search |
| Hybrid | `--hybrid` | BGE + CodeRankEmbed | Best quality |

## How It Works

1. **Indexing**: Files are chunked and converted to vector embeddings locally
2. **Storage**: Embeddings stored in `~/.searchgrep/`
3. **Search**: Query embedded and compared using cosine similarity
4. **Ranking**: Results ranked by semantic similarity
5. **Answers**: Top results sent to GPT for synthesized answers

## Performance

- **Apple Silicon**: Uses Accelerate framework for fast CPU inference
- **Hybrid Mode**: ~3.5s for search (loads both models)
- **Single Mode**: ~2s for search
- **Indexing**: ~100 files/minute (balanced mode)

## Configuration

### Environment Variables

- `OPENAI_API_KEY` - OpenAI API key (for `--answer` mode)
- `OPENAI_BASE_URL` - Custom API base URL

### Ignoring Files

searchgrep respects:
- `.gitignore` patterns
- `.searchgrepignore` (project-specific exclusions)

## Examples

```bash
# Find authentication-related code
searchgrep "user login and session handling"

# Search with content preview
searchgrep search "database connection" --content

# Code-optimized search
searchgrep search --code "async function error handling"

# Best quality hybrid search
searchgrep search --hybrid "vector embedding implementation"

# Get an answer about architecture
searchgrep ask "what's the overall architecture of this project"
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Install locally
cargo install --path .
```

## License

MIT
