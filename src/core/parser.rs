//! Language-aware symbol parser
//!
//! Extracts functions, structs, classes, etc. from source code
//! using regex-based parsing (fast, no external deps)

use anyhow::Result;
use regex::Regex;
use std::path::Path;

use super::codemap::{Symbol, SymbolKind};

pub struct SymbolParser {
    // Rust patterns
    rust_fn: Regex,
    rust_struct: Regex,
    rust_enum: Regex,
    rust_trait: Regex,
    rust_impl: Regex,
    rust_mod: Regex,
    rust_use: Regex,

    // TypeScript/JavaScript patterns
    ts_fn: Regex,
    ts_arrow: Regex,
    ts_class: Regex,
    ts_interface: Regex,
    ts_type: Regex,
    ts_export: Regex,
    ts_import: Regex,

    // Python patterns
    py_fn: Regex,
    py_class: Regex,
    py_import: Regex,

    // Go patterns
    go_fn: Regex,
    go_struct: Regex,
    go_interface: Regex,
}

impl SymbolParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            // Rust
            rust_fn: Regex::new(
                r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)(?:\s*->\s*([^\{]+))?\s*\{",
            )?,
            rust_struct: Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+(\w+)(?:<[^>]*>)?")?,
            rust_enum: Regex::new(r"(?m)^\s*(?:pub\s+)?enum\s+(\w+)(?:<[^>]*>)?")?,
            rust_trait: Regex::new(r"(?m)^\s*(?:pub\s+)?trait\s+(\w+)(?:<[^>]*>)?")?,
            rust_impl: Regex::new(r"(?m)^\s*impl(?:<[^>]*>)?\s+(?:(\w+)\s+for\s+)?(\w+)")?,
            rust_mod: Regex::new(r"(?m)^\s*(?:pub\s+)?mod\s+(\w+)")?,
            rust_use: Regex::new(r"(?m)^\s*use\s+([^;]+);")?,

            // TypeScript/JavaScript
            ts_fn: Regex::new(
                r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)(?:\s*:\s*([^\{]+))?\s*\{",
            )?,
            ts_arrow: Regex::new(
                r"(?m)^\s*(?:export\s+)?(?:const|let)\s+(\w+)\s*(?::\s*[^=]+)?\s*=\s*(?:async\s+)?\([^)]*\)\s*(?::\s*[^=]+)?\s*=>",
            )?,
            ts_class: Regex::new(
                r"(?m)^\s*(?:export\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?(?:\s+implements\s+([^{]+))?\s*\{",
            )?,
            ts_interface: Regex::new(
                r"(?m)^\s*(?:export\s+)?interface\s+(\w+)(?:<[^>]*>)?(?:\s+extends\s+([^{]+))?\s*\{",
            )?,
            ts_type: Regex::new(r"(?m)^\s*(?:export\s+)?type\s+(\w+)(?:<[^>]*>)?\s*=")?,
            ts_export: Regex::new(r"(?m)^\s*export\s+\{([^}]+)\}")?,
            ts_import: Regex::new(
                r#"(?m)^\s*import\s+(?:\{[^}]+\}|[^;]+)\s+from\s+['"]([^'"]+)['"]"#,
            )?,

            // Python
            py_fn: Regex::new(r"(?m)^(?:async\s+)?def\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^:]+))?:")?,
            py_class: Regex::new(r"(?m)^class\s+(\w+)(?:\(([^)]*)\))?:")?,
            py_import: Regex::new(r"(?m)^(?:from\s+(\S+)\s+)?import\s+(.+)$")?,

            // Go
            go_fn: Regex::new(
                r"(?m)^func\s+(?:\([^)]+\)\s+)?(\w+)\s*\(([^)]*)\)(?:\s*\(([^)]+)\)|\s*(\w+))?\s*\{",
            )?,
            go_struct: Regex::new(r"(?m)^type\s+(\w+)\s+struct\s*\{")?,
            go_interface: Regex::new(r"(?m)^type\s+(\w+)\s+interface\s*\{")?,
        })
    }

    /// Parse a file and extract symbols
    pub fn parse_file(&self, path: &Path, content: &str) -> Result<ParsedFile> {
        let lang = detect_language(path);
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        match lang.as_str() {
            "rust" => self.parse_rust(path, content, &lines, &mut symbols, &mut imports),
            "typescript" | "javascript" => {
                self.parse_typescript(path, content, &lines, &mut symbols, &mut imports)
            }
            "python" => self.parse_python(path, content, &lines, &mut symbols, &mut imports),
            "go" => self.parse_go(path, content, &lines, &mut symbols, &mut imports),
            _ => {} // Unsupported language
        }

        Ok(ParsedFile {
            path: path.to_string_lossy().to_string(),
            language: lang,
            symbols,
            imports,
            lines: lines.len(),
        })
    }

    fn parse_rust(
        &self,
        path: &Path,
        content: &str,
        lines: &[&str],
        symbols: &mut Vec<Symbol>,
        imports: &mut Vec<String>,
    ) {
        let file_path = path.to_string_lossy().to_string();

        // Functions
        for cap in self.rust_fn.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("()");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = format!("{}({}) -> {}", name, simplify_params(params), ret);

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Function,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Structs
        for cap in self.rust_struct.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Struct,
                signature: format!("struct {}", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Enums
        for cap in self.rust_enum.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Enum,
                signature: format!("enum {}", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Traits
        for cap in self.rust_trait.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Trait,
                signature: format!("trait {}", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Imports
        for cap in self.rust_use.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                imports.push(m.as_str().to_string());
            }
        }
    }

    fn parse_typescript(
        &self,
        path: &Path,
        content: &str,
        _lines: &[&str],
        symbols: &mut Vec<Symbol>,
        imports: &mut Vec<String>,
    ) {
        let file_path = path.to_string_lossy().to_string();

        // Functions
        for cap in self.ts_fn.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("void");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = format!("{}({}): {}", name, simplify_params(params), ret);

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Function,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Arrow functions
        for cap in self.ts_arrow.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Function,
                signature: format!("{} = () => ...", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Classes
        for cap in self.ts_class.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let extends = cap.get(2).map(|m| m.as_str());
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = if let Some(parent) = extends {
                format!("class {} extends {}", name, parent)
            } else {
                format!("class {}", name)
            };

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Class,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Interfaces
        for cap in self.ts_interface.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Interface,
                signature: format!("interface {}", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Types
        for cap in self.ts_type.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Type,
                signature: format!("type {}", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Imports
        for cap in self.ts_import.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                imports.push(m.as_str().to_string());
            }
        }
    }

    fn parse_python(
        &self,
        path: &Path,
        content: &str,
        _lines: &[&str],
        symbols: &mut Vec<Symbol>,
        imports: &mut Vec<String>,
    ) {
        let file_path = path.to_string_lossy().to_string();

        // Functions
        for cap in self.py_fn.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ret = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("None");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = format!("def {}({}) -> {}", name, simplify_params(params), ret);

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Function,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Classes
        for cap in self.py_class.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let bases = cap.get(2).map(|m| m.as_str());
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = if let Some(b) = bases {
                format!("class {}({})", name, b)
            } else {
                format!("class {}", name)
            };

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Class,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Imports
        for cap in self.py_import.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                imports.push(m.as_str().to_string());
            } else if let Some(m) = cap.get(2) {
                imports.push(m.as_str().to_string());
            }
        }
    }

    fn parse_go(
        &self,
        path: &Path,
        content: &str,
        _lines: &[&str],
        symbols: &mut Vec<Symbol>,
        _imports: &mut Vec<String>,
    ) {
        let file_path = path.to_string_lossy().to_string();

        // Functions
        for cap in self.go_fn.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ret = cap
                .get(3)
                .or(cap.get(4))
                .map(|m| m.as_str().trim())
                .unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            let signature = if ret.is_empty() {
                format!("func {}({})", name, simplify_params(params))
            } else {
                format!("func {}({}) {}", name, simplify_params(params), ret)
            };

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Function,
                signature,
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Structs
        for cap in self.go_struct.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Struct,
                signature: format!("type {} struct", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }

        // Interfaces
        for cap in self.go_interface.captures_iter(content) {
            let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let line = find_line_number(content, cap.get(0).unwrap().start());

            symbols.push(Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                file: file_path.clone(),
                line,
                kind: SymbolKind::Interface,
                signature: format!("type {} interface", name),
                summary: String::new(),
                depends_on: Vec::new(),
                depended_by: Vec::new(),
                embedding: Vec::new(),
            });
        }
    }
}

pub struct ParsedFile {
    pub path: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    pub lines: usize,
}

fn detect_language(path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "cpp" | "cc" | "cxx" | "hpp" | "h" => "cpp",
        "c" => "c",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "zig" => "zig",
        _ => "unknown",
    }
    .to_string()
}

fn find_line_number(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset].matches('\n').count() + 1
}

fn simplify_params(params: &str) -> String {
    // Simplify long parameter lists
    let params = params.trim();
    if params.len() > 50 {
        let parts: Vec<&str> = params.split(',').collect();
        if parts.len() > 3 {
            return format!("{}...", parts.len());
        }
    }
    params.to_string()
}
