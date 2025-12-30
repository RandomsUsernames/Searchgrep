class Searchgrep < Formula
  desc "Semantic grep for the AI era - natural language code search with MCP server"
  homepage "https://github.com/RandomsUsernames/Searchgrep"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v0.1.0/searchgrep-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64"
    else
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v0.1.0/searchgrep-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X64"
    end
  end

  on_linux do
    url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v0.1.0/searchgrep-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "PLACEHOLDER_SHA256_LINUX"
  end

  def install
    bin.install "searchgrep"
  end

  def caveats
    <<~EOS
      searchgrep has been installed!

      To use with Claude Code, add to ~/.claude/mcp_servers.json:
        {
          "mcpServers": {
            "searchgrep": {
              "command": "#{HOMEBREW_PREFIX}/bin/searchgrep",
              "args": ["mcp-server"],
              "env": {}
            }
          }
        }

      Quick start:
        searchgrep watch .          # Index current directory
        searchgrep search "query"   # Semantic search
        searchgrep --help           # See all options
    EOS
  end

  test do
    assert_match "searchgrep", shell_output("#{bin}/searchgrep --version")
  end
end
