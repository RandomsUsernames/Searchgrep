class Searchgrep < Formula
  desc "Semantic grep for the AI era - natural language code search with MCP server"
  homepage "https://github.com/RandomsUsernames/Searchgrep"
  version "2.0.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/RandomsUsernames/Searchgrep/releases/download/v2.0.0/searchgrep-aarch64-apple-darwin.tar.gz"
      sha256 "c2c43fc569186d5fca641cdb1cd87e371b95f3c8fe2986025936764ddbfa2ec0"
    end
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
