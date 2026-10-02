//! RAG Pipeline for ScreenBuddy
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk { pub id: String, pub text: String, pub source: String, pub embedding: Option<Vec<f32>> }

pub trait VectorStore: Send + Sync {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String>;
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)>;
}

pub struct InMemoryVectorStore { chunks: Vec<Chunk> }
impl InMemoryVectorStore { pub fn new() -> Self { Self { chunks: Vec::new() } } }
impl VectorStore for InMemoryVectorStore {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String> { self.chunks.push(chunk); Ok(()) }
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)> {
        let mut r: Vec<(f32, Chunk)> = self.chunks.iter().filter_map(|c| c.embedding.as_ref().map(|e| (cosine(query, e), c.clone()))).collect();
        r.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        r.truncate(top_k); r
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x,y)| x*y).sum();
    let na: f32 = a.iter().map(|x| x*x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x*x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { d / (na * nb) }
}

pub struct TfIdfEmbedding;
impl TfIdfEmbedding {
    pub fn embed(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; 256];
        for t in text.split_whitespace() { v[simple_hash(t) % 256] += 1.0; }
        let n: f32 = v.iter().map(|x| x*x).sum::<f32>().sqrt();
        if n > 0.0 { for x in &mut v { *x /= n; } } v
    }
}

fn simple_hash(s: &str) -> usize { let mut h = 5381usize; for c in s.bytes() { h = ((h << 5).wrapping_add(h)).wrapping_add(c as usize); } h }

pub struct RagPipeline { store: Box<dyn VectorStore>, chunk_size: usize }
impl RagPipeline {
    pub fn new() -> Self { Self { store: Box::new(InMemoryVectorStore::new()), chunk_size: 512 } }
    pub fn ingest_document(&mut self, path: &Path, content: &str) -> Result<usize, String> {
        for (i, chunk) in content.chars().collect::<Vec<_>>().chunks(self.chunk_size).enumerate() {
            let text: String = chunk.iter().collect();
            self.store.insert(Chunk { id: format!("{}-{}", path.display(), i), text, source: path.to_string_lossy().to_string(), embedding: Some(TfIdfEmbedding::embed(&format!("",))) })?;
        }
        Ok(content.len() / self.chunk_size + 1)
    }
}
