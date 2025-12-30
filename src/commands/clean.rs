use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::core::config::Config;

pub struct CleanOptions {
    pub list: bool,
    pub all: bool,
    pub store: Option<String>,
    pub json: bool,
}

pub async fn run(options: CleanOptions) -> Result<()> {
    let config_dir = Config::config_dir()?;

    if options.list {
        if options.json {
            list_indexes_json(&config_dir)?;
        } else {
            list_indexes(&config_dir)?;
        }
        return Ok(());
    }

    if options.all {
        clear_all_indexes(&config_dir)?;
        return Ok(());
    }

    if let Some(store_name) = options.store {
        clear_specific_index(&config_dir, &store_name)?;
        return Ok(());
    }

    // Default: show help
    println!("{}", "searchgrep clean".bold());
    println!();
    println!("Remove indexed data for privacy and storage management.");
    println!();
    println!("{}", "Usage:".bold());
    println!("  searchgrep clean --list          List all indexes and their sizes");
    println!("  searchgrep clean --all           Remove ALL indexes");
    println!("  searchgrep clean --store <name>  Remove a specific index");
    println!();
    println!("{}", "Examples:".bold());
    println!("  searchgrep clean --list");
    println!("  searchgrep clean --store default");
    println!("  searchgrep clean --all");
    println!();

    list_indexes(&config_dir)?;

    Ok(())
}

fn list_indexes(config_dir: &std::path::Path) -> Result<()> {
    println!("{}", "Stored indexes:".bold());
    println!();

    let mut found = false;
    let mut total_size: u64 = 0;

    if config_dir.exists() {
        for entry in fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".store.json") {
                    found = true;
                    let store_name = name.trim_end_matches(".store.json");
                    let metadata = fs::metadata(&path)?;
                    let size = metadata.len();
                    total_size += size;

                    let size_str = format_size(size);
                    println!(
                        "  {} {} {}",
                        "•".cyan(),
                        store_name.white(),
                        size_str.dimmed()
                    );
                }
            }
        }
    }

    if !found {
        println!("  {}", "(no indexes found)".dimmed());
    } else {
        println!();
        println!(
            "  {} {}",
            "Total:".dimmed(),
            format_size(total_size).white()
        );
    }

    println!();
    println!("{}", format!("Location: {}", config_dir.display()).dimmed());

    Ok(())
}

fn list_indexes_json(config_dir: &std::path::Path) -> Result<()> {
    let mut indexes: Vec<serde_json::Value> = Vec::new();
    let mut total_size: u64 = 0;

    if config_dir.exists() {
        for entry in fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".store.json") {
                    let store_name = name.trim_end_matches(".store.json");
                    let metadata = fs::metadata(&path)?;
                    let size = metadata.len();
                    total_size += size;

                    indexes.push(serde_json::json!({
                        "name": store_name,
                        "size_bytes": size,
                        "size_human": format_size(size),
                        "path": path.to_string_lossy()
                    }));
                }
            }
        }
    }

    println!(
        "{}",
        serde_json::json!({
            "indexes": indexes,
            "total_size_bytes": total_size,
            "total_size_human": format_size(total_size),
            "config_dir": config_dir.to_string_lossy()
        })
    );

    Ok(())
}

fn clear_all_indexes(config_dir: &std::path::Path) -> Result<()> {
    let mut count = 0;
    let mut total_size: u64 = 0;

    if config_dir.exists() {
        for entry in fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".store.json") {
                    let metadata = fs::metadata(&path)?;
                    total_size += metadata.len();
                    fs::remove_file(&path)?;
                    count += 1;
                }
            }
        }
    }

    if count > 0 {
        println!(
            "{} Removed {} index{} (freed {})",
            "✓".green(),
            count,
            if count == 1 { "" } else { "es" },
            format_size(total_size)
        );
    } else {
        println!("{}", "No indexes to remove.".dimmed());
    }

    Ok(())
}

fn clear_specific_index(config_dir: &std::path::Path, store_name: &str) -> Result<()> {
    let store_path = config_dir.join(format!("{}.store.json", store_name));

    if store_path.exists() {
        let metadata = fs::metadata(&store_path)?;
        let size = metadata.len();
        fs::remove_file(&store_path)?;
        println!(
            "{} Removed index '{}' (freed {})",
            "✓".green(),
            store_name,
            format_size(size)
        );
    } else {
        println!("{} Index '{}' not found", "✗".red(), store_name);
        println!();
        list_indexes(config_dir)?;
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
