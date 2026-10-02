package com.screenbuddy.android.data.rag

import java.util.Locale
import kotlin.math.ln

/** A retrievable passage. */
data class Document(
    val id: String,
    val title: String,
    val content: String,
    val source: String = ""
)

/** A scored search hit. */
data class ScoredDocument(
    val document: Document,
    val score: Double,
    val snippet: String
)

/**
 * Retrieval-augmented memory, mirroring the desktop `rag.rs` pipeline.
 *
 * Documents are split into overlapping chunks and retrieved with Okapi BM25.
 * BM25 is used rather than embeddings so the whole pipeline is self-contained:
 * no model download, no network, and it works offline, which matches how the
 * desktop core scores its local tier.
 */
class RagPipeline(
    private val chunkSize: Int = DEFAULT_CHUNK_SIZE,
    private val chunkOverlap: Int = DEFAULT_CHUNK_OVERLAP
) {
    init {
        require(chunkSize > 0) { "chunkSize must be positive" }
        require(chunkOverlap >= 0 && chunkOverlap < chunkSize) {
            "chunkOverlap must be in [0, chunkSize)"
        }
    }

    private data class Chunk(val docId: String, val title: String, val source: String, val text: String)

    private val chunks = mutableListOf<Chunk>()
    private val docFreq = mutableMapOf<String, Int>()
    private val docLengths = mutableListOf<Int>()

    val documentCount: Int get() = chunks.map { it.docId }.distinct().size
    val chunkCount: Int get() = chunks.size

    /** Ingest a document, replacing any previous content with the same id. */
    fun ingest(document: Document): Int {
        remove(document.id)
        val newChunks = chunk(document.content).map {
            Chunk(document.id, document.title, document.source, it)
        }
        chunks.addAll(newChunks)
        rebuildIndex()
        return newChunks.size
    }

    fun ingestAll(documents: List<Document>): Int = documents.sumOf { ingest(it) }

    /** Remove every chunk belonging to [documentId]. */
    fun remove(documentId: String): Boolean {
        val before = chunks.size
        chunks.removeAll { it.docId == documentId }
        if (chunks.size != before) rebuildIndex()
        return chunks.size != before
    }

    fun clear() {
        chunks.clear()
        docFreq.clear()
        docLengths.clear()
    }

    /** Top [limit] chunks for [query], highest score first. */
    fun search(query: String, limit: Int = 5): List<ScoredDocument> {
        if (limit <= 0 || chunks.isEmpty()) return emptyList()
        val terms = tokenize(query)
        if (terms.isEmpty()) return emptyList()

        val avgLen = docLengths.average().let { if (it < 1.0) 1.0 else it }
        val n = chunks.size.toDouble()

        return chunks.mapIndexedNotNull { _, chunk ->
            val termsInChunk = tokenize(chunk.text)
            if (termsInChunk.isEmpty()) return@mapIndexedNotNull null

            val counts = termsInChunk.groupingBy { it }.eachCount()
            val len = termsInChunk.size.toDouble()
            var score = 0.0

            for (term in terms) {
                val tf = counts[term]?.toDouble() ?: continue
                val df = docFreq[term]?.toDouble() ?: continue
                // BM25 idf, floored at a small positive so a term present in
                // every chunk still contributes rather than going negative.
                val idf = ln(1.0 + (n - df + 0.5) / (df + 0.5)).coerceAtLeast(0.01)
                val numerator = tf * (K1 + 1)
                val denominator = tf + K1 * (1 - B + B * len / avgLen)
                score += idf * numerator / denominator
            }

            if (score <= 0.0) {
                null
            } else {
                ScoredDocument(
                    document = Document(chunk.docId, chunk.title, chunk.text, chunk.source),
                    score = score,
                    snippet = snippet(chunk.text, terms)
                )
            }
        }
            .sortedByDescending { it.score }
            .take(limit)
    }

    /** Build a prompt block from the best matches, or empty when nothing matches. */
    fun buildContext(query: String, limit: Int = 3): String {
        val hits = search(query, limit)
        if (hits.isEmpty()) return ""
        return hits.joinToString("\n\n") { hit ->
            val source = if (hit.document.source.isNotBlank()) {
                " (${hit.document.source})"
            } else {
                ""
            }
            "[${hit.document.title}$source] ${hit.snippet}"
        }
    }

    /** Split text into overlapping word-boundary chunks. */
    internal fun chunk(text: String): List<String> {
        val words = text.trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
        if (words.isEmpty()) return emptyList()

        val out = mutableListOf<String>()
        val step = chunkSize - chunkOverlap
        var i = 0
        while (i < words.size) {
            val end = minOf(i + chunkSize, words.size)
            out += words.subList(i, end).joinToString(" ")
            if (end >= words.size) break
            i += step
        }
        return out
    }

    private fun rebuildIndex() {
        docFreq.clear()
        docLengths.clear()
        chunks.forEach { chunk ->
            val tokens = tokenize(chunk.text)
            docLengths.add(tokens.size)
            tokens.toSet().forEach { term ->
                docFreq[term] = (docFreq[term] ?: 0) + 1
            }
        }
    }

    private fun snippet(text: String, terms: List<String>): String {
        if (text.length <= SNIPPET_LIMIT) return text
        val lower = text.lowercase(Locale.ROOT)
        val firstTerm = terms.firstOrNull { lower.contains(it) }
        val start = if (firstTerm != null) {
            (lower.indexOf(firstTerm) - SNIPPET_LEAD).coerceAtLeast(0)
        } else {
            0
        }
        val end = (start + SNIPPET_LIMIT).coerceAtMost(text.length)
        val prefix = if (start > 0) "…" else ""
        val suffix = if (end < text.length) "…" else ""
        return prefix + text.substring(start, end).trim() + suffix
    }

    private fun tokenize(text: String): List<String> = text
        .lowercase(Locale.ROOT)
        .split(TOKEN_SPLIT)
        .map { it.trim() }
        .filter { it.length > 1 && it !in STOP_WORDS }

    private companion object {
        const val DEFAULT_CHUNK_SIZE = 120
        const val DEFAULT_CHUNK_OVERLAP = 20
        const val SNIPPET_LIMIT = 240
        const val SNIPPET_LEAD = 40
        const val K1 = 1.2
        const val B = 0.75

        val TOKEN_SPLIT = Regex("[^\\p{L}\\p{N}]+")

        val STOP_WORDS = setOf(
            "the", "and", "for", "are", "but", "not", "you", "all", "can", "her", "was",
            "one", "our", "out", "day", "get", "has", "him", "his", "how", "its", "may",
            "new", "now", "old", "see", "two", "way", "who", "boy", "did", "any", "let",
            "put", "say", "she", "too", "use", "with", "that", "this", "from", "they",
            "have", "been", "were", "will", "would", "there", "their", "what", "about",
            "which", "when", "your", "into", "than", "then", "them", "these", "some",
            "could", "other", "such", "only", "also", "just", "like", "make", "many"
        )
    }
}
