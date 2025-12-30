import { watch } from "chokidar";
import chalk from "chalk";
import ora from "ora";
import { createStore } from "../lib/store.js";
import { createFileSystem } from "../lib/file.js";
import {
  syncFiles,
  hashContent,
  formatDuration,
  formatBytes,
} from "../lib/utils.js";

export interface WatchOptions {
  store?: string;
  once?: boolean;
}

export async function watchCommand(
  path: string | undefined,
  options: WatchOptions,
): Promise<void> {
  const cwd = path || process.cwd();
  const store = createStore(options.store);
  const fileSystem = createFileSystem({ cwd });

  console.log(chalk.cyan(`\n📁 Watching: ${cwd}\n`));

  const spinner = ora("Initial sync in progress...").start();

  try {
    const result = await syncFiles(store, fileSystem, {
      onProgress: (progress) => {
        switch (progress.phase) {
          case "scanning":
            spinner.text = `Scanning files... (${progress.total} found)`;
            break;
          case "comparing":
            spinner.text = `Comparing ${progress.total} files...`;
            break;
          case "uploading":
            if (progress.currentFile) {
              spinner.text = `Uploading [${progress.current}/${progress.total}]: ${progress.currentFile}`;
            }
            break;
          case "deleting":
            if (progress.currentFile) {
              spinner.text = `Removing [${progress.current}/${progress.total}]: ${progress.currentFile}`;
            }
            break;
          case "done":
            break;
        }
      },
    });

    const info = store.getInfo();
    spinner.succeed(
      `Initial sync complete: ${result.uploaded} added, ${result.deleted} removed, ${result.skipped} unchanged`,
    );
    console.log(
      chalk.gray(
        `  Total: ${info.fileCount} files (${formatBytes(info.totalSize)}) in ${formatDuration(result.duration)}`,
      ),
    );

    if (result.errors.length > 0) {
      console.log(chalk.yellow(`\n  ${result.errors.length} errors occurred:`));
      result.errors
        .slice(0, 3)
        .forEach((err) => console.log(chalk.red(`    ${err}`)));
      if (result.errors.length > 3) {
        console.log(
          chalk.yellow(`    ... and ${result.errors.length - 3} more`),
        );
      }
    }
  } catch (error) {
    spinner.fail(`Initial sync failed: ${error}`);

    if (String(error).includes("API key")) {
      console.log(
        chalk.yellow(
          "\nTo configure your API key, run: searchgrep config --api-key <your-openai-key>",
        ),
      );
      console.log(
        chalk.yellow("Or set the OPENAI_API_KEY environment variable."),
      );
      return;
    }
  }

  if (options.once) {
    console.log(chalk.green("\n✓ Sync complete."));
    return;
  }

  console.log(chalk.cyan("\n👀 Watching for changes... (Ctrl+C to stop)\n"));

  const watcher = watch(cwd, {
    ignored: [
      /(^|[\/\\])\../,
      "**/node_modules/**",
      "**/.git/**",
      "**/dist/**",
      "**/build/**",
      "**/*.lock",
      "**/package-lock.json",
    ],
    persistent: true,
    ignoreInitial: true,
    awaitWriteFinish: {
      stabilityThreshold: 500,
      pollInterval: 100,
    },
  });

  const pendingUpdates = new Map<string, NodeJS.Timeout>();

  const processFile = async (
    filePath: string,
    action: "add" | "change" | "unlink",
  ) => {
    const relativePath = filePath
      .replace(cwd + "/", "")
      .replace(cwd + "\\", "");

    const existingTimeout = pendingUpdates.get(relativePath);
    if (existingTimeout) {
      clearTimeout(existingTimeout);
    }

    pendingUpdates.set(
      relativePath,
      setTimeout(async () => {
        pendingUpdates.delete(relativePath);

        try {
          if (action === "unlink") {
            await store.deleteFile(relativePath);
            console.log(chalk.red(`  ✗ Removed: ${relativePath}`));
          } else {
            const file = fileSystem.readFile(relativePath);
            if (file) {
              const hash = await hashContent(file.content);
              await store.uploadFile(
                relativePath,
                file.content,
                hash,
                file.size,
                file.lastModified,
              );
              console.log(
                chalk.green(
                  `  ${action === "add" ? "+" : "~"} ${action === "add" ? "Added" : "Updated"}: ${relativePath}`,
                ),
              );
            }
          }
        } catch (error) {
          console.log(
            chalk.red(`  ✗ Error processing ${relativePath}: ${error}`),
          );
        }
      }, 300),
    );
  };

  watcher
    .on("add", (path) => processFile(path, "add"))
    .on("change", (path) => processFile(path, "change"))
    .on("unlink", (path) => processFile(path, "unlink"))
    .on("error", (error) => console.log(chalk.red(`Watcher error: ${error}`)));

  process.on("SIGINT", () => {
    console.log(chalk.cyan("\n\n👋 Stopping watcher..."));
    watcher.close();
    process.exit(0);
  });

  await new Promise(() => {});
}
