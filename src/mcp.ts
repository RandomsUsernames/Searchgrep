#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { VectorStore } from "./lib/store.js";
import { createFileSystem } from "./lib/file.js";
import { loadConfig } from "./lib/config.js";

const server = new Server(
  {
    name: "searchgrep",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

let store: VectorStore | null = null;

async function getStore(cwd?: string): Promise<VectorStore> {
  const config = loadConfig(cwd);
  if (!store) {
    store = new VectorStore({
      storeName: "default",
      embeddingProvider: config.embeddingProvider,
      openaiApiKey: config.openaiApiKey,
      embeddingModel: config.embeddingModel,
    });
  }
  return store;
}

// List available tools
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "search",
        description:
          "Search the codebase using natural language. Returns relevant file snippets ranked by semantic similarity.",
        inputSchema: {
          type: "object" as const,
          properties: {
            query: {
              type: "string",
              description: "Natural language search query (e.g., 'authentication middleware', 'database connection handling')",
            },
            maxResults: {
              type: "number",
              description: "Maximum number of results to return (default: 10)",
            },
            path: {
              type: "string",
              description: "Directory path to search in (default: current directory)",
            },
          },
          required: ["query"],
        },
      },
      {
        name: "index",
        description:
          "Index files in a directory for semantic search. Run this before searching a new codebase.",
        inputSchema: {
          type: "object" as const,
          properties: {
            path: {
              type: "string",
              description: "Directory path to index (default: current directory)",
            },
          },
        },
      },
      {
        name: "status",
        description: "Get the current index status and statistics.",
        inputSchema: {
          type: "object" as const,
          properties: {
            path: {
              type: "string",
              description: "Directory path to check status for",
            },
          },
        },
      },
    ],
  };
});

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case "search": {
        const query = args?.query as string;
        const maxResults = (args?.maxResults as number) || 10;
        const path = args?.path as string | undefined;

        const s = await getStore(path);
        const results = await s.search(query, maxResults);

        if (results.length === 0) {
          return {
            content: [
              {
                type: "text" as const,
                text: "No results found. Make sure the directory is indexed first using the 'index' tool.",
              },
            ],
          };
        }

        const formattedResults = results.map((r, i) => ({
          rank: i + 1,
          file: r.path,
          lines: `${r.startLine}-${r.endLine}`,
          score: (r.score * 100).toFixed(2) + "%",
          content: r.content.slice(0, 500) + (r.content.length > 500 ? "..." : ""),
        }));

        return {
          content: [
            {
              type: "text" as const,
              text: JSON.stringify(formattedResults, null, 2),
            },
          ],
        };
      }

      case "index": {
        const path = args?.path as string | undefined;
        const cwd = path || process.cwd();

        const fs = createFileSystem({ cwd });
        const s = await getStore(cwd);

        const files = await fs.getAllFiles();
        let indexed = 0;

        for (const file of files) {
          try {
            await s.upsertFile(file.path, file.content, file.lastModified);
            indexed++;
          } catch (e) {
            // Skip files that fail to index
          }
        }

        return {
          content: [
            {
              type: "text" as const,
              text: `Indexed ${indexed} files from ${cwd}`,
            },
          ],
        };
      }

      case "status": {
        const path = args?.path as string | undefined;
        const s = await getStore(path);
        const stats = await s.getStats();

        return {
          content: [
            {
              type: "text" as const,
              text: JSON.stringify(
                {
                  totalFiles: stats.totalFiles,
                  totalChunks: stats.totalChunks,
                  totalSize: stats.totalSize,
                  lastUpdated: stats.lastUpdated,
                },
                null,
                2
              ),
            },
          ],
        };
      }

      default:
        return {
          content: [
            {
              type: "text" as const,
              text: `Unknown tool: ${name}`,
            },
          ],
          isError: true,
        };
    }
  } catch (error) {
    return {
      content: [
        {
          type: "text" as const,
          text: `Error: ${error instanceof Error ? error.message : String(error)}`,
        },
      ],
      isError: true,
    };
  }
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("Searchgrep MCP server running on stdio");
}

main().catch(console.error);
