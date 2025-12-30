#!/usr/bin/env node

import { Command } from "commander";
import chalk from "chalk";
import { searchCommand } from "./commands/search.js";
import { watchCommand } from "./commands/watch.js";
import { configCommand } from "./commands/config.js";
import { statusCommand } from "./commands/status.js";
import { indexCommand } from "./commands/index.js";
import { claudeCommand } from "./commands/claude.js";

const program = new Command();

program
  .name("searchgrep")
  .description("Semantic grep for the AI era - natural language code search")
  .version("1.0.0");

program
  .command("search")
  .alias("s")
  .description("Search files using natural language")
  .argument("<pattern>", "Natural language search query")
  .argument("[path]", "Path to search in (defaults to current directory)")
  .option("-m, --max-count <n>", "Maximum number of results", "10")
  .option("-c, --content", "Show file content snippets", false)
  .option("-a, --answer", "Generate AI answer from search results", false)
  .option("-s, --sync", "Sync files before searching", false)
  .option("-d, --dry-run", "Preview sync without uploading", false)
  .option("--no-rerank", "Disable result reranking")
  .option("-t, --type <ext...>", "Filter by file type (e.g., ts, py, js)")
  .option("--store <name>", "Use alternative store name")
  .action(async (pattern, path, options) => {
    await searchCommand(pattern, path, {
      maxCount: parseInt(options.maxCount, 10),
      content: options.content,
      answer: options.answer,
      sync: options.sync,
      dryRun: options.dryRun,
      rerank: options.rerank !== false,
      fileTypes: options.type,
      store: options.store,
    });
  });

program
  .command("watch")
  .alias("w")
  .description("Index files and watch for changes")
  .argument("[path]", "Path to watch (defaults to current directory)")
  .option("--store <name>", "Use alternative store name")
  .option("--once", "Index files once without watching", false)
  .action(async (path, options) => {
    await watchCommand(path, {
      store: options.store,
      once: options.once,
    });
  });

program
  .command("config")
  .alias("c")
  .description("Configure searchgrep settings")
  .option("--api-key <key>", "Set OpenAI API key")
  .option("--model <model>", "Set embedding model")
  .option("--base-url <url>", "Set custom API base URL")
  .option("--provider <type>", "Set embedding provider (openai or local)")
  .option("--local-url <url>", "Set local embedding server URL")
  .option("--show", "Show current configuration", false)
  .option("--clear", "Clear all indexed files", false)
  .action(async (options) => {
    await configCommand({
      ...options,
      localUrl: options.localUrl,
    });
  });

program
  .command("status")
  .alias("st")
  .description("Show index status and statistics")
  .option("--store <name>", "Use alternative store name")
  .option("--files", "List indexed files", false)
  .action(async (options) => {
    await statusCommand(options);
  });

program
  .command("index")
  .alias("i")
  .description("Index your codebase for semantic search")
  .argument("[path]", "Path to index (defaults to current directory)")
  .option("--store <name>", "Use alternative store name")
  .option("-f, --force", "Force reindex all files", false)
  .option("-v, --verbose", "Show detailed output", false)
  .option("--fast", "Fast mode - smaller model, 2-3x faster indexing", false)
  .option("--quality", "Quality mode - higher accuracy, slower indexing", false)
  .action(async (path, options) => {
    await indexCommand(path, options);
  });

program
  .command("claude")
  .description("Register searchgrep MCP server with Claude")
  .action(async () => {
    await claudeCommand();
  });

program
  .command("ask")
  .alias("a")
  .description("Ask a question about your codebase")
  .argument("<question>", "Question to ask about the code")
  .argument("[path]", "Path to search in")
  .option("-m, --max-count <n>", "Number of context files to use", "5")
  .option("-s, --sync", "Sync files before asking", false)
  .option("--store <name>", "Use alternative store name")
  .action(async (question, path, options) => {
    await searchCommand(question, path, {
      maxCount: parseInt(options.maxCount, 10),
      content: false,
      answer: true,
      sync: options.sync,
      dryRun: false,
      rerank: true,
      fileTypes: undefined,
      store: options.store,
    });
  });

program
  .argument("[pattern]", "Search pattern (if no command specified)")
  .action(async (pattern) => {
    if (pattern) {
      await searchCommand(pattern, undefined, {
        maxCount: 10,
        content: false,
        answer: false,
        sync: false,
        dryRun: false,
        rerank: true,
        fileTypes: undefined,
      });
    } else {
      program.help();
    }
  });

program.addHelpText(
  "after",
  `
${chalk.cyan("Examples:")}
  ${chalk.gray("# Index your codebase")}
  $ searchgrep watch

  ${chalk.gray("# Search with natural language")}
  $ searchgrep search "authentication middleware"
  $ searchgrep search "where are API errors handled" --content

  ${chalk.gray("# Ask questions about your code")}
  $ searchgrep ask "how does the login flow work"

  ${chalk.gray("# Quick search (default command)")}
  $ searchgrep "database connection pooling"

${chalk.cyan("Configuration:")}
  ${chalk.gray("# Set your OpenAI API key")}
  $ searchgrep config --api-key sk-...
  $ export OPENAI_API_KEY=sk-...

  ${chalk.gray("# Use local embeddings (BGE-base, no API key needed)")}
  $ searchgrep config --provider local

  ${chalk.gray("# Filter by file type")}
  $ searchgrep search "api routes" --type ts
  $ searchgrep search "data models" -t py -t sql

  ${chalk.gray("# View current config")}
  $ searchgrep config --show
`,
);

program.parse();
