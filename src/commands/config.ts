import chalk from "chalk";
import * as clack from "@clack/prompts";
import {
  loadConfig,
  saveGlobalConfig,
  getGlobalConfigPath,
  getDataDir,
} from "../lib/config.js";
import { createStore } from "../lib/store.js";
import { formatBytes } from "../lib/utils.js";

export interface ConfigOptions {
  apiKey?: string;
  model?: string;
  baseUrl?: string;
  show?: boolean;
  clear?: boolean;
  provider?: "openai" | "local";
  localUrl?: string;
}

export async function configCommand(options: ConfigOptions): Promise<void> {
  if (options.clear) {
    const store = createStore();
    store.clear();
    console.log(chalk.green("✓ Store cleared successfully."));
    return;
  }

  if (options.show) {
    const config = loadConfig();
    const store = createStore();
    const info = store.getInfo();

    console.log(chalk.cyan("\n📋 searchgrep Configuration\n"));
    console.log(chalk.gray("─".repeat(40)));

    console.log(chalk.bold("Config file: ") + getGlobalConfigPath());
    console.log(chalk.bold("Data directory: ") + getDataDir());
    console.log();

    console.log(chalk.bold("Settings:"));
    console.log(
      `  Embedding Provider: ${config.embeddingProvider === "local" ? chalk.cyan("local (C2LLM-0.5B)") : chalk.blue("openai")}`,
    );
    if (config.embeddingProvider === "local") {
      console.log(`  Local Server URL: ${config.localEmbeddingUrl}`);
    } else {
      console.log(
        `  API Key: ${config.openaiApiKey ? chalk.green("configured") : chalk.yellow("not set")}`,
      );
      console.log(`  Embedding Model: ${config.embeddingModel}`);
      console.log(`  Base URL: ${config.baseUrl || chalk.gray("(default)")}`);
    }
    console.log(`  Max File Size: ${formatBytes(config.maxFileSize)}`);
    console.log(`  Max File Count: ${config.maxFileCount.toLocaleString()}`);
    console.log();

    console.log(chalk.bold("Store:"));
    console.log(`  Files indexed: ${info.fileCount.toLocaleString()}`);
    console.log(`  Total size: ${formatBytes(info.totalSize)}`);
    console.log(
      `  Last updated: ${info.lastUpdated ? new Date(info.lastUpdated).toLocaleString() : "never"}`,
    );

    console.log(chalk.gray("\n─".repeat(40)));
    return;
  }

  if (options.apiKey) {
    saveGlobalConfig({ openaiApiKey: options.apiKey });
    console.log(chalk.green("✓ API key saved successfully."));
    return;
  }

  if (options.model) {
    saveGlobalConfig({ embeddingModel: options.model });
    console.log(chalk.green(`✓ Embedding model set to: ${options.model}`));
    return;
  }

  if (options.baseUrl) {
    saveGlobalConfig({ baseUrl: options.baseUrl });
    console.log(chalk.green(`✓ Base URL set to: ${options.baseUrl}`));
    return;
  }

  if (options.provider) {
    saveGlobalConfig({ embeddingProvider: options.provider });
    if (options.provider === "local") {
      console.log(chalk.green(`✓ Embedding provider set to: local (BGE-base)`));
      console.log(
        chalk.gray(
          `  No API key required. Model downloads on first use (~90MB).`,
        ),
      );
    } else {
      console.log(chalk.green(`✓ Embedding provider set to: openai`));
    }
    return;
  }

  if (options.localUrl) {
    saveGlobalConfig({ localEmbeddingUrl: options.localUrl });
    console.log(
      chalk.green(`✓ Local embedding URL set to: ${options.localUrl}`),
    );
    return;
  }

  clack.intro(chalk.cyan("searchgrep configuration"));

  const config = loadConfig();

  const apiKey = await clack.text({
    message: "Enter your OpenAI API key:",
    placeholder: config.openaiApiKey ? "(keep existing)" : "sk-...",
    validate: (value) => {
      if (!value && !config.openaiApiKey) {
        return "API key is required";
      }
    },
  });

  if (clack.isCancel(apiKey)) {
    clack.cancel("Configuration cancelled.");
    return;
  }

  if (apiKey) {
    saveGlobalConfig({ openaiApiKey: apiKey });
  }

  const model = await clack.select({
    message: "Select embedding model:",
    options: [
      {
        value: "text-embedding-3-small",
        label: "text-embedding-3-small (recommended)",
      },
      {
        value: "text-embedding-3-large",
        label: "text-embedding-3-large (higher quality)",
      },
      {
        value: "text-embedding-ada-002",
        label: "text-embedding-ada-002 (legacy)",
      },
    ],
    initialValue: config.embeddingModel,
  });

  if (clack.isCancel(model)) {
    clack.cancel("Configuration cancelled.");
    return;
  }

  saveGlobalConfig({ embeddingModel: model as string });

  clack.outro(chalk.green("Configuration saved!"));
}
