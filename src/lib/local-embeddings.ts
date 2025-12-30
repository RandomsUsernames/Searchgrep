// Dynamic import to avoid sharp loading issues
let transformersModule: typeof import("@huggingface/transformers") | null =
  null;

// Singleton embedder instance
let embedder: any = null;
let isLoading = false;
let loadPromise: Promise<void> | null = null;

const MODEL_ID = "Xenova/bge-base-en-v1.5";

async function getTransformers() {
  if (!transformersModule) {
    transformersModule = await import("@huggingface/transformers");
    // Configure after import
    transformersModule.env.cacheDir = "./.searchgrep-cache";
    transformersModule.env.allowLocalModels = true;
  }
  return transformersModule;
}

export async function initLocalEmbedder(): Promise<void> {
  if (embedder) return;
  if (isLoading && loadPromise) {
    await loadPromise;
    return;
  }

  isLoading = true;
  loadPromise = (async () => {
    console.log("Loading local embedding model (first run downloads ~90MB)...");
    const { pipeline } = await getTransformers();
    embedder = await pipeline("feature-extraction", MODEL_ID, {
      dtype: "fp32",
    });
    console.log("Local embedding model loaded.");
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

  // Process in batches to avoid memory issues
  const batchSize = 8;
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
  }

  return results;
}

export function isLocalEmbedderReady(): boolean {
  return embedder !== null;
}
