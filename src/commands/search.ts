import chalk from "chalk";
import ora from "ora";
import { createStore } from "../lib/store.js";
import { createFileSystem } from "../lib/file.js";
import {
  syncFiles,
  formatScore,
  formatBytes,
  formatDuration,
} from "../lib/utils.js";

export interface SearchOptions {
  maxCount: number;
  content: boolean;
  answer: boolean;
  sync: boolean;
  dryRun: boolean;
  rerank: boolean;
  fileTypes?: string[];
  store?: string;
}

export async function searchCommand(
  pattern: string,
  path: string | undefined,
  options: SearchOptions,
): Promise<void> {
  const store = createStore(options.store);
  const storeInfo = store.getInfo();

  if (options.sync || storeInfo.fileCount === 0) {
    const spinner = ora("Syncing files...").start();
    const fileSystem = createFileSystem({ cwd: path });

    try {
      const result = await syncFiles(store, fileSystem, {
        dryRun: options.dryRun,
        onProgress: (progress) => {
          switch (progress.phase) {
            case "scanning":
              spinner.text = `Scanning files... (${progress.total} found)`;
              break;
            case "comparing":
              spinner.text = `Comparing ${progress.total} files...`;
              break;
            case "uploading":
              spinner.text = `Uploading ${progress.current}/${progress.total} files...`;
              break;
            case "deleting":
              spinner.text = `Cleaning up ${progress.current}/${progress.total} files...`;
              break;
            case "done":
              break;
          }
        },
      });

      if (options.dryRun) {
        spinner.info(
          `Dry run: would upload ${result.uploaded}, delete ${result.deleted}, skip ${result.skipped}`,
        );
      } else {
        spinner.succeed(
          `Synced: ${result.uploaded} uploaded, ${result.deleted} deleted, ${result.skipped} unchanged (${formatDuration(result.duration)})`,
        );
      }

      if (result.errors.length > 0) {
        console.log(
          chalk.yellow(
            `\n${result.errors.length} errors occurred during sync:`,
          ),
        );
        result.errors
          .slice(0, 5)
          .forEach((err) => console.log(chalk.red(`  ${err}`)));
        if (result.errors.length > 5) {
          console.log(
            chalk.yellow(`  ... and ${result.errors.length - 5} more`),
          );
        }
      }
    } catch (error) {
      spinner.fail(`Sync failed: ${error}`);
      if (storeInfo.fileCount === 0) {
        console.log(
          chalk.yellow(
            "\nNo files indexed. Run 'searchgrep watch' to index your files.",
          ),
        );
        return;
      }
    }
  }

  if (store.getInfo().fileCount === 0) {
    console.log(
      chalk.yellow(
        "No files indexed. Run 'searchgrep watch' or 'searchgrep search --sync' first.",
      ),
    );
    return;
  }

  const searchSpinner = ora("Searching...").start();

  try {
    if (options.answer) {
      searchSpinner.text = "Generating answer...";
      const answer = await store.ask(pattern, options.maxCount);
      searchSpinner.stop();

      console.log(chalk.cyan("\n" + "─".repeat(60)));
      console.log(chalk.bold("Answer:\n"));
      console.log(answer);
      console.log(chalk.cyan("─".repeat(60) + "\n"));
    } else {
      // Get more results for reranking, then narrow down
      const fetchCount = options.rerank
        ? options.maxCount * 3
        : options.maxCount;
      let results = await store.search(pattern, fetchCount, {
        hybrid: true,
        fileTypes: options.fileTypes,
      });

      // Apply reranking if enabled
      if (options.rerank && results.length > 0) {
        searchSpinner.text = "Reranking results...";
        results = await store.rerank(pattern, results, options.maxCount);
      }

      searchSpinner.stop();

      if (results.length === 0) {
        console.log(chalk.yellow("No results found."));
        return;
      }

      console.log(chalk.gray(`\nFound ${results.length} results:\n`));

      for (const result of results) {
        const score = formatScore(result.score);
        const location =
          result.lineStart && result.lineEnd
            ? `${result.path}:${result.lineStart}-${result.lineEnd}`
            : result.path;

        console.log(chalk.green(location) + chalk.gray(` (${score} match)`));

        if (options.content && result.chunk) {
          const lines = result.chunk.split("\n").slice(0, 10);
          const preview = lines.join("\n");
          console.log(chalk.gray("─".repeat(40)));
          console.log(preview);
          if (lines.length < result.chunk.split("\n").length) {
            console.log(chalk.gray("..."));
          }
          console.log(chalk.gray("─".repeat(40)));
        }

        console.log();
      }
    }
  } catch (error) {
    searchSpinner.fail(`Search failed: ${error}`);

    if (String(error).includes("API key")) {
      console.log(
        chalk.yellow(
          "\nTo configure your API key, run: searchgrep config --api-key <your-openai-key>",
        ),
      );
      console.log(
        chalk.yellow("Or set the OPENAI_API_KEY environment variable."),
      );
    }
  }
}
