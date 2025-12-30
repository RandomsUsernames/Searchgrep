import chalk from "chalk";
import { createStore } from "../lib/store.js";
import { loadConfig } from "../lib/config.js";
import { formatBytes } from "../lib/utils.js";

export interface StatusOptions {
  store?: string;
  files?: boolean;
}

export async function statusCommand(options: StatusOptions): Promise<void> {
  const config = loadConfig();
  const store = createStore(options.store);
  const info = store.getInfo();

  console.log(chalk.cyan("\n📊 searchgrep Status\n"));
  console.log(chalk.gray("─".repeat(50)));

  const apiConfigured = !!config.openaiApiKey;
  console.log(chalk.bold("Configuration:"));
  console.log(
    `  API Key: ${apiConfigured ? chalk.green("✓ configured") : chalk.red("✗ not set")}`,
  );
  console.log(`  Model: ${config.embeddingModel}`);
  console.log();

  console.log(chalk.bold("Index:"));
  console.log(`  Store: ${info.name}`);
  console.log(`  Files: ${info.fileCount.toLocaleString()}`);
  console.log(`  Size: ${formatBytes(info.totalSize)}`);
  console.log(
    `  Updated: ${info.lastUpdated ? new Date(info.lastUpdated).toLocaleString() : "never"}`,
  );

  if (options.files && info.fileCount > 0) {
    console.log(chalk.gray("\n─".repeat(50)));
    console.log(chalk.bold("\nIndexed Files:"));

    const files = store.listFiles();
    const sorted = files.sort((a, b) => b.lastModified - a.lastModified);

    for (const file of sorted.slice(0, 50)) {
      const sizeStr = formatBytes(file.size).padStart(10);
      const linesStr = `${file.lines} lines`.padStart(12);
      console.log(chalk.gray(`  ${sizeStr} ${linesStr}  `) + file.path);
    }

    if (files.length > 50) {
      console.log(chalk.gray(`  ... and ${files.length - 50} more files`));
    }
  }

  console.log(chalk.gray("\n─".repeat(50)));

  if (!apiConfigured) {
    console.log(chalk.yellow("\n⚠ To get started, configure your API key:"));
    console.log(chalk.gray("  searchgrep config --api-key <your-openai-key>"));
    console.log(chalk.gray("  # or set OPENAI_API_KEY environment variable\n"));
  } else if (info.fileCount === 0) {
    console.log(chalk.yellow("\n⚠ No files indexed. Run:"));
    console.log(
      chalk.gray("  searchgrep watch       # to index and watch for changes"),
    );
    console.log(
      chalk.gray(
        "  searchgrep watch --once  # to index once without watching\n",
      ),
    );
  } else {
    console.log(chalk.green("\n✓ Ready to search. Try:"));
    console.log(chalk.gray("  searchgrep search 'your query'\n"));
  }
}
