//! Codebase Map - Compact semantic representation for LLMs
//!
//! Instead of LLMs reading entire files, they query a pre-computed map:
//! - Symbol table (functions, types, exports)
//! - One-line semantic summaries
//! - Dependency graph
//! - Embeddings for search
//!
//! Result: 90%+ token reduction for LLM code understanding

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A symbol in the codebase (function, struct, type, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Unique identifier: file:name
    pub id: String,
    /// Symbol name
    pub name: String,
    /// File path
    pub file: String,
    /// Line number
    pub line: usize,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Signature (for functions: params + return type)
    pub signature: String,
    /// One-line semantic summary (auto-generated)
    pub summary: String,
    /// Symbols this depends on (calls, uses)
    pub depends_on: Vec<String>,
    /// Symbols that depend on this
    pub depended_by: Vec<String>,
    /// Embedding for semantic search
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Interface,
    Class,
    Type,
    Const,
    Module,
    Export,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Interface => "interface",
            SymbolKind::Class => "class",
            SymbolKind::Type => "type",
            SymbolKind::Const => "const",
            SymbolKind::Module => "mod",
            SymbolKind::Export => "export",
        }
    }
}

/// File summary in the codebase map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub language: String,
    pub symbols: Vec<String>, // Symbol IDs in this file
    pub imports: Vec<String>, // External dependencies
    pub exports: Vec<String>, // Exported symbols
    pub summary: String,      // One-line file description
    pub lines: usize,
}

/// The complete codebase map
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeMap {
    /// Project root
    pub root: String,
    /// All symbols indexed by ID
    pub symbols: HashMap<String, Symbol>,
    /// Files indexed by path
    pub files: HashMap<String, FileSummary>,
    /// Module/package structure
    pub modules: HashMap<String, Vec<String>>, // module -> files
    /// Global dependency graph edges
    pub edges: Vec<(String, String)>, // (from_symbol, to_symbol)
    /// Version for cache invalidation
    pub version: u64,
}

impl CodeMap {
    pub fn new(root: &str) -> Self {
        Self {
            root: root.to_string(),
            symbols: HashMap::new(),
            files: HashMap::new(),
            modules: HashMap::new(),
            edges: Vec::new(),
            version: 1,
        }
    }

    /// Get map storage path
    pub fn map_path(root: &Path) -> PathBuf {
        root.join(".searchgrep").join("map.json")
    }

    /// Load existing map
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = Self::map_path(root);
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let map: CodeMap = serde_json::from_str(&content)?;
            Ok(Some(map))
        } else {
            Ok(None)
        }
    }

    /// Save map to disk
    pub fn save(&self, root: &Path) -> Result<()> {
        let dir = root.join(".searchgrep");
        fs::create_dir_all(&dir)?;
        let path = Self::map_path(root);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Add a symbol
    pub fn add_symbol(&mut self, symbol: Symbol) {
        let id = symbol.id.clone();
        let file = symbol.file.clone();

        // Add to file's symbol list
        if let Some(file_summary) = self.files.get_mut(&file) {
            if !file_summary.symbols.contains(&id) {
                file_summary.symbols.push(id.clone());
            }
        }

        self.symbols.insert(id, symbol);
    }

    /// Add a file summary
    pub fn add_file(&mut self, file: FileSummary) {
        self.files.insert(file.path.clone(), file);
    }

    /// Generate compact overview for LLM (minimal tokens)
    pub fn to_compact_overview(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "# {} ({} files, {} symbols)\n\n",
            self.root,
            self.files.len(),
            self.symbols.len()
        ));

        // Group by file
        for (path, file) in &self.files {
            output.push_str(&format!("## {}\n", path));
            if !file.summary.is_empty() {
                output.push_str(&format!("{}\n", file.summary));
            }

            // List symbols compactly
            for sym_id in &file.symbols {
                if let Some(sym) = self.symbols.get(sym_id) {
                    output.push_str(&format!(
                        "  {} {} {}\n",
                        sym.kind.as_str(),
                        sym.signature,
                        if sym.summary.is_empty() {
                            "".to_string()
                        } else {
                            format!("// {}", sym.summary)
                        }
                    ));
                }
            }
            output.push('\n');
        }

        output
    }

    /// Generate ultra-compact overview (just signatures)
    pub fn to_minimal_overview(&self) -> String {
        let mut output = String::new();

        for (path, file) in &self.files {
            let short_path = path.split('/').last().unwrap_or(path);
            output.push_str(&format!("{}: ", short_path));

            let sigs: Vec<String> = file
                .symbols
                .iter()
                .filter_map(|id| self.symbols.get(id))
                .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
                .map(|s| s.name.clone())
                .collect();

            output.push_str(&sigs.join(", "));
            output.push('\n');
        }

        output
    }

    /// Search symbols by query
    pub fn search(&self, query: &str) -> Vec<&Symbol> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&Symbol> = self
            .symbols
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.signature.to_lowercase().contains(&query_lower)
                    || s.summary.to_lowercase().contains(&query_lower)
            })
            .collect();

        // Sort by relevance (exact name match first)
        results.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;
            b_exact.cmp(&a_exact)
        });

        results
    }

    /// Get symbol with its dependencies
    pub fn expand(&self, symbol_id: &str) -> Option<ExpandedSymbol> {
        let symbol = self.symbols.get(symbol_id)?;

        let dependencies: Vec<&Symbol> = symbol
            .depends_on
            .iter()
            .filter_map(|id| self.symbols.get(id))
            .collect();

        let dependents: Vec<&Symbol> = symbol
            .depended_by
            .iter()
            .filter_map(|id| self.symbols.get(id))
            .collect();

        Some(ExpandedSymbol {
            symbol,
            dependencies,
            dependents,
        })
    }

    /// Get stats
    pub fn stats(&self) -> CodeMapStats {
        let mut functions = 0;
        let mut structs = 0;
        let mut other = 0;

        for sym in self.symbols.values() {
            match sym.kind {
                SymbolKind::Function | SymbolKind::Method => functions += 1,
                SymbolKind::Struct | SymbolKind::Class => structs += 1,
                _ => other += 1,
            }
        }

        CodeMapStats {
            files: self.files.len(),
            symbols: self.symbols.len(),
            functions,
            structs,
            other,
            edges: self.edges.len(),
        }
    }
}

pub struct ExpandedSymbol<'a> {
    pub symbol: &'a Symbol,
    pub dependencies: Vec<&'a Symbol>,
    pub dependents: Vec<&'a Symbol>,
}

#[derive(Debug)]
pub struct CodeMapStats {
    pub files: usize,
    pub symbols: usize,
    pub functions: usize,
    pub structs: usize,
    pub other: usize,
    pub edges: usize,
}
