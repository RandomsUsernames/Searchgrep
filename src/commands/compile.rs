//! Compile command - generates codebase map for LLM consumption

use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::core::codemap::{CodeMap, FileSummary};
use crate::core::parser::SymbolParser;
use crate::core::scanner::FileScanner;

pub struct CompileOptions {
    pub path: Option<String>,
    pub show: bool,
    pub minimal: bool,
}

pub async fn run(options: CompileOptions) -> Result<()> {
    let start = Instant::now();
    let path = options.path.unwrap_or_else(|| ".".to_string());
    let root = Path::new(&path).canonicalize()?;

    if options.show {
        return show_map(&root, options.minimal);
    }

    println!("{}", "Compiling codebase map...".cyan());
    println!();

    let parser = SymbolParser::new()?;
    let scanner = FileScanner::new(&root.to_string_lossy());
    let files = scanner.scan()?;

    let mut map = CodeMap::new(&root.to_string_lossy());
    let mut file_count = 0;
    let mut symbol_count = 0;

    for file in &files {
        let file_path = Path::new(&file.path);
        let content = fs::read_to_string(file_path)?;

        let parsed = parser.parse_file(file_path, &content)?;

        if parsed.symbols.is_empty() {
            continue;
        }

        file_count += 1;
        symbol_count += parsed.symbols.len();

        // Add file summary
        let relative_path = file_path
            .strip_prefix(&root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let symbol_ids: Vec<String> = parsed.symbols.iter().map(|s| s.id.clone()).collect();

        let exports: Vec<String> = parsed
            .symbols
            .iter()
            .filter(|s| s.signature.contains("pub ") || s.signature.starts_with("export"))
            .map(|s| s.name.clone())
            .collect();

        map.add_file(FileSummary {
            path: relative_path.clone(),
            language: parsed.language,
            symbols: symbol_ids,
            imports: parsed.imports,
            exports,
            summary: String::new(), // TODO: Generate with LLM
            lines: parsed.lines,
        });

        // Add symbols
        for mut symbol in parsed.symbols {
            // Update path to relative
            symbol.file = relative_path.clone();
            symbol.id = format!("{}:{}", relative_path, symbol.name);
            map.add_symbol(symbol);
        }

        // Progress indicator
        if file_count % 10 == 0 {
            print!("\r  {} files, {} symbols...", file_count, symbol_count);
        }
    }

    println!("\r                                          ");

    // Save map
    map.save(&root)?;

    let elapsed = start.elapsed();
    let stats = map.stats();

    println!("{} Compiled codebase map", "✓".green());
    println!();
    println!("  {} {} files", "•".cyan(), stats.files);
    println!("  {} {} symbols", "•".cyan(), stats.symbols);
    println!("    {} {} functions", "├".dimmed(), stats.functions);
    println!("    {} {} structs/classes", "├".dimmed(), stats.structs);
    println!("    {} {} other", "└".dimmed(), stats.other);
    println!();
    println!("  {} {:.1}s", "Time:".dimmed(), elapsed.as_secs_f32());
    println!(
        "  {} {}",
        "Saved:".dimmed(),
        CodeMap::map_path(&root).display()
    );
    println!();

    // Show compact overview
    let overview = map.to_minimal_overview();
    let token_estimate = overview.len() / 4; // Rough estimate
    println!(
        "  {} ~{} tokens (vs ~{}K reading all files)",
        "LLM cost:".dimmed(),
        token_estimate,
        (stats.files * 500) / 1000 // Assume 500 tokens per file average
    );

    Ok(())
}

fn show_map(root: &Path, minimal: bool) -> Result<()> {
    let map = CodeMap::load(root)?;

    match map {
        Some(m) => {
            if minimal {
                println!("{}", m.to_minimal_overview());
            } else {
                println!("{}", m.to_compact_overview());
            }
        }
        None => {
            println!("{} No codebase map found.", "✗".red());
            println!("  Run: {} to generate", "searchgrep compile".yellow());
        }
    }

    Ok(())
}
