import { execSync } from "node:child_process";
import { existsSync, readFileSync, statSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import ignore, { type Ignore } from "ignore";
import { isText } from "istextorbinary";
import { loadConfig } from "./config.js";

const DEFAULT_IGNORE_PATTERNS = [
  "node_modules",
  ".git",
  ".svn",
  ".hg",
  "dist",
  "build",
  "out",
  ".next",
  ".nuxt",
  "coverage",
  ".nyc_output",
  "*.lock",
  "package-lock.json",
  "yarn.lock",
  "pnpm-lock.yaml",
  "bun.lockb",
  "*.min.js",
  "*.min.css",
  "*.map",
  "*.bundle.js",
  "*.chunk.js",
  "*.d.ts",
  "*.pyc",
  "__pycache__",
  ".pytest_cache",
  ".mypy_cache",
  ".tox",
  "*.egg-info",
  ".eggs",
  "venv",
  ".venv",
  "env",
  ".env.local",
  ".env.*.local",
  "*.log",
  "*.tmp",
  "*.temp",
  "*.swp",
  "*.swo",
  ".DS_Store",
  "Thumbs.db",
  "*.ico",
  "*.png",
  "*.jpg",
  "*.jpeg",
  "*.gif",
  "*.svg",
  "*.webp",
  "*.mp3",
  "*.mp4",
  "*.wav",
  "*.avi",
  "*.mov",
  "*.pdf",
  "*.doc",
  "*.docx",
  "*.xls",
  "*.xlsx",
  "*.ppt",
  "*.pptx",
  "*.zip",
  "*.tar",
  "*.gz",
  "*.rar",
  "*.7z",
  "*.exe",
  "*.dll",
  "*.so",
  "*.dylib",
  "*.bin",
  "*.o",
  "*.a",
  "*.wasm",
  "*.ttf",
  "*.otf",
  "*.woff",
  "*.woff2",
  "*.eot",
  ".searchgrep",
  ".searchgreprc.yaml",
];

export interface FileInfo {
  path: string;
  absolutePath: string;
  content: string;
  size: number;
  lastModified: number;
  lines: number;
}

export interface FileSystemOptions {
  cwd?: string;
  maxFileSize?: number;
  maxFileCount?: number;
  additionalIgnore?: string[];
}

export class FileSystem {
  private cwd: string;
  private maxFileSize: number;
  private maxFileCount: number;
  private ignoreFilter: Ignore;
  private isGitRepo: boolean;

  constructor(options: FileSystemOptions = {}) {
    const config = loadConfig(options.cwd);

    this.cwd = resolve(options.cwd || process.cwd());
    this.maxFileSize = options.maxFileSize ?? config.maxFileSize;
    this.maxFileCount = options.maxFileCount ?? config.maxFileCount;
    this.isGitRepo = this.checkGitRepo();
    this.ignoreFilter = this.buildIgnoreFilter(options.additionalIgnore);
  }

  private checkGitRepo(): boolean {
    try {
      execSync("git rev-parse --is-inside-work-tree", {
        cwd: this.cwd,
        stdio: "pipe",
      });
      return true;
    } catch {
      return false;
    }
  }

  private buildIgnoreFilter(additionalPatterns?: string[]): Ignore {
    const ig = ignore();

    ig.add(DEFAULT_IGNORE_PATTERNS);

    const gitignorePath = join(this.cwd, ".gitignore");
    if (existsSync(gitignorePath)) {
      try {
        const gitignoreContent = readFileSync(gitignorePath, "utf-8");
        ig.add(
          gitignoreContent
            .split("\n")
            .filter((line) => line.trim() && !line.startsWith("#")),
        );
      } catch {
        // Ignore errors reading .gitignore
      }
    }

    const searchgrepignorePath = join(this.cwd, ".searchgrepignore");
    if (existsSync(searchgrepignorePath)) {
      try {
        const searchgrepignoreContent = readFileSync(
          searchgrepignorePath,
          "utf-8",
        );
        ig.add(
          searchgrepignoreContent
            .split("\n")
            .filter((line) => line.trim() && !line.startsWith("#")),
        );
      } catch {
        // Ignore errors
      }
    }

    if (additionalPatterns) {
      ig.add(additionalPatterns);
    }

    return ig;
  }

  private getGitFiles(): string[] {
    try {
      const output = execSync(
        "git ls-files --cached --others --exclude-standard",
        {
          cwd: this.cwd,
          encoding: "utf-8",
          maxBuffer: 50 * 1024 * 1024,
        },
      );
      return output
        .split("\n")
        .filter((line) => line.trim())
        .map((file) => file.trim());
    } catch {
      return [];
    }
  }

  private walkDirectory(dir: string, files: string[] = []): string[] {
    try {
      const entries = readdirSync(dir, { withFileTypes: true });

      for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        const relativePath = relative(this.cwd, fullPath);

        if (entry.name.startsWith(".")) continue;

        if (this.ignoreFilter.ignores(relativePath)) continue;

        if (entry.isDirectory()) {
          this.walkDirectory(fullPath, files);
        } else if (entry.isFile()) {
          files.push(relativePath);
        }
      }
    } catch {
      // Ignore permission errors
    }

    return files;
  }

  async *getFiles(): AsyncGenerator<FileInfo> {
    let files: string[];

    if (this.isGitRepo) {
      files = this.getGitFiles();
    } else {
      files = this.walkDirectory(this.cwd);
    }

    files = files.filter((file) => !this.ignoreFilter.ignores(file));

    let fileCount = 0;

    for (const file of files) {
      if (fileCount >= this.maxFileCount) {
        break;
      }

      const absolutePath = join(this.cwd, file);

      try {
        const stat = statSync(absolutePath);

        if (!stat.isFile()) continue;
        if (stat.size > this.maxFileSize) continue;
        if (stat.size === 0) continue;

        const buffer = readFileSync(absolutePath);

        if (!isText(file, buffer)) continue;

        const content = buffer.toString("utf-8");

        fileCount++;
        yield {
          path: file,
          absolutePath,
          content,
          size: stat.size,
          lastModified: stat.mtimeMs,
          lines: content.split("\n").length,
        };
      } catch {
        // Skip files we can't read
        continue;
      }
    }
  }

  async getAllFiles(): Promise<FileInfo[]> {
    const files: FileInfo[] = [];
    for await (const file of this.getFiles()) {
      files.push(file);
    }
    return files;
  }

  readFile(filePath: string): FileInfo | null {
    const absolutePath = resolve(this.cwd, filePath);
    const relativePath = relative(this.cwd, absolutePath);

    try {
      const stat = statSync(absolutePath);
      if (!stat.isFile()) return null;

      const buffer = readFileSync(absolutePath);
      if (!isText(filePath, buffer)) return null;

      const content = buffer.toString("utf-8");

      return {
        path: relativePath,
        absolutePath,
        content,
        size: stat.size,
        lastModified: stat.mtimeMs,
        lines: content.split("\n").length,
      };
    } catch {
      return null;
    }
  }

  getCwd(): string {
    return this.cwd;
  }

  isGit(): boolean {
    return this.isGitRepo;
  }
}

export function createFileSystem(options?: FileSystemOptions): FileSystem {
  return new FileSystem(options);
}
