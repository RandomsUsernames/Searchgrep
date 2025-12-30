import ora from "ora";
import chalk from "chalk";
import { VectorStore } from "../lib/store.js";
import { createFileSystem } from "../lib/file.js";
import {
  analyzeCodebase,
  classifyCodebaseSize,
  getScalingConfig,
  getCodebaseSizeDescription,
  optimizeFileOrder,
  createBatches,
  formatBytes,
  isGeneratedFile,
  isMinifiedFile,
  isVendorFile,
  type ScalingConfig,
} from "../lib/scaling.js";
import { setSpeedMode } from "../lib/local-embeddings.js";

interface IndexOptions {
  store?: string;
  force?: boolean; // Force reindex all files
  verbose?: boolean;
  fast?: boolean; // Use fast mode (smaller model, larger batches)
  quality?: boolean; // Use quality mode (fp32, slower but more accurate)
}

export async function indexCommand(
  path?: string,
  options: IndexOptions = {},
): Promise<void> {
  const cwd = path || process.cwd();
  const storeName = options.store || "searchgrep";

  console.log(chalk.cyan("\n🔍 Searchgrep") + " - Smart Codebase Indexer\n");

  // Set embedding speed mode
  if (options.fast) {
    setSpeedMode("fast");
    console.log(
      chalk.yellow("⚡ Fast mode enabled (smaller model, faster indexing)\n"),
    );
  } else if (options.quality) {
    setSpeedMode("quality");
    console.log(
      chalk.blue(
        "🎯 Quality mode enabled (higher accuracy, slower indexing)\n",
      ),
    );
  } else {
    setSpeedMode("balanced");
  }

  const spinner = ora("Scanning files...").start();

  try {
    // Scan files
    const fs = createFileSystem({ cwd });
    const files = await fs.getAllFiles();

    if (files.length === 0) {
      spinner.fail("No files found to index");
      console.log(
        chalk.yellow(
          "\nMake sure you're in a directory with source code files.",
        ),
      );
      return;
    }

    spinner.text = `Found ${files.length} files. Analyzing codebase...`;

    // Analyze codebase and get scaling config
    const metrics = analyzeCodebase(
      files.map((f) => ({ size: f.size, path: f.path })),
    );
    const codebaseSize = classifyCodebaseSize(metrics);
    const config = getScalingConfig(codebaseSize);

    spinner.succeed(getCodebaseSizeDescription(codebaseSize, metrics));

    // Show filtering stats
    const filesWithContent = files.map((f) => ({
      path: f.path,
      size: f.size,
      content: f.content,
    }));

    const optimizedFiles = optimizeFileOrder(filesWithContent, config);
    const filteredCount = files.length - optimizedFiles.length;

    if (filteredCount > 0) {
      console.log(
        chalk.gray(
          `  Filtered ${filteredCount} files (generated/minified/vendor)`,
        ),
      );
    }

    if (options.verbose) {
      showFilteredDetails(files, optimizedFiles, config);
    }

    // Initialize store with scaling config
    spinner.start(`Initializing index...`);
    const store = new VectorStore(storeName);

    // Check for existing files (incremental indexing)
    const existingFiles = store.listFiles();
    const existingHashes = new Map(existingFiles.map((f) => [f.path, f.hash]));

    // Determine which files need indexing
    const filesToIndex: typeof optimizedFiles = [];
    const skippedUnchanged: string[] = [];

    for (const file of optimizedFiles) {
      const fullFile = files.find((f) => f.path === file.path);
      if (!fullFile) continue;

      const hash = Buffer.from(fullFile.content)
        .toString("base64")
        .slice(0, 32);
      const existingHash = existingHashes.get(file.path);

      if (!options.force && existingHash === hash) {
        skippedUnchanged.push(file.path);
      } else {
        filesToIndex.push(file);
      }
    }

    if (skippedUnchanged.length > 0) {
      spinner.succeed(
        `Skipping ${skippedUnchanged.length} unchanged files (incremental mode)`,
      );
    }

    if (filesToIndex.length === 0) {
      console.log(chalk.green("\n✓ Index is already up to date!"));
      showStats(store, metrics);
      return;
    }

    // Create batches for processing
    const batches = createBatches(filesToIndex, config.batchSize);
    const totalFiles = filesToIndex.length;

    spinner.start(`Indexing ${totalFiles} files...`);

    let indexed = 0;
    let errors = 0;
    const startTime = Date.now();

    // Process batches
    for (let i = 0; i < batches.length; i++) {
      const batch = batches[i];

      // Process files in batch
      for (const fileInfo of batch) {
        const fullFile = files.find((f) => f.path === fileInfo.path);
        if (!fullFile) continue;

        try {
          const hash = Buffer.from(fullFile.content)
            .toString("base64")
            .slice(0, 32);

          await store.uploadFile(
            fullFile.path,
            fullFile.content,
            hash,
            fullFile.size,
            fullFile.lastModified,
          );

          indexed++;

          // Update progress
          const percent = Math.round((indexed / totalFiles) * 100);
          const elapsed = (Date.now() - startTime) / 1000;
          const rate = indexed / elapsed;
          const remaining = Math.round((totalFiles - indexed) / rate);

          spinner.text = `Indexing... ${indexed}/${totalFiles} (${percent}%) - ${remaining}s remaining`;
        } catch (e) {
          errors++;
          if (options.verbose) {
            console.log(chalk.red(`\n  Error indexing ${fileInfo.path}`));
          }
        }
      }
    }

    const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
    spinner.succeed(`Indexed ${indexed} files in ${elapsed}s`);

    if (errors > 0) {
      console.log(
        chalk.yellow(`  ${errors} files skipped (errors during indexing)`),
      );
    }

    // Show final stats
    showStats(store, metrics);

    console.log(
      chalk.green("\n✓ Ready to search!") +
        chalk.gray(" Run: ") +
        chalk.white("searchgrep search 'your query'\n"),
    );
  } catch (error) {
    spinner.fail("Indexing failed");
    console.error(
      chalk.red("\nError:"),
      error instanceof Error ? error.message : String(error),
    );
    process.exit(1);
  }
}

function showFilteredDetails(
  allFiles: { path: string; size: number; content: string }[],
  optimizedFiles: { path: string }[],
  config: ScalingConfig,
): void {
  const optimizedPaths = new Set(optimizedFiles.map((f) => f.path));
  const filtered = allFiles.filter((f) => !optimizedPaths.has(f.path));

  if (filtered.length === 0) return;

  console.log(chalk.gray("\n  Filtered files:"));

  let generated = 0;
  let minified = 0;
  let vendor = 0;

  for (const file of filtered) {
    if (isGeneratedFile(file.path, file.content)) generated++;
    else if (isMinifiedFile(file.path, file.content)) minified++;
    else if (isVendorFile(file.path)) vendor++;
  }

  if (generated > 0) console.log(chalk.gray(`    - ${generated} generated`));
  if (minified > 0) console.log(chalk.gray(`    - ${minified} minified`));
  if (vendor > 0) console.log(chalk.gray(`    - ${vendor} vendor/third-party`));
}

function showStats(
  store: VectorStore,
  metrics: { totalFiles: number; totalSize: number },
): void {
  const info = store.getInfo();

  console.log(chalk.gray("\n  Index stats:"));
  console.log(chalk.gray(`    Files indexed: ${info.fileCount}`));
  console.log(chalk.gray(`    Index size: ${formatBytes(info.totalSize)}`));
  console.log(
    chalk.gray(
      `    Last updated: ${new Date(info.lastUpdated).toLocaleString()}`,
    ),
  );
}
