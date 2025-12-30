import { existsSync, readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, dirname } from "node:path";
import { parse as parseYaml } from "yaml";
import { z } from "zod";

const ConfigSchema = z.object({
  maxFileSize: z
    .number()
    .optional()
    .default(10 * 1024 * 1024), // 10MB
  maxFileCount: z.number().optional().default(10000),
  openaiApiKey: z.string().optional(),
  embeddingModel: z.string().optional().default("text-embedding-3-small"),
  baseUrl: z.string().optional(),
  // Local embedding options
  embeddingProvider: z.enum(["openai", "local"]).optional().default("openai"),
  localEmbeddingUrl: z.string().optional().default("http://127.0.0.1:11434"),
});

export type Config = z.infer<typeof ConfigSchema>;

const configCache = new Map<string, Config>();

export function getConfigDir(): string {
  const configDir = join(homedir(), ".config", "searchgrep");
  if (!existsSync(configDir)) {
    mkdirSync(configDir, { recursive: true });
  }
  return configDir;
}

export function getDataDir(): string {
  const dataDir = join(homedir(), ".searchgrep");
  if (!existsSync(dataDir)) {
    mkdirSync(dataDir, { recursive: true });
  }
  return dataDir;
}

export function getGlobalConfigPath(): string {
  return join(getConfigDir(), "config.yaml");
}

export function getLocalConfigPath(cwd: string = process.cwd()): string {
  return join(cwd, ".searchgreprc.yaml");
}

function loadConfigFile(path: string): Partial<Config> {
  if (!existsSync(path)) {
    return {};
  }

  try {
    const content = readFileSync(path, "utf-8");
    const parsed = parseYaml(content);
    return ConfigSchema.partial().parse(parsed || {});
  } catch (error) {
    console.warn(`Warning: Failed to parse config at ${path}`);
    return {};
  }
}

export function loadConfig(cwd: string = process.cwd()): Config {
  const cacheKey = cwd;
  if (configCache.has(cacheKey)) {
    return configCache.get(cacheKey)!;
  }

  const globalConfig = loadConfigFile(getGlobalConfigPath());
  const localConfig = loadConfigFile(getLocalConfigPath(cwd));

  const envConfig: Partial<Config> = {};
  if (process.env.OPENAI_API_KEY) {
    envConfig.openaiApiKey = process.env.OPENAI_API_KEY;
  }
  if (process.env.SEARCHGREP_MAX_FILE_SIZE) {
    envConfig.maxFileSize = parseInt(process.env.SEARCHGREP_MAX_FILE_SIZE, 10);
  }
  if (process.env.SEARCHGREP_MAX_FILE_COUNT) {
    envConfig.maxFileCount = parseInt(
      process.env.SEARCHGREP_MAX_FILE_COUNT,
      10,
    );
  }
  if (process.env.SEARCHGREP_EMBEDDING_MODEL) {
    envConfig.embeddingModel = process.env.SEARCHGREP_EMBEDDING_MODEL;
  }
  if (process.env.OPENAI_BASE_URL) {
    envConfig.baseUrl = process.env.OPENAI_BASE_URL;
  }
  if (process.env.SEARCHGREP_EMBEDDING_PROVIDER) {
    envConfig.embeddingProvider = process.env.SEARCHGREP_EMBEDDING_PROVIDER as
      | "openai"
      | "local";
  }
  if (process.env.SEARCHGREP_LOCAL_EMBEDDING_URL) {
    envConfig.localEmbeddingUrl = process.env.SEARCHGREP_LOCAL_EMBEDDING_URL;
  }

  const mergedConfig = {
    ...ConfigSchema.parse({}),
    ...globalConfig,
    ...localConfig,
    ...envConfig,
  };

  configCache.set(cacheKey, mergedConfig);
  return mergedConfig;
}

export function saveGlobalConfig(config: Partial<Config>): void {
  const configPath = getGlobalConfigPath();
  const existingConfig = loadConfigFile(configPath);
  const newConfig = { ...existingConfig, ...config };

  const configDir = dirname(configPath);
  if (!existsSync(configDir)) {
    mkdirSync(configDir, { recursive: true });
  }

  const yamlContent = Object.entries(newConfig)
    .map(([key, value]) => `${key}: ${JSON.stringify(value)}`)
    .join("\n");

  writeFileSync(configPath, yamlContent, "utf-8");
  configCache.clear();
}

export function clearConfigCache(): void {
  configCache.clear();
}
