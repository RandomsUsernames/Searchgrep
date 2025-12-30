#!/bin/bash
# searchgrep installer
# Installs searchgrep and configures it for Claude Code

set -e

REPO="RandomsUsernames/Searchgrep"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

echo "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓"
echo "┃  searchgrep installer                  ┃"
echo "┃  Semantic grep for the AI era          ┃"
echo "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛"
echo

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin)
    case "$ARCH" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  linux)
    case "$ARCH" in
      x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
      *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

echo "Detected: $OS $ARCH"
echo "Target: $TARGET"
echo

# Check if cargo is available for building from source
if command -v cargo &> /dev/null; then
  echo "Rust found! Building from source for best performance..."
  echo

  # Clone and build
  TMP_DIR=$(mktemp -d)
  cd "$TMP_DIR"

  git clone --depth 1 "https://github.com/$REPO.git" searchgrep-rs
  cd searchgrep-rs

  cargo build --release

  # Install
  if [ -w "$INSTALL_DIR" ]; then
    cp target/release/searchgrep "$INSTALL_DIR/"
  else
    sudo cp target/release/searchgrep "$INSTALL_DIR/"
  fi

  # Cleanup
  cd /
  rm -rf "$TMP_DIR"
else
  echo "Rust not found. Downloading pre-built binary..."
  echo "(Install Rust for optimized native builds: https://rustup.rs)"
  echo

  # Get latest release
  LATEST=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)

  if [ -z "$LATEST" ]; then
    echo "Could not determine latest release. Building from source..."
    echo "Please install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
  fi

  URL="https://github.com/$REPO/releases/download/$LATEST/searchgrep-$TARGET.tar.gz"

  TMP_DIR=$(mktemp -d)
  cd "$TMP_DIR"

  curl -sL "$URL" | tar xz

  if [ -w "$INSTALL_DIR" ]; then
    mv searchgrep "$INSTALL_DIR/"
  else
    sudo mv searchgrep "$INSTALL_DIR/"
  fi

  cd /
  rm -rf "$TMP_DIR"
fi

echo
echo "✓ searchgrep installed to $INSTALL_DIR/searchgrep"
echo

# Configure Claude Code MCP
CLAUDE_CONFIG="$HOME/.claude/mcp_servers.json"

configure_claude() {
  mkdir -p "$HOME/.claude"

  if [ -f "$CLAUDE_CONFIG" ]; then
    # Check if searchgrep already configured
    if grep -q '"searchgrep"' "$CLAUDE_CONFIG"; then
      echo "✓ searchgrep already configured in Claude Code"
      return
    fi

    # Add to existing config
    echo "Adding searchgrep to Claude Code MCP servers..."
    # Use jq if available, otherwise provide manual instructions
    if command -v jq &> /dev/null; then
      jq '.mcpServers.searchgrep = {"command": "searchgrep", "args": ["mcp-server"], "env": {}}' \
        "$CLAUDE_CONFIG" > "$CLAUDE_CONFIG.tmp" && mv "$CLAUDE_CONFIG.tmp" "$CLAUDE_CONFIG"
      echo "✓ Added searchgrep to Claude Code"
    else
      echo "Please add to $CLAUDE_CONFIG:"
      echo '  "searchgrep": { "command": "searchgrep", "args": ["mcp-server"], "env": {} }'
    fi
  else
    # Create new config
    cat > "$CLAUDE_CONFIG" << 'EOF'
{
  "mcpServers": {
    "searchgrep": {
      "command": "searchgrep",
      "args": ["mcp-server"],
      "env": {}
    }
  }
}
EOF
    echo "✓ Created Claude Code MCP config"
  fi
}

read -p "Configure for Claude Code? [Y/n] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Nn]$ ]]; then
  configure_claude
fi

echo
echo "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓"
echo "┃  Installation complete!                ┃"
echo "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛"
echo
echo "Quick start:"
echo "  searchgrep watch .          # Index current directory"
echo "  searchgrep search \"query\"   # Semantic search"
echo "  searchgrep --help           # See all options"
echo
echo "Restart Claude Code to use the MCP tools."
