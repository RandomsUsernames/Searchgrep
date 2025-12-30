import { createHash } from "node:crypto";
import xxhash from "xxhash-wasm";
import type { VectorStore, FileMetadata } from "./store.js";
import type { FileSystem, FileInfo } from "./file.js";

let xxhashInstance: Awaited<ReturnType<typeof xxhash>> | null = null;

async function getXxhash() {
	if (!xxhashInstance) {
		xxhashInstance = await xxhash();
	}
	return xxhashInstance;
}

export async function hashContent(content: string): Promise<string> {
	try {
		const hasher = await getXxhash();
		const hash = hasher.h64(content);
		return `xxh64:${hash}`;
	} catch {
		const hash = createHash("sha256").update(content).digest("hex");
		return `sha256:${hash}`;
	}
}

export interface SyncProgress {
	phase: "scanning" | "comparing" | "uploading" | "deleting" | "done";
	total: number;
	current: number;
	currentFile?: string;
	uploaded: number;
	deleted: number;
	skipped: number;
	errors: number;
}

export interface SyncOptions {
	onProgress?: (progress: SyncProgress) => void;
	dryRun?: boolean;
	concurrency?: number;
}

export interface SyncResult {
	uploaded: number;
	deleted: number;
	skipped: number;
	errors: string[];
	duration: number;
}

export async function syncFiles(
	store: VectorStore,
	fileSystem: FileSystem,
	options: SyncOptions = {}
): Promise<SyncResult> {
	const startTime = Date.now();
	const { onProgress, dryRun = false, concurrency = 10 } = options;

	const progress: SyncProgress = {
		phase: "scanning",
		total: 0,
		current: 0,
		uploaded: 0,
		deleted: 0,
		skipped: 0,
		errors: 0,
	};

	const report = () => onProgress?.(progress);
	report();

	const localFiles: FileInfo[] = [];
	for await (const file of fileSystem.getFiles()) {
		localFiles.push(file);
		progress.total = localFiles.length;
		report();
	}

	progress.phase = "comparing";
	progress.total = localFiles.length;
	report();

	const storeFiles = store.listFiles();
	const storeFileMap = new Map<string, FileMetadata>();
	for (const file of storeFiles) {
		storeFileMap.set(file.path, file);
	}

	const toUpload: FileInfo[] = [];
	const toSkip: string[] = [];

	for (const file of localFiles) {
		const hash = await hashContent(file.content);
		const existing = storeFileMap.get(file.path);

		if (existing && existing.hash === hash) {
			toSkip.push(file.path);
		} else {
			toUpload.push({ ...file, content: file.content });
			(toUpload[toUpload.length - 1] as any).hash = hash;
		}
	}

	const localPathSet = new Set(localFiles.map((f) => f.path));
	const toDelete: string[] = [];
	for (const storePath of storeFileMap.keys()) {
		if (!localPathSet.has(storePath)) {
			toDelete.push(storePath);
		}
	}

	progress.skipped = toSkip.length;
	const errors: string[] = [];

	progress.phase = "uploading";
	progress.total = toUpload.length;
	progress.current = 0;
	report();

	if (!dryRun) {
		const pLimit = (await import("p-limit")).default;
		const limit = pLimit(concurrency);

		const uploadPromises = toUpload.map((file) =>
			limit(async () => {
				try {
					progress.currentFile = file.path;
					report();

					await store.uploadFile(
						file.path,
						file.content,
						(file as any).hash,
						file.size,
						file.lastModified
					);

					progress.uploaded++;
					progress.current++;
					report();
				} catch (error) {
					errors.push(`Failed to upload ${file.path}: ${error}`);
					progress.errors++;
					progress.current++;
					report();
				}
			})
		);

		await Promise.all(uploadPromises);
	} else {
		progress.uploaded = toUpload.length;
		progress.current = toUpload.length;
		report();
	}

	progress.phase = "deleting";
	progress.total = toDelete.length;
	progress.current = 0;
	report();

	if (!dryRun) {
		for (const path of toDelete) {
			try {
				progress.currentFile = path;
				report();

				await store.deleteFile(path);

				progress.deleted++;
				progress.current++;
				report();
			} catch (error) {
				errors.push(`Failed to delete ${path}: ${error}`);
				progress.errors++;
				progress.current++;
				report();
			}
		}
	} else {
		progress.deleted = toDelete.length;
		progress.current = toDelete.length;
		report();
	}

	progress.phase = "done";
	progress.currentFile = undefined;
	report();

	return {
		uploaded: progress.uploaded,
		deleted: progress.deleted,
		skipped: progress.skipped,
		errors,
		duration: Date.now() - startTime,
	};
}

export function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

export function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
	const minutes = Math.floor(ms / 60000);
	const seconds = Math.floor((ms % 60000) / 1000);
	return `${minutes}m ${seconds}s`;
}

export function formatScore(score: number): string {
	return `${(score * 100).toFixed(2)}%`;
}

export function truncateMiddle(str: string, maxLen: number): string {
	if (str.length <= maxLen) return str;
	const halfLen = Math.floor((maxLen - 3) / 2);
	return `${str.slice(0, halfLen)}...${str.slice(-halfLen)}`;
}
