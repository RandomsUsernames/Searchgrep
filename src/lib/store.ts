import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  unlinkSync,
} from "node:fs";
import { join } from "node:path";
import OpenAI from "openai";
import { loadConfig, getDataDir } from "./config.js";
import {
  getLocalEmbedding,
  getLocalEmbeddings,
  initLocalEmbedder,
} from "./local-embeddings.js";

export interface FileMetadata {
  path: string;
  hash: string;
  lines?: number;
  size: number;
  lastModified: number;
}

export interface SearchResult {
  path: string;
  score: number;
  content?: string;
  lineStart?: number;
  lineEnd?: number;
  chunk?: string;
}

export interface StoreInfo {
  name: string;
  fileCount: number;
  totalSize: number;
  lastUpdated: number;
}

interface StoredDocument {
  id: string;
  path: string;
  hash: string;
  content: string;
  embedding: number[];
  lines: number;
  size: number;
  lastModified: number;
  chunks: ChunkData[];
}

interface ChunkData {
  content: string;
  embedding: number[];
  lineStart: number;
  lineEnd: number;
}

interface StoreData {
  documents: StoredDocument[];
  metadata: {
    name: string;
    created: number;
    updated: number;
  };
}

export class VectorStore {
  private storePath: string;
  private data: StoreData;
  private openai: OpenAI | null = null;
  private embeddingModel: string;
  private embeddingDimension: number = 1536;
  private embeddingProvider: "openai" | "local";

  constructor(storeName: string = "searchgrep") {
    const dataDir = getDataDir();
    this.storePath = join(dataDir, `${storeName}.json`);
    this.data = this.loadStore();

    const config = loadConfig();
    this.embeddingModel = config.embeddingModel || "text-embedding-3-small";
    this.embeddingProvider = config.embeddingProvider || "openai";

    if (config.openaiApiKey) {
      this.openai = new OpenAI({
        apiKey: config.openaiApiKey,
        baseURL: config.baseUrl,
      });
    }
  }

  private loadStore(): StoreData {
    if (existsSync(this.storePath)) {
      try {
        const content = readFileSync(this.storePath, "utf-8");
        return JSON.parse(content);
      } catch {
        return this.createEmptyStore();
      }
    }
    return this.createEmptyStore();
  }

  private createEmptyStore(): StoreData {
    return {
      documents: [],
      metadata: {
        name: "searchgrep",
        created: Date.now(),
        updated: Date.now(),
      },
    };
  }

  private saveStore(): void {
    const dir = getDataDir();
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
    this.data.metadata.updated = Date.now();
    writeFileSync(this.storePath, JSON.stringify(this.data, null, 2));
  }

  async getEmbedding(
    text: string,
    isQuery: boolean = false,
  ): Promise<number[]> {
    if (this.embeddingProvider === "local") {
      return this.getLocalEmbeddingSingle(text, isQuery);
    }
    return this.getOpenAIEmbedding(text);
  }

  async getEmbeddings(
    texts: string[],
    isQuery: boolean = false,
  ): Promise<number[][]> {
    if (this.embeddingProvider === "local") {
      return this.getLocalEmbeddingsBatch(texts, isQuery);
    }
    return this.getOpenAIEmbeddings(texts);
  }

  private async getOpenAIEmbedding(text: string): Promise<number[]> {
    if (!this.openai) {
      throw new Error(
        "OpenAI API key not configured. Set OPENAI_API_KEY environment variable or run 'searchgrep config --api-key <key>'",
      );
    }

    const truncatedText = text.slice(0, 8000);

    const response = await this.openai.embeddings.create({
      model: this.embeddingModel,
      input: truncatedText,
    });

    return response.data[0].embedding;
  }

  private async getOpenAIEmbeddings(texts: string[]): Promise<number[][]> {
    if (!this.openai) {
      throw new Error("OpenAI API key not configured.");
    }

    const truncatedTexts = texts.map((t) => t.slice(0, 8000));

    const response = await this.openai.embeddings.create({
      model: this.embeddingModel,
      input: truncatedTexts,
    });

    return response.data.map((d) => d.embedding);
  }

  private async getLocalEmbeddingSingle(
    text: string,
    _isQuery: boolean = false,
  ): Promise<number[]> {
    return getLocalEmbedding(text);
  }

  private async getLocalEmbeddingsBatch(
    texts: string[],
    _isQuery: boolean = false,
  ): Promise<number[][]> {
    return getLocalEmbeddings(texts);
  }

  async rerank(
    _query: string,
    results: SearchResult[],
    topK?: number,
  ): Promise<SearchResult[]> {
    if (results.length === 0) return results;
    // Results are already ranked by vector similarity
    return topK ? results.slice(0, topK) : results;
  }

  private cosineSimilarity(a: number[], b: number[]): number {
    if (a.length !== b.length) return 0;

    let dotProduct = 0;
    let normA = 0;
    let normB = 0;

    for (let i = 0; i < a.length; i++) {
      dotProduct += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }

    const magnitude = Math.sqrt(normA) * Math.sqrt(normB);
    return magnitude === 0 ? 0 : dotProduct / magnitude;
  }

  // BM25 implementation for hybrid search
  private tokenize(text: string): string[] {
    return text
      .toLowerCase()
      .replace(/[^\w\s]/g, " ")
      .split(/\s+/)
      .filter((t) => t.length > 1);
  }

  private computeIDF(term: string, documents: { tokens: string[] }[]): number {
    const n = documents.length;
    const df = documents.filter((d) => d.tokens.includes(term)).length;
    return Math.log((n - df + 0.5) / (df + 0.5) + 1);
  }

  private bm25Score(
    queryTokens: string[],
    docTokens: string[],
    avgDocLength: number,
    idfCache: Map<string, number>,
    k1: number = 1.5,
    b: number = 0.75,
  ): number {
    const docLength = docTokens.length;
    const termFreq = new Map<string, number>();

    for (const token of docTokens) {
      termFreq.set(token, (termFreq.get(token) || 0) + 1);
    }

    let score = 0;
    for (const term of queryTokens) {
      const tf = termFreq.get(term) || 0;
      const idf = idfCache.get(term) || 0;
      const numerator = tf * (k1 + 1);
      const denominator = tf + k1 * (1 - b + b * (docLength / avgDocLength));
      score += idf * (numerator / denominator);
    }

    return score;
  }

  private bm25Search(
    query: string,
    topK: number,
    documents: StoredDocument[],
  ): { path: string; score: number; chunk: ChunkData; doc: StoredDocument }[] {
    const queryTokens = this.tokenize(query);
    if (queryTokens.length === 0) return [];

    // Build document token list for IDF calculation
    const allChunks: {
      tokens: string[];
      chunk: ChunkData;
      doc: StoredDocument;
    }[] = [];

    for (const doc of documents) {
      for (const chunk of doc.chunks) {
        allChunks.push({
          tokens: this.tokenize(chunk.content),
          chunk,
          doc,
        });
      }
    }

    if (allChunks.length === 0) return [];

    // Compute IDF for query terms
    const idfCache = new Map<string, number>();
    for (const term of queryTokens) {
      idfCache.set(term, this.computeIDF(term, allChunks));
    }

    // Compute average document length
    const avgDocLength =
      allChunks.reduce((sum, d) => sum + d.tokens.length, 0) / allChunks.length;

    // Score all chunks
    const results = allChunks.map((item) => ({
      path: item.doc.path,
      score: this.bm25Score(queryTokens, item.tokens, avgDocLength, idfCache),
      chunk: item.chunk,
      doc: item.doc,
    }));

    // Sort by score and return top results
    results.sort((a, b) => b.score - a.score);
    return results.slice(0, topK * 3); // Get more for fusion
  }

  // Reciprocal Rank Fusion for combining BM25 and vector search
  private rrfFusion(
    vectorResults: {
      path: string;
      score: number;
      chunk: ChunkData;
      doc: StoredDocument;
    }[],
    bm25Results: {
      path: string;
      score: number;
      chunk: ChunkData;
      doc: StoredDocument;
    }[],
    k: number = 60,
  ): { path: string; score: number; chunk: ChunkData; doc: StoredDocument }[] {
    const scores = new Map<
      string,
      { score: number; chunk: ChunkData; doc: StoredDocument }
    >();

    // Score from vector search
    vectorResults.forEach((result, rank) => {
      const key = `${result.path}:${result.chunk.lineStart}`;
      const rrfScore = 1 / (k + rank + 1);
      const existing = scores.get(key);
      if (existing) {
        existing.score += rrfScore;
      } else {
        scores.set(key, {
          score: rrfScore,
          chunk: result.chunk,
          doc: result.doc,
        });
      }
    });

    // Score from BM25 search
    bm25Results.forEach((result, rank) => {
      const key = `${result.path}:${result.chunk.lineStart}`;
      const rrfScore = 1 / (k + rank + 1);
      const existing = scores.get(key);
      if (existing) {
        existing.score += rrfScore;
      } else {
        scores.set(key, {
          score: rrfScore,
          chunk: result.chunk,
          doc: result.doc,
        });
      }
    });

    // Convert to array and sort
    const fusedResults = Array.from(scores.entries()).map(([key, value]) => ({
      path: value.doc.path,
      score: value.score,
      chunk: value.chunk,
      doc: value.doc,
    }));

    fusedResults.sort((a, b) => b.score - a.score);
    return fusedResults;
  }

  // Code-aware chunking patterns for different languages
  private readonly codeBlockPatterns: RegExp[] = [
    // JavaScript/TypeScript function and class definitions
    /^(?:export\s+)?(?:async\s+)?(?:function\s+\w+|const\s+\w+\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>|class\s+\w+|interface\s+\w+|type\s+\w+\s*=)/,
    // Python function and class definitions
    /^(?:async\s+)?def\s+\w+|^class\s+\w+/,
    // Go function definitions
    /^func\s+(?:\([^)]+\)\s+)?\w+/,
    // Rust function and impl definitions
    /^(?:pub\s+)?(?:async\s+)?fn\s+\w+|^impl\s+/,
    // Java/C# method and class definitions
    /^(?:public|private|protected)?\s*(?:static\s+)?(?:async\s+)?(?:class|interface|void|int|string|bool|\w+)\s+\w+\s*[({]/,
    // Ruby method and class definitions
    /^(?:def\s+\w+|class\s+\w+|module\s+\w+)/,
  ];

  private isCodeBlockStart(line: string): boolean {
    const trimmed = line.trim();
    return this.codeBlockPatterns.some((pattern) => pattern.test(trimmed));
  }

  private getIndentLevel(line: string): number {
    const match = line.match(/^(\s*)/);
    return match ? match[1].length : 0;
  }

  private chunkText(
    content: string,
    chunkSize: number = 500,
    overlap: number = 100,
  ): { content: string; lineStart: number; lineEnd: number }[] {
    const lines = content.split("\n");
    const chunks: { content: string; lineStart: number; lineEnd: number }[] =
      [];

    // Try code-aware chunking first
    const codeChunks = this.chunkByCodeBlocks(lines, chunkSize);
    if (codeChunks.length > 0) {
      return codeChunks;
    }

    // Fall back to simple line-based chunking
    return this.chunkByLines(lines, chunkSize, overlap);
  }

  private chunkByCodeBlocks(
    lines: string[],
    maxChunkSize: number,
  ): { content: string; lineStart: number; lineEnd: number }[] {
    const chunks: { content: string; lineStart: number; lineEnd: number }[] =
      [];
    let currentChunk: string[] = [];
    let currentLineStart = 1;
    let blockStartIndent = -1;
    let inBlock = false;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const indent = this.getIndentLevel(line);
      const isBlockStart = this.isCodeBlockStart(line);

      // Start a new block
      if (isBlockStart && !inBlock) {
        // Save current chunk if it has content
        if (currentChunk.length > 0) {
          const chunkContent = currentChunk.join("\n");
          if (chunkContent.trim().length > 0) {
            chunks.push({
              content: chunkContent,
              lineStart: currentLineStart,
              lineEnd: currentLineStart + currentChunk.length - 1,
            });
          }
          currentChunk = [];
          currentLineStart = i + 1;
        }
        inBlock = true;
        blockStartIndent = indent;
      }

      currentChunk.push(line);

      // Check if block ends (back to same or lower indent with non-empty line)
      if (
        inBlock &&
        line.trim().length > 0 &&
        indent <= blockStartIndent &&
        i > currentLineStart - 1 &&
        !isBlockStart
      ) {
        // Check for closing braces or dedent
        if (
          line.trim() === "}" ||
          line.trim() === "};" ||
          line.trim() === "end" ||
          (indent < blockStartIndent && !line.trim().startsWith("//"))
        ) {
          const chunkContent = currentChunk.join("\n");
          chunks.push({
            content: chunkContent,
            lineStart: currentLineStart,
            lineEnd: currentLineStart + currentChunk.length - 1,
          });
          currentChunk = [];
          currentLineStart = i + 2;
          inBlock = false;
          blockStartIndent = -1;
        }
      }

      // Force chunk split if too large
      const currentSize = currentChunk.join("\n").length;
      if (currentSize >= maxChunkSize) {
        chunks.push({
          content: currentChunk.join("\n"),
          lineStart: currentLineStart,
          lineEnd: currentLineStart + currentChunk.length - 1,
        });
        currentChunk = [];
        currentLineStart = i + 2;
        inBlock = false;
        blockStartIndent = -1;
      }
    }

    // Add remaining content
    if (currentChunk.length > 0) {
      const chunkContent = currentChunk.join("\n");
      if (chunkContent.trim().length > 0) {
        chunks.push({
          content: chunkContent,
          lineStart: currentLineStart,
          lineEnd: currentLineStart + currentChunk.length - 1,
        });
      }
    }

    return chunks;
  }

  private chunkByLines(
    lines: string[],
    chunkSize: number,
    overlap: number,
  ): { content: string; lineStart: number; lineEnd: number }[] {
    const chunks: { content: string; lineStart: number; lineEnd: number }[] =
      [];

    let currentChunk: string[] = [];
    let currentLineStart = 1;
    let currentCharCount = 0;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      currentChunk.push(line);
      currentCharCount += line.length + 1;

      if (currentCharCount >= chunkSize || i === lines.length - 1) {
        chunks.push({
          content: currentChunk.join("\n"),
          lineStart: currentLineStart,
          lineEnd: currentLineStart + currentChunk.length - 1,
        });

        const overlapLines = Math.ceil(
          overlap / (currentCharCount / currentChunk.length),
        );
        const keepLines = Math.min(overlapLines, currentChunk.length);
        currentChunk = currentChunk.slice(-keepLines);
        currentLineStart =
          currentLineStart + currentChunk.length - keepLines + 1;
        currentCharCount = currentChunk.join("\n").length;
      }
    }

    return chunks;
  }

  async uploadFile(
    path: string,
    content: string,
    hash: string,
    size: number,
    lastModified: number,
  ): Promise<void> {
    const existingIndex = this.data.documents.findIndex((d) => d.path === path);
    if (existingIndex !== -1) {
      if (this.data.documents[existingIndex].hash === hash) {
        return;
      }
      this.data.documents.splice(existingIndex, 1);
    }

    const textChunks = this.chunkText(content);
    const chunkContents = textChunks.map(
      (c) => `File: ${path}\n\n${c.content}`,
    );

    const embeddings = await this.getEmbeddings(chunkContents);

    const chunks: ChunkData[] = textChunks.map((chunk, i) => ({
      content: chunk.content,
      embedding: embeddings[i],
      lineStart: chunk.lineStart,
      lineEnd: chunk.lineEnd,
    }));

    const fullEmbedding = await this.getEmbedding(
      `File: ${path}\n\n${content.slice(0, 2000)}`,
    );

    const doc: StoredDocument = {
      id: `${path}-${hash}`,
      path,
      hash,
      content,
      embedding: fullEmbedding,
      lines: content.split("\n").length,
      size,
      lastModified,
      chunks,
    };

    this.data.documents.push(doc);
    this.saveStore();
  }

  async deleteFile(path: string): Promise<void> {
    const index = this.data.documents.findIndex((d) => d.path === path);
    if (index !== -1) {
      this.data.documents.splice(index, 1);
      this.saveStore();
    }
  }

  // Check if a file path matches the given file types
  private matchesFileType(path: string, fileTypes: string[]): boolean {
    if (!fileTypes || fileTypes.length === 0) return true;

    const ext = path.split(".").pop()?.toLowerCase() || "";
    return fileTypes.some((type) => {
      const normalizedType = type.toLowerCase().replace(/^\./, "");
      return ext === normalizedType;
    });
  }

  // Get documents filtered by file type
  private getFilteredDocuments(fileTypes?: string[]): StoredDocument[] {
    if (!fileTypes || fileTypes.length === 0) {
      return this.data.documents;
    }
    return this.data.documents.filter((doc) =>
      this.matchesFileType(doc.path, fileTypes),
    );
  }

  async search(
    query: string,
    topK: number = 10,
    options: { hybrid?: boolean; fileTypes?: string[] } = {
      hybrid: true,
    },
  ): Promise<SearchResult[]> {
    const documents = this.getFilteredDocuments(options.fileTypes);

    if (documents.length === 0) {
      return [];
    }

    // Vector search
    const queryEmbedding = await this.getEmbedding(query, true);

    const vectorResults: {
      path: string;
      score: number;
      chunk: ChunkData;
      doc: StoredDocument;
    }[] = [];

    for (const doc of documents) {
      for (const chunk of doc.chunks) {
        const score = this.cosineSimilarity(queryEmbedding, chunk.embedding);
        vectorResults.push({ path: doc.path, score, chunk, doc });
      }
    }

    vectorResults.sort((a, b) => b.score - a.score);
    const topVectorResults = vectorResults.slice(0, topK * 3);

    // Hybrid search with BM25 + RRF fusion
    let results: {
      path: string;
      score: number;
      chunk: ChunkData;
      doc: StoredDocument;
    }[];

    if (options.hybrid !== false) {
      const bm25Results = this.bm25Search(query, topK, documents);
      results = this.rrfFusion(topVectorResults, bm25Results);
    } else {
      results = topVectorResults;
    }

    // Deduplicate by path, keeping highest scoring chunk
    const seenPaths = new Map<string, number>();
    const uniqueResults: SearchResult[] = [];

    for (const result of results) {
      const existingScore = seenPaths.get(result.path);
      if (existingScore === undefined || result.score > existingScore) {
        if (existingScore !== undefined) {
          const existingIdx = uniqueResults.findIndex(
            (r) => r.path === result.path,
          );
          if (existingIdx !== -1) {
            uniqueResults.splice(existingIdx, 1);
          }
        }
        seenPaths.set(result.path, result.score);
        uniqueResults.push({
          path: result.path,
          score: result.score,
          content: result.doc.content,
          lineStart: result.chunk.lineStart,
          lineEnd: result.chunk.lineEnd,
          chunk: result.chunk.content,
        });
      }

      if (uniqueResults.length >= topK * 2) break;
    }

    uniqueResults.sort((a, b) => b.score - a.score);
    return uniqueResults.slice(0, topK);
  }

  async ask(query: string, topK: number = 5): Promise<string> {
    if (!this.openai) {
      throw new Error("OpenAI API key not configured.");
    }

    const searchResults = await this.search(query, topK);

    if (searchResults.length === 0) {
      return "No relevant files found for your query.";
    }

    const context = searchResults
      .map((r) => {
        const snippet = r.chunk || r.content?.slice(0, 1000) || "";
        return `File: ${r.path} (lines ${r.lineStart}-${r.lineEnd})\n\`\`\`\n${snippet}\n\`\`\``;
      })
      .join("\n\n");

    const response = await this.openai.chat.completions.create({
      model: "gpt-4o-mini",
      messages: [
        {
          role: "system",
          content:
            "You are a helpful code assistant. Answer questions about the codebase based on the provided context. Be concise and specific.",
        },
        {
          role: "user",
          content: `Context from codebase:\n\n${context}\n\nQuestion: ${query}`,
        },
      ],
      max_tokens: 1000,
    });

    return (
      response.choices[0]?.message?.content || "Unable to generate answer."
    );
  }

  listFiles(): FileMetadata[] {
    return this.data.documents.map((doc) => ({
      path: doc.path,
      hash: doc.hash,
      lines: doc.lines,
      size: doc.size,
      lastModified: doc.lastModified,
    }));
  }

  getFileByPath(path: string): StoredDocument | undefined {
    return this.data.documents.find((d) => d.path === path);
  }

  getInfo(): StoreInfo {
    const totalSize = this.data.documents.reduce(
      (sum, doc) => sum + doc.size,
      0,
    );
    return {
      name: this.data.metadata.name,
      fileCount: this.data.documents.length,
      totalSize,
      lastUpdated: this.data.metadata.updated,
    };
  }

  clear(): void {
    this.data = this.createEmptyStore();
    if (existsSync(this.storePath)) {
      unlinkSync(this.storePath);
    }
  }
}

let storeInstance: VectorStore | null = null;

export function getStore(storeName?: string): VectorStore {
  if (!storeInstance || storeName) {
    storeInstance = new VectorStore(storeName);
  }
  return storeInstance;
}

export function createStore(storeName?: string): VectorStore {
  return new VectorStore(storeName);
}
