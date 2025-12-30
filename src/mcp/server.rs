//! MCP Server implementation for searchgrep
//!
//! Runs as a stdio JSON-RPC server for Claude Code integration.

use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::core::codemap::CodeMap;
use crate::core::hybrid_embedder::HybridEmbedder;
use crate::core::local_embeddings::{LocalEmbedder, SpeedMode};
use crate::core::search::HybridSearcher;
use crate::core::store::VectorStore;

use super::protocol::*;

pub struct McpServer {
    initialized: bool,
}

impl McpServer {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Run the MCP server (blocking, reads from stdin, writes to stdout)
    pub fn run(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            // Parse JSON-RPC request
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let response =
                        JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                    self.write_response(&mut stdout, &response)?;
                    continue;
                }
            };

            // Handle the request
            let response = self.handle_request(request);
            self.write_response(&mut stdout, &response)?;
        }

        Ok(())
    }

    fn write_response(&self, stdout: &mut io::Stdout, response: &JsonRpcResponse) -> Result<()> {
        let json = serde_json::to_string(response)?;
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "initialized" => JsonRpcResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            _ => JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&mut self, id: Option<Value>) -> JsonRpcResponse {
        self.initialized = true;

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: "searchgrep".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools = vec![
            ToolDefinition {
                name: "semantic_search".to_string(),
                description: "Search code semantically using natural language. Finds relevant code based on meaning, not just keywords. Uses AI embeddings to understand code context and find related files, functions, and patterns.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language search query (e.g., 'error handling for HTTP requests', 'database connection pooling', 'authentication middleware')"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional: Directory path to search in (defaults to current indexed directory)"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of results to return (default: 10, max: 50)",
                            "default": 10
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["balanced", "code", "hybrid"],
                            "description": "Search mode: 'balanced' (general), 'code' (code-optimized), 'hybrid' (best quality, combines both)",
                            "default": "balanced"
                        },
                        "include_content": {
                            "type": "boolean",
                            "description": "Include file content in results",
                            "default": true
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "index_directory".to_string(),
                description: "Index a directory for semantic search. Creates vector embeddings of all code files for fast semantic search.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path to index"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["fast", "balanced", "code"],
                            "description": "Indexing mode: 'fast' (quick, lower quality), 'balanced' (default), 'code' (code-optimized)",
                            "default": "balanced"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "get_codebase_map".to_string(),
                description: "Get a compact semantic map of the codebase. Returns all symbols (functions, structs, classes) with signatures - 90% fewer tokens than reading files. Use this FIRST to understand codebase structure before reading individual files.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path (defaults to current indexed directory)"
                        },
                        "minimal": {
                            "type": "boolean",
                            "description": "Return ultra-compact view (just function names per file)",
                            "default": false
                        }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "search_symbols".to_string(),
                description: "Search for symbols (functions, structs, classes) by name or signature. Much faster than grep for finding specific code elements.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Symbol name or signature to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory path (defaults to current indexed directory)"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum results to return (default: 20)",
                            "default": 20
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "expand_symbol".to_string(),
                description: "Get detailed info about a specific symbol including its dependencies and dependents. Use after search_symbols to understand code relationships.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "symbol_id": {
                            "type": "string",
                            "description": "Symbol ID in format 'file:name' (from search_symbols results)"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory path (defaults to current indexed directory)"
                        },
                        "include_code": {
                            "type": "boolean",
                            "description": "Include source code snippet for the symbol",
                            "default": false
                        }
                    },
                    "required": ["symbol_id"]
                }),
            },
            ToolDefinition {
                name: "find_similar_code".to_string(),
                description: "Find code similar to a given snippet or file. Useful for finding duplicates, similar patterns, or related implementations.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Code snippet to find similar code for"
                        },
                        "file": {
                            "type": "string",
                            "description": "Or: path to a file to find similar files to"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum results to return (default: 10)",
                            "default": 10
                        }
                    },
                    "required": []
                }),
            },
            ToolDefinition {
                name: "ask_codebase".to_string(),
                description: "Ask a natural language question about the codebase and get an AI-synthesized answer based on relevant code. Best for understanding how things work.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "Question about the codebase (e.g., 'How does authentication work?', 'Where are API routes defined?')"
                        },
                        "max_context": {
                            "type": "integer",
                            "description": "Maximum number of code snippets to use as context (default: 5)",
                            "default": 5
                        }
                    },
                    "required": ["question"]
                }),
            },
            ToolDefinition {
                name: "get_file_context".to_string(),
                description: "Get rich context about a specific file including its imports, exports, dependencies, and role in the codebase.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to get context for"
                        },
                        "include_content": {
                            "type": "boolean",
                            "description": "Include file content in response",
                            "default": false
                        }
                    },
                    "required": ["file_path"]
                }),
            },
            ToolDefinition {
                name: "list_indexed_files".to_string(),
                description: "List all files currently indexed for semantic search. Useful to check what's available to search.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Optional glob pattern to filter files (e.g., '*.rs', 'src/**/*.ts')"
                        }
                    },
                    "required": []
                }),
            },
        ];

        let result = ToolsListResult { tools };
        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing params".to_string());
            }
        };

        let call: ToolCallParams = match serde_json::from_value(params) {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid params: {}", e));
            }
        };

        let result = match call.name.as_str() {
            "semantic_search" => self.execute_semantic_search(call.arguments),
            "index_directory" => self.execute_index_directory(call.arguments),
            "get_codebase_map" => self.execute_get_codebase_map(call.arguments),
            "search_symbols" => self.execute_search_symbols(call.arguments),
            "expand_symbol" => self.execute_expand_symbol(call.arguments),
            "find_similar_code" => self.execute_find_similar_code(call.arguments),
            "ask_codebase" => self.execute_ask_codebase(call.arguments),
            "get_file_context" => self.execute_get_file_context(call.arguments),
            "list_indexed_files" => self.execute_list_indexed_files(call.arguments),
            _ => ToolCallResult::error(format!("Unknown tool: {}", call.name)),
        };

        JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
    }

    fn execute_semantic_search(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::error("Missing required 'query' argument".to_string()),
        };

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as usize;

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        let include_content = args
            .get("include_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let store_path = args.get("path").and_then(|v| v.as_str());

        // Load the vector store
        let store = match VectorStore::load(store_path) {
            Ok(s) => s,
            Err(e) => return ToolCallResult::error(format!("Failed to load index: {}", e)),
        };

        if store.chunk_count() == 0 {
            return ToolCallResult::error(
                "No files indexed. Run 'searchgrep watch <path>' first to index your codebase."
                    .to_string(),
            );
        }

        // Generate query embedding based on mode
        let query_embedding = match mode {
            "hybrid" => match HybridEmbedder::new() {
                Ok(mut embedder) => match embedder.embed_query(&query) {
                    Ok(emb) => emb,
                    Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                },
                Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
            },
            "code" => match LocalEmbedder::with_speed_mode(SpeedMode::Code) {
                Ok(mut embedder) => match embedder.embed_query(&query) {
                    Ok(emb) => emb,
                    Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                },
                Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
            },
            _ => {
                // balanced mode
                match LocalEmbedder::with_speed_mode(SpeedMode::Balanced) {
                    Ok(mut embedder) => match embedder.embed_query(&query) {
                        Ok(emb) => emb,
                        Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
                    },
                    Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
                }
            }
        };

        // Search
        let searcher = HybridSearcher::default();
        let results = searcher.search(
            &store,
            &query_embedding,
            &query,
            max_results,
            None,
            false,
            None,
        );

        if results.is_empty() {
            return ToolCallResult::success(format!(
                "No results found for query: '{}'\n\nTry:\n- Different search terms\n- Check if the directory is indexed",
                query
            ));
        }

        // Format results
        let mut output = format!("Found {} results for: '{}'\n\n", results.len(), query);

        for (i, result) in results.iter().enumerate() {
            let score_pct = (result.score * 100.0) as u32;
            output.push_str(&format!(
                "{}. {} ({}% match)\n",
                i + 1,
                result.chunk.file_path,
                score_pct
            ));
            output.push_str(&format!(
                "   Lines {}-{}\n",
                result.chunk.start_line, result.chunk.end_line
            ));

            if include_content {
                output.push_str("   ```\n");
                for line in result.chunk.content.lines().take(15) {
                    output.push_str(&format!("   {}\n", line));
                }
                if result.chunk.content.lines().count() > 15 {
                    output.push_str("   ...\n");
                }
                output.push_str("   ```\n");
            }
            output.push('\n');
        }

        ToolCallResult::success(output)
    }

    fn execute_index_directory(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::error("Missing required 'path' argument".to_string()),
        };

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        let speed_mode = match mode {
            "fast" => SpeedMode::Fast,
            "code" => SpeedMode::Code,
            _ => SpeedMode::Balanced,
        };

        // Run indexing synchronously (blocking)
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => return ToolCallResult::error(format!("Runtime error: {}", e)),
        };

        match rt.block_on(async {
            crate::commands::watch::sync_files(&path, None, speed_mode).await
        }) {
            Ok(_) => ToolCallResult::success(format!(
                "Successfully indexed directory: {}\n\nYou can now use semantic_search to find code.",
                path
            )),
            Err(e) => ToolCallResult::error(format!("Indexing failed: {}", e)),
        }
    }

    fn execute_get_codebase_map(&self, args: Option<Value>) -> ToolCallResult {
        let args = args.unwrap_or(json!({}));

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ".".to_string());

        let minimal = args
            .get("minimal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let root = match Path::new(&path).canonicalize() {
            Ok(p) => p,
            Err(e) => return ToolCallResult::error(format!("Invalid path: {}", e)),
        };

        // Load the codebase map
        match CodeMap::load(&root) {
            Ok(Some(map)) => {
                let overview = if minimal {
                    map.to_minimal_overview()
                } else {
                    map.to_compact_overview()
                };

                let stats = map.stats();
                let token_estimate = overview.len() / 4;

                let mut output = format!(
                    "# Codebase Map\n\n{} files, {} symbols (~{} tokens)\n\n",
                    stats.files, stats.symbols, token_estimate
                );
                output.push_str(&overview);

                ToolCallResult::success(output)
            }
            Ok(None) => {
                ToolCallResult::error(
                    "No codebase map found. Run 'searchgrep compile' first to generate a map of your codebase.".to_string()
                )
            }
            Err(e) => ToolCallResult::error(format!("Failed to load map: {}", e)),
        }
    }

    fn execute_search_symbols(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::error("Missing required 'query' argument".to_string()),
        };

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ".".to_string());

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        let root = match Path::new(&path).canonicalize() {
            Ok(p) => p,
            Err(e) => return ToolCallResult::error(format!("Invalid path: {}", e)),
        };

        match CodeMap::load(&root) {
            Ok(Some(map)) => {
                let results = map.search(&query);

                if results.is_empty() {
                    return ToolCallResult::success(format!(
                        "No symbols found matching '{}'\n\nTry:\n- Different search terms\n- Run 'searchgrep compile' to update the map",
                        query
                    ));
                }

                let mut output = format!(
                    "Found {} symbols matching '{}':\n\n",
                    results.len().min(max_results),
                    query
                );

                for (i, sym) in results.iter().take(max_results).enumerate() {
                    output.push_str(&format!(
                        "{}. [{}] {}\n   File: {}:{}\n   ID: {}\n",
                        i + 1,
                        sym.kind.as_str(),
                        sym.signature,
                        sym.file,
                        sym.line,
                        sym.id
                    ));
                    if !sym.summary.is_empty() {
                        output.push_str(&format!("   Summary: {}\n", sym.summary));
                    }
                    output.push('\n');
                }

                ToolCallResult::success(output)
            }
            Ok(None) => ToolCallResult::error(
                "No codebase map found. Run 'searchgrep compile' first.".to_string(),
            ),
            Err(e) => ToolCallResult::error(format!("Failed to load map: {}", e)),
        }
    }

    fn execute_expand_symbol(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let symbol_id = match args.get("symbol_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                return ToolCallResult::error("Missing required 'symbol_id' argument".to_string())
            }
        };

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ".".to_string());

        let include_code = args
            .get("include_code")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let root = match Path::new(&path).canonicalize() {
            Ok(p) => p,
            Err(e) => return ToolCallResult::error(format!("Invalid path: {}", e)),
        };

        match CodeMap::load(&root) {
            Ok(Some(map)) => {
                match map.expand(&symbol_id) {
                    Some(expanded) => {
                        let sym = expanded.symbol;
                        let mut output = format!(
                            "# {} {}\n\nFile: {}:{}\nKind: {}\nSignature: {}\n",
                            sym.kind.as_str(),
                            sym.name,
                            sym.file,
                            sym.line,
                            sym.kind.as_str(),
                            sym.signature
                        );

                        if !sym.summary.is_empty() {
                            output.push_str(&format!("Summary: {}\n", sym.summary));
                        }

                        if !expanded.dependencies.is_empty() {
                            output.push_str("\n## Dependencies (calls/uses):\n");
                            for dep in &expanded.dependencies {
                                output.push_str(&format!("  - {} ({})\n", dep.name, dep.file));
                            }
                        }

                        if !expanded.dependents.is_empty() {
                            output.push_str("\n## Dependents (called by):\n");
                            for dep in &expanded.dependents {
                                output.push_str(&format!("  - {} ({})\n", dep.name, dep.file));
                            }
                        }

                        // Include source code if requested
                        if include_code {
                            let file_path = root.join(&sym.file);
                            if file_path.exists() {
                                if let Ok(content) = fs::read_to_string(&file_path) {
                                    let lines: Vec<&str> = content.lines().collect();
                                    let start = sym.line.saturating_sub(1);
                                    let end = (start + 30).min(lines.len()); // Max 30 lines

                                    output.push_str("\n## Source Code:\n```\n");
                                    for (i, line) in lines[start..end].iter().enumerate() {
                                        output.push_str(&format!(
                                            "{:4} | {}\n",
                                            start + i + 1,
                                            line
                                        ));
                                    }
                                    if end < lines.len() {
                                        output.push_str("     | ...\n");
                                    }
                                    output.push_str("```\n");
                                }
                            }
                        }

                        ToolCallResult::success(output)
                    }
                    None => ToolCallResult::error(format!(
                        "Symbol '{}' not found. Use search_symbols to find valid symbol IDs.",
                        symbol_id
                    )),
                }
            }
            Ok(None) => ToolCallResult::error(
                "No codebase map found. Run 'searchgrep compile' first.".to_string(),
            ),
            Err(e) => ToolCallResult::error(format!("Failed to load map: {}", e)),
        }
    }

    fn execute_find_similar_code(&self, args: Option<Value>) -> ToolCallResult {
        let args = args.unwrap_or(json!({}));

        let code = args.get("code").and_then(|v| v.as_str());
        let file = args.get("file").and_then(|v| v.as_str());

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let query_text = match (code, file) {
            (Some(c), _) => c.to_string(),
            (_, Some(f)) => match fs::read_to_string(f) {
                Ok(content) => content,
                Err(e) => return ToolCallResult::error(format!("Failed to read file: {}", e)),
            },
            (None, None) => {
                return ToolCallResult::error(
                    "Either 'code' or 'file' argument is required".to_string(),
                );
            }
        };

        // Load the vector store
        let store = match VectorStore::load(None) {
            Ok(s) => s,
            Err(e) => return ToolCallResult::error(format!("Failed to load index: {}", e)),
        };

        if store.chunk_count() == 0 {
            return ToolCallResult::error(
                "No files indexed. Run 'searchgrep watch <path>' first.".to_string(),
            );
        }

        // Generate embedding for the code
        let query_embedding = match LocalEmbedder::with_speed_mode(SpeedMode::Code) {
            Ok(mut embedder) => match embedder.embed_query(&query_text) {
                Ok(emb) => emb,
                Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
            },
            Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
        };

        // Search for similar code
        let searcher = HybridSearcher::default();
        let results = searcher.search(
            &store,
            &query_embedding,
            &query_text,
            max_results,
            None,
            false,
            None,
        );

        if results.is_empty() {
            return ToolCallResult::success("No similar code found.".to_string());
        }

        let mut output = format!("Found {} similar code snippets:\n\n", results.len());

        for (i, result) in results.iter().enumerate() {
            let score_pct = (result.score * 100.0) as u32;
            output.push_str(&format!(
                "{}. {} ({}% similar)\n",
                i + 1,
                result.chunk.file_path,
                score_pct
            ));
            output.push_str(&format!(
                "   Lines {}-{}\n",
                result.chunk.start_line, result.chunk.end_line
            ));
            output.push_str("   ```\n");
            for line in result.chunk.content.lines().take(10) {
                output.push_str(&format!("   {}\n", line));
            }
            if result.chunk.content.lines().count() > 10 {
                output.push_str("   ...\n");
            }
            output.push_str("   ```\n\n");
        }

        ToolCallResult::success(output)
    }

    fn execute_ask_codebase(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let question = match args.get("question").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => {
                return ToolCallResult::error("Missing required 'question' argument".to_string())
            }
        };

        let max_context = args
            .get("max_context")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // Load the vector store
        let store = match VectorStore::load(None) {
            Ok(s) => s,
            Err(e) => return ToolCallResult::error(format!("Failed to load index: {}", e)),
        };

        if store.chunk_count() == 0 {
            return ToolCallResult::error(
                "No files indexed. Run 'searchgrep watch <path>' first.".to_string(),
            );
        }

        // Generate embedding for the question
        let query_embedding = match LocalEmbedder::with_speed_mode(SpeedMode::Balanced) {
            Ok(mut embedder) => match embedder.embed_query(&question) {
                Ok(emb) => emb,
                Err(e) => return ToolCallResult::error(format!("Embedding failed: {}", e)),
            },
            Err(e) => return ToolCallResult::error(format!("Model load failed: {}", e)),
        };

        // Search for relevant context
        let searcher = HybridSearcher::default();
        let results = searcher.search(
            &store,
            &query_embedding,
            &question,
            max_context,
            None,
            false,
            None,
        );

        if results.is_empty() {
            return ToolCallResult::success(format!(
                "No relevant code found for: '{}'\n\nThe codebase may not contain information about this topic.",
                question
            ));
        }

        // Build context from results
        let mut output = format!("# Question: {}\n\n", question);
        output.push_str("## Relevant Code Context:\n\n");

        for (i, result) in results.iter().enumerate() {
            let score_pct = (result.score * 100.0) as u32;
            output.push_str(&format!(
                "### {}. {} ({}% relevant)\n",
                i + 1,
                result.chunk.file_path,
                score_pct
            ));
            output.push_str(&format!(
                "Lines {}-{}:\n",
                result.chunk.start_line, result.chunk.end_line
            ));
            output.push_str("```\n");
            output.push_str(&result.chunk.content);
            if !result.chunk.content.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("```\n\n");
        }

        output.push_str("## Summary:\n");
        output.push_str(&format!(
            "Found {} relevant code sections that may help answer: '{}'\n",
            results.len(),
            question
        ));
        output.push_str("Review the code above to understand the implementation.");

        ToolCallResult::success(output)
    }

    fn execute_get_file_context(&self, args: Option<Value>) -> ToolCallResult {
        let args = match args {
            Some(a) => a,
            None => return ToolCallResult::error("Missing arguments".to_string()),
        };

        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => {
                return ToolCallResult::error("Missing required 'file_path' argument".to_string())
            }
        };

        let include_content = args
            .get("include_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = Path::new(&file_path);
        if !path.exists() {
            return ToolCallResult::error(format!("File not found: {}", file_path));
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return ToolCallResult::error(format!("Failed to read file: {}", e)),
        };

        let lines = content.lines().count();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        let mut output = format!("# File: {}\n\n", file_path);
        output.push_str(&format!("- **Lines**: {}\n", lines));
        output.push_str(&format!("- **Extension**: .{}\n", ext));

        // Try to get symbol information from codemap
        if let Ok(Some(map)) = CodeMap::load(Path::new(".")) {
            let symbols: Vec<_> = map
                .search(&file_path)
                .into_iter()
                .filter(|s| s.file == file_path || file_path.ends_with(&s.file))
                .collect();

            if !symbols.is_empty() {
                output.push_str(&format!("- **Symbols**: {}\n\n", symbols.len()));
                output.push_str("## Symbols in this file:\n\n");
                for sym in symbols.iter().take(20) {
                    output.push_str(&format!(
                        "- [{}] {} (line {})\n",
                        sym.kind.as_str(),
                        sym.signature,
                        sym.line
                    ));
                }
                if symbols.len() > 20 {
                    output.push_str(&format!("  ... and {} more\n", symbols.len() - 20));
                }
            }
        }

        // Detect imports/dependencies
        let imports: Vec<&str> = content
            .lines()
            .filter(|l| {
                l.trim().starts_with("use ")
                    || l.trim().starts_with("import ")
                    || l.trim().starts_with("from ")
                    || l.trim().starts_with("require(")
                    || l.trim().starts_with("#include")
            })
            .take(20)
            .collect();

        if !imports.is_empty() {
            output.push_str("\n## Imports/Dependencies:\n\n");
            for imp in &imports {
                output.push_str(&format!("- {}\n", imp.trim()));
            }
        }

        if include_content {
            output.push_str("\n## Content:\n\n```");
            output.push_str(ext);
            output.push('\n');
            output.push_str(&content);
            output.push_str("\n```\n");
        }

        ToolCallResult::success(output)
    }

    fn execute_list_indexed_files(&self, args: Option<Value>) -> ToolCallResult {
        let args = args.unwrap_or(json!({}));
        let pattern = args.get("pattern").and_then(|v| v.as_str());

        // Load the vector store
        let store = match VectorStore::load(None) {
            Ok(s) => s,
            Err(e) => return ToolCallResult::error(format!("Failed to load index: {}", e)),
        };

        if store.chunk_count() == 0 {
            return ToolCallResult::error(
                "No files indexed. Run 'searchgrep watch <path>' first.".to_string(),
            );
        }

        let files = store.list_files();

        let filtered: Vec<&String> = if let Some(pat) = pattern {
            files
                .iter()
                .filter(|f| {
                    // Simple glob matching
                    if pat.contains('*') {
                        let parts: Vec<&str> = pat.split('*').collect();
                        if parts.len() == 2 {
                            let (prefix, suffix) = (parts[0], parts[1]);
                            f.starts_with(prefix) && f.ends_with(suffix)
                        } else {
                            f.contains(&pat.replace('*', ""))
                        }
                    } else {
                        f.contains(pat)
                    }
                })
                .collect()
        } else {
            files.iter().collect()
        };

        let mut output = format!("# Indexed Files ({})\n\n", filtered.len());

        // Group by directory
        let mut by_dir: std::collections::HashMap<String, Vec<&String>> =
            std::collections::HashMap::new();
        for file in &filtered {
            let dir = Path::new(file)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".")
                .to_string();
            by_dir.entry(dir).or_default().push(file);
        }

        let mut dirs: Vec<_> = by_dir.keys().collect();
        dirs.sort();

        for dir in dirs {
            let files = by_dir.get(dir).unwrap();
            output.push_str(&format!("## {}/\n", dir));
            for file in files {
                let name = Path::new(file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file);
                output.push_str(&format!("  - {}\n", name));
            }
            output.push('\n');
        }

        ToolCallResult::success(output)
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
