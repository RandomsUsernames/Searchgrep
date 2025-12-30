// Dynamic import to avoid sharp loading issues
let transformersModule: typeof import("@huggingface/transformers") | null =
  null;

// Singleton embedder instance
let embedder: any = null;
let isLoading = false;
let loadPromise: Promise<void> | null = null;

// Speed mode: "quality" (fp32), "balanced" (fp16), "fast" (quantized)
let currentSpeedMode: "quality" | "balanced" | "fast" = "balanced";

const MODEL_ID = "Xenova/bge-base-en-v1.5";
const FAST_MODEL_ID = "Xenova/all-MiniLM-L6-v2"; // Smaller, faster model

async function getTransformers() {
  if (!transformersModule) {
    transformersModule = await import("@huggingface/transformers");
    // Configure after import
    transformersModule.env.cacheDir = "./.searchgrep-cache";
    transformersModule.env.allowLocalModels = true;
  }
  return transformersModule;
}

export function setSpeedMode(mode: "quality" | "balanced" | "fast"): void {
  if (mode !== currentSpeedMode && embedder) {
    // Reset embedder to reload with new settings
    embedder = null;
    isLoading = false;
    loadPromise = null;
  }
  currentSpeedMode = mode;
}

export function getSpeedMode(): "quality" | "balanced" | "fast" {
  return currentSpeedMode;
}

export async function initLocalEmbedder(): Promise<void> {
  if (embedder) return;
  if (isLoading && loadPromise) {
    await loadPromise;
    return;
  }

  isLoading = true;
  loadPromise = (async () => {
    const { pipeline } = await getTransformers();

    // Choose model and dtype based on speed mode
    const modelId = currentSpeedMode === "fast" ? FAST_MODEL_ID : MODEL_ID;
    const dtype = currentSpeedMode === "quality" ? "fp32" : "fp16";

    const modeDesc =
      currentSpeedMode === "fast"
        ? "fast"
        : currentSpeedMode === "quality"
          ? "high-quality"
          : "balanced";
    console.log(`Loading ${modeDesc} embedding model...`);

    embedder = await pipeline("feature-extraction", modelId, {
      dtype,
    });
    console.log("Embedding model loaded.");
  })();

  await loadPromise;
  isLoading = false;
}

export async function getLocalEmbedding(text: string): Promise<number[]> {
  if (!embedder) {
    await initLocalEmbedder();
  }

  // Truncate to ~8000 chars (roughly 2000 tokens)
  const truncated = text.slice(0, 8000);

  const result = await embedder(truncated, {
    pooling: "mean",
    normalize: true,
  });

  // Convert to regular array
  return Array.from(result.data as Float32Array);
}

export async function getLocalEmbeddings(texts: string[]): Promise<number[][]> {
  if (!embedder) {
    await initLocalEmbedder();
  }

  const results: number[][] = [];

  // Larger batch size for speed - adjust based on mode
  const batchSize =
    currentSpeedMode === "fast" ? 32 : currentSpeedMode === "balanced" ? 16 : 8;

  for (let i = 0; i < texts.length; i += batchSize) {
    const batch = texts.slice(i, i + batchSize);

    // Process batch in parallel
    const batchResults = await Promise.all(
      batch.map(async (text) => {
        const truncated = text.slice(0, 8000);
        const result = await embedder(truncated, {
          pooling: "mean",
          normalize: true,
        });
        return Array.from(result.data as Float32Array);
      }),
    );
    results.push(...batchResults);
  }

  return results;
}

// Batch embeddings with progress callback
export async function getLocalEmbeddingsWithProgress(
  texts: string[],
  onProgress?: (done: number, total: number) => void,
): Promise<number[][]> {
  if (!embedder) {
    await initLocalEmbedder();
  }

  const results: number[][] = [];
  const batchSize =
    currentSpeedMode === "fast" ? 32 : currentSpeedMode === "balanced" ? 16 : 8;

  for (let i = 0; i < texts.length; i += batchSize) {
    const batch = texts.slice(i, i + batchSize);

    const batchResults = await Promise.all(
      batch.map(async (text) => {
        const truncated = text.slice(0, 8000);
        const result = await embedder(truncated, {
          pooling: "mean",
          normalize: true,
        });
        return Array.from(result.data as Float32Array);
      }),
    );
    results.push(...batchResults);

    if (onProgress) {
      onProgress(Math.min(i + batchSize, texts.length), texts.length);
    }
  }

  return results;
}

export function isLocalEmbedderReady(): boolean {
  return embedder !== null;
}
