use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::core::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub id: String,
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: String,
    pub language: Option<String>,
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub token_embeddings: Option<Vec<Vec<f32>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub path: String,
    pub hash: String,
    pub chunks: Vec<String>,
    pub indexed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStore {
    pub files: HashMap<String, IndexedFile>,
    pub chunks: HashMap<String, FileChunk>,
    #[serde(default)]
    pub bm25_idf: HashMap<String, f32>,
    #[serde(default)]
    pub doc_count: usize,
}

impl Default for VectorStore {
    fn default() -> Self {
        Self {
            files: HashMap::new(),
            chunks: HashMap::new(),
            bm25_idf: HashMap::new(),
            doc_count: 0,
        }
    }
}

impl VectorStore {
    /// Binary store path (fast loading)
    pub fn store_path_bin(store_name: Option<&str>) -> Result<PathBuf> {
        let config_dir = Config::config_dir()?;
        let name = store_name.unwrap_or("default");
        Ok(config_dir.join(format!("{}.store.bin", name)))
    }

    /// Legacy JSON store path
    pub fn store_path(store_name: Option<&str>) -> Result<PathBuf> {
        let config_dir = Config::config_dir()?;
        let name = store_name.unwrap_or("default");
        Ok(config_dir.join(format!("{}.store.json", name)))
    }

    /// Load store - prefers binary format, falls back to JSON
    pub fn load(store_name: Option<&str>) -> Result<Self> {
        let bin_path = Self::store_path_bin(store_name)?;
        let json_path = Self::store_path(store_name)?;

        // Try binary format first (fast)
        if bin_path.exists() {
            let data = fs::read(&bin_path)?;
            let store: VectorStore =
                bincode::deserialize(&data).context("Failed to deserialize binary store")?;
            return Ok(store);
        }

        // Fall back to JSON (legacy)
        if json_path.exists() {
            let content = fs::read_to_string(&json_path)?;
            let store: VectorStore = serde_json::from_str(&content)?;
            return Ok(store);
        }

        Ok(VectorStore::default())
    }

    /// Save store in binary format (fast)
    pub fn save(&self, store_name: Option<&str>) -> Result<()> {
        let bin_path = Self::store_path_bin(store_name)?;
        let data = bincode::serialize(self)?;
        fs::write(&bin_path, data)?;
        Ok(())
    }

    /// Save store in JSON format (for debugging/export)
    pub fn save_json(&self, store_name: Option<&str>) -> Result<()> {
        let path = Self::store_path(store_name)?;
        let content = serde_json::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Migrate from JSON to binary format
    pub fn migrate_to_binary(store_name: Option<&str>) -> Result<bool> {
        let json_path = Self::store_path(store_name)?;
        let bin_path = Self::store_path_bin(store_name)?;

        if json_path.exists() && !bin_path.exists() {
            let content = fs::read_to_string(&json_path)?;
            let store: VectorStore = serde_json::from_str(&content)?;
            store.save(store_name)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.chunks.clear();
        self.bm25_idf.clear();
        self.doc_count = 0;
    }

    pub fn add_file(&mut self, file: IndexedFile) {
        self.files.insert(file.path.clone(), file);
    }

    pub fn add_chunk(&mut self, chunk: FileChunk) {
        self.chunks.insert(chunk.id.clone(), chunk);
    }

    pub fn remove_file(&mut self, path: &str) {
        if let Some(file) = self.files.remove(path) {
            for chunk_id in file.chunks {
                self.chunks.remove(&chunk_id);
            }
        }
    }

    pub fn get_file(&self, path: &str) -> Option<&IndexedFile> {
        self.files.get(path)
    }

    pub fn file_needs_update(&self, path: &str, new_hash: &str) -> bool {
        match self.files.get(path) {
            Some(file) => file.hash != new_hash,
            None => true,
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn list_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    pub fn all_chunks(&self) -> impl Iterator<Item = &FileChunk> {
        self.chunks.values()
    }

    pub fn chunks_for_file(&self, path: &str) -> Vec<&FileChunk> {
        match self.files.get(path) {
            Some(file) => file
                .chunks
                .iter()
                .filter_map(|id| self.chunks.get(id))
                .collect(),
            None => vec![],
        }
    }

    pub fn update_bm25_stats(&mut self) {
        let mut term_doc_freq: HashMap<String, usize> = HashMap::new();
        let total_docs = self.chunks.len();

        for chunk in self.chunks.values() {
            let terms: std::collections::HashSet<String> = chunk
                .content
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            for term in terms {
                *term_doc_freq.entry(term).or_insert(0) += 1;
            }
        }

        self.bm25_idf.clear();
        for (term, doc_freq) in term_doc_freq {
            let idf =
                ((total_docs as f32 - doc_freq as f32 + 0.5) / (doc_freq as f32 + 0.5) + 1.0).ln();
            self.bm25_idf.insert(term, idf);
        }
        self.doc_count = total_docs;
    }
}

pub fn compute_file_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_chunk_id(file_path: &str, start_line: usize, end_line: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}:{}", file_path, start_line, end_line).as_bytes());
    hex::encode(&hasher.finalize()[..8])
}
