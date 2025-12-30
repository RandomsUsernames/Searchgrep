use anyhow::Result;
use colored::Colorize;

use crate::core::store::VectorStore;

pub struct StatusOptions {
    pub store: Option<String>,
    pub files: bool,
    pub json: bool,
}

pub async fn run(options: StatusOptions) -> Result<()> {
    let store = VectorStore::load(options.store.as_deref())?;

    let file_count = store.file_count();
    let chunk_count = store.chunk_count();

    // Calculate embedding info
    let (embedding_dim, embedding_size_mb) = if let Some(chunk) = store.chunks.values().next() {
        let dim = chunk.embedding.len();
        let total_size = chunk_count * dim * 4; // 4 bytes per f32
        (dim, total_size as f64 / (1024.0 * 1024.0))
    } else {
        (0, 0.0)
    };

    if options.json {
        let mut json_output = serde_json::json!({
            "files_indexed": file_count,
            "total_chunks": chunk_count,
            "bm25_terms": store.bm25_idf.len(),
            "embedding_dimension": embedding_dim,
            "embedding_size_mb": embedding_size_mb
        });

        if options.files {
            let mut files: Vec<_> = store.files.values().collect();
            files.sort_by(|a, b| a.path.cmp(&b.path));

            let file_list: Vec<serde_json::Value> = files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "path": f.path,
                        "chunks": f.chunks.len(),
                        "indexed_at": f.indexed_at
                    })
                })
                .collect();

            json_output["files"] = serde_json::json!(file_list);
        }

        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    println!("{}", "searchgrep index status".bold());
    println!();

    println!("  {} {}", "Files indexed:".dimmed(), file_count);
    println!("  {} {}", "Total chunks:".dimmed(), chunk_count);

    if store.doc_count > 0 {
        println!("  {} {}", "BM25 terms:".dimmed(), store.bm25_idf.len());
    }

    if embedding_dim > 0 {
        println!(
            "  {} {} ({}D)",
            "Embedding size:".dimmed(),
            format!("{:.2} MB", embedding_size_mb),
            embedding_dim
        );
    }

    if options.files {
        println!();
        println!("{}", "Indexed files:".bold());

        let mut files: Vec<_> = store.files.values().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        for file in files {
            let chunk_count = file.chunks.len();
            println!(
                "  {} {} {}",
                file.path,
                format!("({} chunks)", chunk_count).dimmed(),
                file.indexed_at.dimmed()
            );
        }
    }

    if file_count == 0 {
        println!();
        println!("{}", "No files indexed yet. Run:".yellow());
        println!("  searchgrep watch [path]");
    }

    Ok(())
}
