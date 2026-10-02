//! RAG Pipeline for ScreenBuddy
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub text: String,
    pub source: String,
    pub embedding: Option<Vec<f32>>,
}

pub trait VectorStore: Send + Sync {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String>;
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)>;
    /// Number of stored chunks.
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct InMemoryVectorStore {
    chunks: Vec<Chunk>,
}
impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }
}
impl VectorStore for InMemoryVectorStore {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String> {
        self.chunks.push(chunk);
        Ok(())
    }
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)> {
        let mut r: Vec<(f32, Chunk)> = self
            .chunks
            .iter()
            .filter_map(|c| c.embedding.as_ref().map(|e| (cosine(query, e), c.clone())))
            .collect();
        r.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        r.truncate(top_k);
        r
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        d / (na * nb)
    }
}

pub struct TfIdfEmbedding;
impl TfIdfEmbedding {
    /// Hashed bag-of-words projection into a fixed 256-dimension vector.
    ///
    /// Dimension count must stay in sync with every stored embedding, since
    /// `cosine` zips the two slices.
    pub const DIM: usize = 256;

    pub fn embed(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; Self::DIM];
        for t in text.split_whitespace() {
            // Fold case so "Cat" and "cat" land in the same bucket.
            let normalized = t.to_lowercase();
            v[simple_hash(&normalized) % Self::DIM] += 1.0;
        }
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 {
            for x in &mut v {
                *x /= n;
            }
        }
        v
    }
}

fn simple_hash(s: &str) -> usize {
    let mut h = 5381usize;
    for c in s.bytes() {
        h = ((h << 5).wrapping_add(h)).wrapping_add(c as usize);
    }
    h
}

pub struct RagPipeline {
    store: Box<dyn VectorStore>,
    chunk_size: usize,
}
impl Default for RagPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl RagPipeline {
    pub fn new() -> Self {
        Self {
            store: Box::new(InMemoryVectorStore::new()),
            chunk_size: 512,
        }
    }

    /// Split on word boundaries so a chunk never cuts a word in half.
    fn split_chunks(content: &str, chunk_size: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = String::new();
        for word in content.split_whitespace() {
            // +1 for the space that will join this word to the previous one.
            if current.len() + word.len() + 1 > chunk_size && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            out.push(current);
        }
        out
    }

    pub fn ingest_document(&mut self, path: &Path, content: &str) -> Result<usize, String> {
        let pieces = Self::split_chunks(content, self.chunk_size);
        if pieces.is_empty() {
            return Ok(0);
        }
        for (i, text) in pieces.iter().enumerate() {
            self.store.insert(Chunk {
                id: format!("{}-{}", path.display(), i),
                text: text.clone(),
                source: path.to_string_lossy().to_string(),
                // Embed the chunk itself. Embedding an empty string made every
                // chunk identical, so search could not distinguish them.
                embedding: Some(TfIdfEmbedding::embed(text)),
            })?;
        }
        Ok(pieces.len())
    }

    /// Ingest from a string source identifier (e.g. `memory://note`).
    pub fn ingest_str(&mut self, source: &str, content: &str) -> Result<usize, String> {
        self.ingest_document(Path::new(source), content)
    }

    /// Rank stored chunks against [query], best first.
    ///
    /// Chunks with a non-positive score are dropped so callers are not handed
    /// unrelated text as if it were a match.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(f32, Chunk)> {
        if top_k == 0 || query.trim().is_empty() {
            return Vec::new();
        }
        let embedding = TfIdfEmbedding::embed(query);
        self.store
            .search(&embedding, top_k)
            .into_iter()
            .filter(|(score, _)| *score > 0.0)
            .collect()
    }

    /// Total chunks currently stored.
    pub fn chunk_count(&self) -> usize {
        self.store.len()
    }

    pub fn clear(&mut self) {
        self.store = Box::new(InMemoryVectorStore::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_is_normalised_and_fixed_width() {
        let v = TfIdfEmbedding::embed("hello world");
        assert_eq!(v.len(), TfIdfEmbedding::DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "expected unit norm, got {norm}");
    }

    #[test]
    fn embedding_folds_case() {
        // Regression: hashing was case-sensitive, so "Cat" and "cat" hashed apart.
        assert_eq!(TfIdfEmbedding::embed("Cat"), TfIdfEmbedding::embed("cat"));
    }

    #[test]
    fn empty_text_embeds_to_zero_vector() {
        let v = TfIdfEmbedding::embed("");
        assert_eq!(v.len(), TfIdfEmbedding::DIM);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn ingest_embeds_each_chunk_not_the_empty_string() {
        // Regression: every chunk was embedded from "", making them identical
        // and search unable to distinguish them.
        let mut rag = RagPipeline::new();
        rag.ingest_str("a://1", "cats purr loudly").unwrap();
        rag.ingest_str("a://2", "quantum chromodynamics describes the strong force")
            .unwrap();
        assert_eq!(rag.chunk_count(), 2);

        let hits = rag.search("cats purr", 5);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].1.text, "cats purr loudly");
        assert_eq!(hits[0].1.source, "a://1");
    }

    #[test]
    fn search_ranks_the_best_match_first() {
        let mut rag = RagPipeline::new();
        rag.ingest_str("s://cats", "the domestic cat is a small carnivorous mammal")
            .unwrap();
        rag.ingest_str("s://dogs", "dogs are loyal animals descended from wolves")
            .unwrap();
        rag.ingest_str("s://cars", "a car is a road vehicle with four wheels")
            .unwrap();

        let hits = rag.search("wolves loyal descended", 3);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].1.source, "s://dogs");
    }

    #[test]
    fn search_returns_nothing_for_unrelated_queries() {
        let mut rag = RagPipeline::new();
        rag.ingest_str("s://a", "cats purr").unwrap();
        // Non-positive scores are filtered out.
        assert!(rag.search("zzzz qqqq xxxx", 3).is_empty());
    }

    #[test]
    fn search_respects_top_k() {
        let mut rag = RagPipeline::new();
        for i in 0..10 {
            rag.ingest_str(&format!("s://{i}"), &format!("shared token document {i}"))
                .unwrap();
        }
        assert_eq!(rag.search("shared token", 3).len(), 3);
        assert_eq!(rag.search("shared token", 0).len(), 0);
    }

    #[test]
    fn search_on_empty_pipeline_returns_nothing() {
        let rag = RagPipeline::new();
        assert!(rag.search("anything", 5).is_empty());
        assert_eq!(rag.chunk_count(), 0);
    }

    #[test]
    fn blank_query_returns_nothing() {
        let mut rag = RagPipeline::new();
        rag.ingest_str("s://a", "some content").unwrap();
        assert!(rag.search("   ", 5).is_empty());
    }

    #[test]
    fn chunking_splits_on_word_boundaries() {
        // Regression: chunking cut mid-word on character boundaries.
        let long = (0..200)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = RagPipeline::split_chunks(&long, 50);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 50, "chunk exceeded size: {chunk:?}");
            assert!(!chunk.contains("  "), "double space in {chunk:?}");
            // No word should be truncated.
            for token in chunk.split(' ') {
                assert!(token.starts_with("word"), "truncated token {token:?}");
            }
        }
    }

    #[test]
    fn chunking_keeps_every_word() {
        let text = (0..100)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = RagPipeline::split_chunks(&text, 20);
        let rejoined: Vec<&str> = chunks.iter().flat_map(|c| c.split(' ')).collect();
        assert_eq!(rejoined.len(), 100, "no words may be lost or duplicated");
    }

    #[test]
    fn empty_content_ingests_nothing() {
        let mut rag = RagPipeline::new();
        assert_eq!(rag.ingest_str("s://empty", "   ").unwrap(), 0);
        assert_eq!(rag.chunk_count(), 0);
    }

    #[test]
    fn clear_empties_the_store() {
        let mut rag = RagPipeline::new();
        rag.ingest_str("s://a", "content here").unwrap();
        rag.clear();
        assert_eq!(rag.chunk_count(), 0);
        assert!(rag.search("content", 5).is_empty());
    }

    #[test]
    fn cosine_of_identical_vectors_is_one() {
        let v = TfIdfEmbedding::embed("alpha beta");
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_handles_zero_vectors() {
        let zero = vec![0.0f32; 8];
        let v = TfIdfEmbedding::embed("x");
        assert_eq!(cosine(&zero, &v), 0.0);
    }
}
