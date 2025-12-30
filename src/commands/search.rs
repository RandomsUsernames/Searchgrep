use anyhow::{Context, Result};
use colored::Colorize;
use std::time::Instant;

use crate::core::config::Config;
use crate::core::embeddings::EmbeddingProvider;
use crate::core::hybrid_embedder::HybridEmbedder;
use crate::core::local_embeddings::{LocalEmbedder, SpeedMode};
use crate::core::reranker::{simple_rerank, Reranker};
use crate::core::search::{HybridSearcher, SearchResult};
use crate::core::store::VectorStore;
use crate::ui::progress::SearchAnimation;
use crate::ui::search_display;

pub struct SearchOptions {
    pub pattern: String,
    pub path: Option<String>,
    pub max_count: usize,
    pub content: bool,
    pub answer: bool,
    pub sync: bool,
    pub rerank: bool,
    pub colbert: bool,
    pub file_types: Option<Vec<String>>,
    pub store: Option<String>,
    pub code: bool,
    pub hybrid: bool,
    pub json: bool,
}

pub async fn run(options: SearchOptions) -> Result<()> {
    let start_time = Instant::now();
    let config = Config::load()?;
    let store = VectorStore::load(options.store.as_deref())?;

    if store.chunk_count() == 0 {
        if options.json {
            println!(
                "{}",
                serde_json::json!({
                    "error": "No files indexed",
                    "results": []
                })
            );
        } else {
            println!("{}", "No files indexed yet. Run:".yellow());
            println!("  searchgrep watch [path]");
        }
        return Ok(());
    }

    // Determine speed mode
    let speed_mode = if options.code {
        SpeedMode::Code
    } else {
        SpeedMode::default()
    };

    // Sync if requested
    if options.sync {
        let path = options.path.clone().unwrap_or_else(|| ".".to_string());
        crate::commands::watch::sync_files(&path, options.store.as_deref(), speed_mode.clone())
            .await?;
    }

    // Start search animation (skip if JSON output)
    let animation = if !options.json {
        let anim = SearchAnimation::new(&options.pattern);
        anim.start();
        Some(anim)
    } else {
        None
    };

    // Generate query embedding based on mode
    if let Some(ref anim) = animation {
        anim.update_stage("Generating embeddings...");
    }

    let (query_embedding, query_tokens) = if options.hybrid {
        // Use hybrid fusion model (BGE + CodeRankEmbed)
        let mut embedder = HybridEmbedder::new()?;
        let embedding = embedder.embed_query(&options.pattern)?;
        (embedding, vec![])
    } else if options.code {
        // Use local CodeRankEmbed for code search
        let mut embedder = LocalEmbedder::with_speed_mode(SpeedMode::Code)?;
        let embedding = embedder.embed_query(&options.pattern)?;
        (embedding, vec![])
    } else if options.colbert {
        let embeddings = EmbeddingProvider::new(config.clone());
        embeddings.embed_with_tokens(&options.pattern).await?
    } else {
        let embeddings = EmbeddingProvider::new(config.clone());
        (embeddings.embed_single(&options.pattern).await?, vec![])
    };

    // Search
    if let Some(ref anim) = animation {
        anim.update_stage("Searching index...");
    }
    let searcher = HybridSearcher::default();
    let file_types = options.file_types.as_ref().map(|v| v.as_slice());

    let mut results = searcher.search(
        &store,
        &query_embedding,
        &options.pattern,
        options.max_count * 3, // Get more for reranking
        file_types,
        options.colbert,
        if options.colbert {
            Some(&query_tokens)
        } else {
            None
        },
    );

    if results.is_empty() {
        if let Some(ref anim) = animation {
            anim.finish(0, start_time.elapsed().as_millis());
        }
        if options.json {
            println!(
                "{}",
                serde_json::json!({
                    "query": options.pattern,
                    "results": [],
                    "duration_ms": start_time.elapsed().as_millis()
                })
            );
        } else {
            println!("{}", "No results found".yellow());
        }
        return Ok(());
    }

    // Rerank if enabled
    if options.rerank && results.len() > 1 {
        if let Some(ref anim) = animation {
            anim.update_stage("Reranking results...");
        }
        let reranker = Reranker::new(config.clone());
        let results_clone = results.clone();
        results = match reranker
            .rerank(&options.pattern, results_clone, options.max_count)
            .await
        {
            Ok(reranked) => reranked,
            Err(_) => {
                // Fall back to simple reranking
                simple_rerank(&options.pattern, results)
                    .into_iter()
                    .take(options.max_count)
                    .collect()
            }
        };
    } else {
        results.truncate(options.max_count);
    }

    // Finish animation
    let duration = start_time.elapsed().as_millis();
    if let Some(ref anim) = animation {
        anim.finish(results.len(), duration);
    }

    // Display results
    if options.json {
        // Output as JSON
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "file": r.chunk.file_path,
                    "start_line": r.chunk.start_line,
                    "end_line": r.chunk.end_line,
                    "score": r.score,
                    "content": if options.content { Some(&r.chunk.content) } else { None }
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "query": options.pattern,
                "results": json_results,
                "count": results.len(),
                "duration_ms": duration
            })
        );
    } else if options.answer {
        display_answer(&options.pattern, &results, &config).await?;
    } else {
        // Use the beautiful new UI
        search_display::display_results(&options.pattern, &results, options.content);
    }

    Ok(())
}

async fn display_answer(query: &str, results: &[SearchResult], config: &Config) -> Result<()> {
    let api_key = config
        .get_api_key()
        .context("No API key for answer generation")?;
    let base_url = config.get_base_url();

    // Build context from results
    let context: String = results
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, r)| {
            format!(
                "--- File: {} (lines {}-{}) ---\n{}\n",
                r.chunk.file_path, r.chunk.start_line, r.chunk.end_line, r.chunk.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Based on the following code context, answer this question: {}\n\nContext:\n{}",
        query, context
    );

    println!("{}", "Generating answer...".dimmed());
    println!();

    // Call OpenAI chat API
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful code assistant. Answer questions about code concisely and accurately. Reference specific files and line numbers when relevant."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 1000
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        println!("{} API error {}: {}", "Error:".red(), status, text);
        return Ok(());
    }

    let result: serde_json::Value = response.json().await?;

    if let Some(content) = result["choices"][0]["message"]["content"].as_str() {
        println!("{}", content);
    }

    println!();
    println!("{}", "Sources:".dimmed());
    for result in results.iter().take(5) {
        println!(
            "  {} {}:{}-{}",
            "•".dimmed(),
            result.chunk.file_path.cyan(),
            result.chunk.start_line,
            result.chunk.end_line
        );
    }

    Ok(())
}
