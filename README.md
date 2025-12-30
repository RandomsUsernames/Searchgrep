# searchgrep

Semantic grep for the AI era - natural language code search.

searchgrep brings grep into 2025 by combining traditional file search with AI-powered semantic understanding. Search your codebase using natural language queries like "where are authentication errors handled" or "database connection pooling logic".

## Features

- **Semantic Search**: Find code by meaning, not just keywords
- **Natural Language Queries**: Ask questions like you would ask a colleague
- **Local Embeddings**: Works offline with no API key required
- **AI Answers**: Get synthesized answers about your codebase
- **File Watching**: Keep your index up-to-date automatically
- **Git-Aware**: Respects `.gitignore` patterns
- **Fast**: Uses efficient vector embeddings for instant search

## Installation

```bash
bun install -g searchgrep
```

## Quick Start

1. **Use local embeddings (no API key needed)**:
```bash
searchgrep config --provider local
```

Or use OpenAI embeddings:
```bash
searchgrep config --api-key sk-your-openai-key
```

2. **Index your codebase**:
```bash
searchgrep watch --once
# or watch for changes
searchgrep watch
```

3. **Search**:
```bash
searchgrep search "authentication middleware"
searchgrep search "where are errors handled" --content
searchgrep ask "how does the login flow work"
```

## Commands

### `searchgrep search <pattern> [path]`

Search files using natural language.

```bash
searchgrep search "database queries"
searchgrep search "API error handling" --content
searchgrep search "user authentication" --answer
searchgrep search "config loading" --sync  # sync before searching
```

Options:
- `-m, --max-count <n>` - Maximum results (default: 10)
- `-c, --content` - Show file content snippets
- `-a, --answer` - Generate AI answer from results
- `-s, --sync` - Sync files before searching
- `-d, --dry-run` - Preview sync without uploading
- `--store <name>` - Use alternative store

### `searchgrep watch [path]`

Index files and watch for changes.

```bash
searchgrep watch           # Watch current directory
searchgrep watch ./src     # Watch specific path
searchgrep watch --once    # Index once, don't watch
```

Options:
- `--once` - Index files once without watching
- `--store <name>` - Use alternative store

### `searchgrep ask <question>`

Ask a question about your codebase and get an AI-generated answer.

```bash
searchgrep ask "how does error handling work"
searchgrep ask "what database is used and how"
searchgrep ask "explain the authentication flow"
```

### `searchgrep config`

Configure searchgrep settings.

```bash
searchgrep config --provider local        # Use local embeddings (BGE-base)
searchgrep config --api-key sk-...        # Set OpenAI API key
searchgrep config --model text-embedding-3-large  # Change model
searchgrep config --show                  # Show current config
searchgrep config --clear                 # Clear indexed files
```

### `searchgrep status`

Show index status and statistics.

```bash
searchgrep status          # Show overview
searchgrep status --files  # List indexed files
```

## Embedding Providers

### Local (Recommended for getting started)

No API key required. Uses BGE-base-en-v1.5 model (~90MB, downloads on first use).

```bash
searchgrep config --provider local
```

### OpenAI

For higher quality embeddings with OpenAI's models.

```bash
searchgrep config --provider openai --api-key sk-...
```

Supported models:
- `text-embedding-3-small` (default, recommended)
- `text-embedding-3-large` (higher quality)
- `text-embedding-ada-002` (legacy)

## Configuration

searchgrep can be configured via:

1. **Environment variables**:
   - `OPENAI_API_KEY` - OpenAI API key
   - `OPENAI_BASE_URL` - Custom API base URL (for OpenAI-compatible APIs)

2. **Global config** (`~/.config/searchgrep/config.yaml`):
```yaml
embeddingProvider: local
openaiApiKey: sk-...
embeddingModel: text-embedding-3-small
```

3. **Local config** (`.searchgreprc.yaml` in project root):
```yaml
maxFileSize: 5242880
maxFileCount: 5000
```

## Ignoring Files

searchgrep respects:
- `.gitignore` patterns
- `.searchgrepignore` (project-specific exclusions)
- Built-in patterns for common non-text files

Create `.searchgrepignore` in your project root:
```
docs/generated/
*.generated.ts
test/fixtures/
```

## How It Works

1. **Indexing**: Files are chunked and converted to vector embeddings (locally or via OpenAI).
2. **Storage**: Embeddings are stored locally in `~/.searchgrep/`.
3. **Search**: Your query is embedded and compared using cosine similarity.
4. **Ranking**: Results are ranked by semantic similarity.
5. **Answers**: For `ask` commands, top results are sent to GPT to generate answers.

## Examples

```bash
# Find authentication-related code
searchgrep "user login and session handling"

# Search with content preview
searchgrep search "database connection" --content

# Get an answer about architecture
searchgrep ask "what's the overall architecture of this project"

# Search and sync first
searchgrep search "error handling" --sync
```

## License

MIT
