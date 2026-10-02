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
    /// Hashed subword projection into a fixed 512-dimension vector.
    ///
    /// Three signals are combined, because no single one covers real queries:
    ///
    /// * **Word tokens**, stemmed, so `running`, `runs` and `run` collapse
    ///   together. Plain bag-of-words treated those as three unrelated words.
    /// * **Character 3-grams**, so morphological variants and typos still
    ///   overlap: `feline`/`felines` and `cheetah`/`cheetahs` share grams.
    /// * **Concept clusters**, a small curated table that maps related terms onto
    ///   shared dimensions so `feline` can reach a document about `cats`.
    ///   Without it the embedding is purely lexical and no amount of hashing
    ///   makes synonyms comparable.
    ///
    /// Dimension count must stay in sync with every stored embedding, since
    /// `cosine` zips the two slices. It is 512 rather than 256 because at 256
    /// unrelated short documents collided often enough to distort ranking.
    pub const DIM: usize = 512;

    /// Substrings of length 3 that carry most of the signal. Full 1- and 2-grams
    /// are mostly noise and would crowd out the word features.
    const NGRAM: usize = 3;

    /// Words that should not be indexed at all.
    const STOPWORDS: &'static [&'static str] = &[
        "the", "a", "an", "and", "or", "but", "if", "then", "else", "of", "to", "in", "on", "at",
        "by", "for", "with", "about", "as", "is", "are", "was", "were", "be", "been", "being",
        "it", "its", "this", "that", "these", "those", "i", "you", "he", "she", "we", "they",
        "them", "my", "your", "his", "her", "our", "their", "do", "does", "did", "so", "not", "no",
        "can", "will", "would", "there", "here", "what", "which", "who", "how",
    ];

    pub fn embed(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; Self::DIM];
        let mut any = false;

        for raw in text.split_whitespace() {
            let token = normalize(raw);
            if token.is_empty() || Self::STOPWORDS.contains(&token.as_str()) {
                continue;
            }
            any = true;

            // 1. The stemmed word itself.
            let stem = stem(&token);
            bump(&mut v, &stem, 1.0);

            // 2. Character n-grams over the stem, so variants and typos overlap.
            // Unigrams are excluded; short words would otherwise add noise.
            let chars: Vec<char> = stem.chars().collect();
            if chars.len() > Self::NGRAM {
                for window in chars.windows(Self::NGRAM) {
                    let gram: String = window.iter().collect();
                    bump(&mut v, &gram, 0.35);
                }
            }

            // 3. Concept clusters, when this token belongs to one.
            if let Some(cluster) = concept_cluster(&stem) {
                bump(&mut v, cluster, 0.8);
            }
        }

        // An all-zero vector would make cosine return 0 for everything, so a
        // query made only of stopwords falls back to its raw characters.
        if !any {
            let squashed: String = text
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(64)
                .collect();
            for window in squashed.as_bytes().chunks(Self::NGRAM) {
                if window.len() == Self::NGRAM {
                    bump(&mut v, std::str::from_utf8(window).unwrap_or(""), 0.2);
                }
            }
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

/// Add weight to the bucket for `feature`, saturating rather than wrapping.
///
/// Wrapping let unrelated features bleed into one bucket; saturating keeps a
/// dominant term from inflating similarity with itself.
fn bump(v: &mut [f32], feature: &str, weight: f32) {
    if feature.is_empty() {
        return;
    }
    let idx = simple_hash(feature) % v.len();
    v[idx] = (v[idx] + weight).min(4.0);
}

/// Lowercase and strip surrounding punctuation.
fn normalize(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
        .collect::<String>()
        .to_lowercase()
}

/// A deliberately small suffix stripper in the spirit of Porter.
///
/// Full Porter is overkill here and easy to get subtly wrong; this collapses the
/// inflections that actually show up in prose without mangling short words.
/// Irregular forms that no suffix rule can reach. Kept short and common-only:
/// this is a lookup for the words that would otherwise mismatch outright, not an
/// attempt at linguistic completeness.
const IRREGULAR: &[(&str, &str)] = &[
    ("slept", "sleep"),
    ("sleeping", "sleep"),
    ("ran", "run"),
    ("running", "run"),
    ("flew", "fly"),
    ("swam", "swim"),
    ("children", "child"),
    ("mice", "mouse"),
    ("geese", "goose"),
    ("feet", "foot"),
    ("better", "good"),
    ("best", "good"),
];

fn stem(word: &str) -> String {
    if let Some((_, canonical)) = IRREGULAR.iter().find(|(w, _)| *w == word) {
        return (*canonical).to_string();
    }

    // Shortest stem we will produce. Three characters, not four: "runs" -> "run"
    // is a real stem, and a floor of 4 silently rejected it. Short words that
    // would be mangled this way ("is", "as", "was") are filtered as stopwords
    // before stemming runs.
    const MIN_STEM: usize = 3;
    let mut w = word.to_string();

    // Plurals and third person. "ies" is tried first so "flies" becomes "fly"
    // and agrees with the singular; the bare-"s" rule would otherwise claim it
    // and yield "flie".
    for suffix in ["ies", "es", "s"] {
        if !w.ends_with(suffix) {
            continue;
        }
        // "ies" -> "y" ("flies" -> "fly"); "es"/"s" just drop.
        let base = if suffix == "ies" {
            format!("{}y", &w[..w.len() - 3])
        } else {
            w[..w.len() - suffix.len()].to_string()
        };
        // The guard is on the result, not the input: "flies" is only 5 letters,
        // but stripping "ies" still leaves a valid 3-letter stem.
        // Order matters: with the bare-"s" rule first, "classes" stripped to
        // "classe". Trying "es" before "s" yields "class".
        //
        // The double-s guard applies only to the bare-"s" rule. There it stops
        // "class" becoming "clas"; applying it to "es" too would reject the
        // correct "classes" -> "class", which legitimately ends in "ss".
        let eats_double_s = suffix == "s" && base.ends_with("ss");
        if !eats_double_s && !w.ends_with("ss") && base.len() >= MIN_STEM {
            w = base;
            break;
        }
    }

    // Verb and adjective endings, applied after the noun pass.
    for suffix in ["ingly", "edly", "ing", "ed"] {
        if w.len() >= MIN_STEM + suffix.len() && w.ends_with(suffix) {
            let base = w[..w.len() - suffix.len()].to_string();
            // "running" -> "runn" -> "run": collapse a doubled final consonant.
            let last = base.chars().last();
            let prev = base.chars().rev().nth(1);
            let doubled = matches!(
                (last, prev),
                (Some(a), Some(b)) if a == b && !"aeiou".contains(a)
            );
            let trimmed = if doubled {
                base[..base.len() - a_char_len(last.unwrap())].to_string()
            } else {
                base
            };
            if trimmed.len() >= MIN_STEM {
                w = trimmed;
                break;
            }
        }
    }

    w
}

fn a_char_len(c: char) -> usize {
    c.len_utf8()
}

/// Curated synonym groups. Each group maps to one shared dimension, so any two
/// members score as related even though they share no characters.
///
/// This is intentionally small and hand-written: it is the minimum needed for
/// the obvious animal/companion vocabulary, and it keeps the crate free of any
/// model download. A learned embedding would do better but needs training data
/// and would make the build non-reproducible.
const CONCEPT_GROUPS: &[&[&str]] = &[
    &["cat", "feline", "kitten", "kitty", "meow"],
    &["dog", "canine", "puppy", "woof", "hound"],
    &["bird", "avian", "tweet", "songbird"],
    &["fish", "aquatic", "swim", "swimming"],
    &["dragon", "drake", "wyrm", "wyvern"],
    &["ghost", "spirit", "phantom", "spectral", "haunt"],
    &["slime", "ooze", "blob", "gel"],
    &["wizard", "mage", "sorcerer", "witch"],
    &["robot", "bot", "android", "machine", "droid"],
    &["jellyfish", "medusa", "sting"],
    &["companion", "friend", "buddy", "pal", "pet"],
    &["fast", "quick", "rapid", "speedy", "swift"],
    &["slow", "sluggish", "lazy", "gradual"],
    &["big", "large", "huge", "giant", "massive"],
    &["small", "tiny", "little", "mini"],
    &["happy", "glad", "cheerful", "joyful", "delighted"],
    &["sad", "unhappy", "sorrowful", "gloomy"],
    &["sleep", "sleeping", "snooze", "doze", "rest", "nap"],
    &["walk", "walking", "step", "stroll", "amble"],
    &["fly", "flying", "soar", "hover", "glide"],
    &["eat", "eating", "devour", "consume"],
    &["run", "running", "sprint", "dash"],
    &["swim", "swimming", "paddle"],
];

fn concept_cluster(stemmed: &str) -> Option<&'static str> {
    CONCEPT_GROUPS
        .iter()
        .find(|group| group.contains(&stemmed))
        // The dimension is derived from the first member, which is a literal.
        .map(|group| group[0])
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

    // ---- embedding behaviour that motivated replacing the old hashing ----

    fn cosine_between(a: &str, b: &str) -> f32 {
        cosine(&TfIdfEmbedding::embed(a), &TfIdfEmbedding::embed(b))
    }

    /// Inflected forms must reduce to the same stem, or a query for "flies"
    /// cannot reach a document that says "fly".
    #[test]
    fn inflections_share_a_stem() {
        for (forms, expected) in [
            (&["run", "running", "runs"][..], "run"),
            (&["fly", "flies", "flying"][..], "fly"),
            (&["cat", "cats"][..], "cat"),
            (&["walk", "walking", "walked"][..], "walk"),
            (&["sleep", "sleeps"][..], "sleep"),
        ] {
            for form in forms {
                assert_eq!(
                    stem(form),
                    expected,
                    "stem({form:?}) should be {expected:?}"
                );
            }
        }
    }

    /// Words that merely end in "ss" must not lose letters ("class" -> "clas").
    /// Irregular forms no suffix rule can derive.
    #[test]
    fn irregular_forms_map_to_a_canonical_stem() {
        for (form, canonical) in [
            ("slept", "sleep"),
            ("ran", "run"),
            ("flew", "fly"),
            ("children", "child"),
            ("feet", "foot"),
        ] {
            assert_eq!(
                stem(form),
                canonical,
                "stem({form:?}) should be {canonical:?}"
            );
        }
    }

    #[test]
    fn stemmer_does_not_mangle_ss_words() {
        assert_eq!(stem("class"), "class");
        assert_eq!(stem("classes"), "class");
    }

    /// The whole point of the concept table: synonyms with no shared characters
    /// must still score above zero.
    #[test]
    fn synonyms_are_retrievable() {
        let score = cosine_between("feline", "cats purr when content");
        assert!(
            score > 0.2,
            "expected 'feline' to reach a document about cats, got {score}"
        );
        assert!(cosine_between("kitten", "a small cat") > 0.2);
        assert!(cosine_between("puppy", "the dog barks") > 0.2);
    }

    /// Morphological variants and typos overlap via character n-grams even with
    /// no concept-table entry.
    #[test]
    fn near_miss_spellings_still_overlap() {
        assert!(cosine_between("cheetahs", "a cheetah sprints") > 0.3);
    }

    /// Unrelated text must not be dragged in; a stopword-only or foreign query
    /// should score at or near zero.
    #[test]
    fn unrelated_text_scores_zero() {
        let score = cosine_between("completely unrelated", "quantum chromodynamics");
        assert!(score < 0.05, "expected no relationship, got {score}");
    }

    /// A query of only stopwords must not collapse to the zero vector, or every
    /// comparison would return 0 and search would return nothing.
    #[test]
    fn stopword_only_query_still_produces_a_usable_vector() {
        let v = TfIdfEmbedding::embed("the and of it");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm > 0.0, "stopword-only text must not embed to zero");
        assert_eq!(v.len(), TfIdfEmbedding::DIM);
    }

    #[test]
    fn stopwords_do_not_dominate_similarity() {
        // Two texts differing only in stopwords should be very similar.
        let score = cosine_between("the cat and the mat", "a cat of the mat");
        assert!(
            score > 0.5,
            "stopwords should not dilute meaning, got {score}"
        );
    }

    #[test]
    fn concept_groups_map_to_one_dimension() {
        assert_eq!(concept_cluster(&stem("feline")), Some("cat"));
        assert_eq!(concept_cluster(&stem("kitten")), Some("cat"));
        assert_eq!(concept_cluster(&stem("puppy")), Some("dog"));
        assert_eq!(concept_cluster(&stem("xyzzy")), None);
    }

    /// End-to-end through the pipeline: a synonym query must retrieve the chunk.
    #[test]
    fn search_finds_a_chunk_via_a_synonym() {
        let mut rag = RagPipeline::new();
        rag.ingest_str(
            "memory://cats",
            "The domestic feline is a small carnivorous mammal kept as a pet.",
        )
        .expect("ingest");
        let hits = rag.search("feline", 5);
        assert_eq!(hits.len(), 1, "expected the cat chunk to be found");
        assert!(hits[0].0 > 0.0);
        assert!(hits[0].1.text.contains("feline"));
    }

    /// Ranking must put the on-topic chunk first, not merely return something.
    #[test]
    fn search_ranks_the_relevant_chunk_first() {
        let mut rag = RagPipeline::new();
        rag.ingest_str(
            "memory://a",
            "Rust ownership rules and borrow checker errors.",
        )
        .expect("ingest");
        rag.ingest_str(
            "memory://b",
            "The avian species migrates thousands of miles.",
        )
        .expect("ingest");
        rag.ingest_str("memory://c", "A puppy is a young dog that needs training.")
            .expect("ingest");

        let hits = rag.search("dog training", 3);
        assert!(!hits.is_empty(), "expected at least one hit");
        assert!(
            hits[0].1.text.contains("puppy") || hits[0].1.text.contains("dog"),
            "expected the dog chunk first, got {:?}",
            hits[0].1.text
        );
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
