package com.screenbuddy.android.data.rag

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Exercises the BM25 retrieval pipeline that backs RAG memory. */
class RagPipelineTest {

    private fun doc(id: String, title: String, content: String) =
        Document(id = id, title = title, content = content, source = "$id.md")

    @Test
    fun `ingest creates chunks`() {
        val rag = RagPipeline(chunkSize = 10, chunkOverlap = 2)
        val text = (1..30).joinToString(" ") { "word$it" }
        val created = rag.ingest(doc("a", "Alpha", text))
        assertTrue("expected multiple chunks, got $created", created > 1)
        assertEquals(1, rag.documentCount)
    }

    @Test
    fun `search finds the most relevant document`() {
        val rag = RagPipeline()
        rag.ingest(doc("cats", "Cats", "The domestic cat is a small carnivorous mammal kept as a pet."))
        rag.ingest(doc("dogs", "Dogs", "Dogs are loyal domesticated animals descended from wolves."))
        rag.ingest(doc("cars", "Cars", "A car is a road vehicle with four wheels and an engine."))

        val hits = rag.search("domesticated wolves")
        assertTrue(hits.isNotEmpty())
        assertEquals("dogs", hits.first().document.id)
    }

    @Test
    fun `search returns nothing for an empty pipeline`() {
        val rag = RagPipeline()
        assertTrue(rag.search("anything").isEmpty())
    }

    @Test
    fun `search returns nothing when nothing matches`() {
        val rag = RagPipeline()
        rag.ingest(doc("cats", "Cats", "The domestic cat is a small carnivorous mammal."))
        assertTrue(rag.search("quantum chromodynamics").isEmpty())
    }

    @Test
    fun `stop words do not create matches`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "The and for are but not you all can"))
        // Query is entirely stop words, so it should not match anything.
        assertTrue(rag.search("the and for").isEmpty())
    }

    @Test
    fun `results are ordered by descending score`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "pneumothorax pneumothorax treatment"))
        rag.ingest(doc("b", "B", "pneumothorax rare"))
        val hits = rag.search("pneumothorax")
        assertTrue(hits.size >= 2)
        // More occurrences should score higher.
        assertTrue(hits[0].score >= hits[1].score)
        assertEquals("a", hits[0].document.id)
    }

    @Test
    fun `limit caps the number of results`() {
        val rag = RagPipeline()
        repeat(10) { i -> rag.ingest(doc("d$i", "Doc $i", "shared keyword content $i")) }
        assertEquals(3, rag.search("keyword", limit = 3).size)
    }

    @Test
    fun `limit of zero or less returns nothing`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "keyword"))
        assertTrue(rag.search("keyword", limit = 0).isEmpty())
    }

    @Test
    fun `re-ingesting the same id replaces the old content`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "original content about penguins"))
        rag.ingest(doc("a", "A", "replacement content about giraffes"))
        assertEquals(1, rag.documentCount)
        // The replacement text is retrievable and the stale text is gone.
        assertEquals(1, rag.search("giraffes").size)
        assertTrue("stale content should not match", rag.search("penguins").isEmpty())
    }

    @Test
    fun `remove deletes a document`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "alpha keyword"))
        rag.ingest(doc("b", "B", "beta keyword"))
        assertTrue(rag.remove("a"))
        assertEquals(1, rag.documentCount)
        assertFalse(rag.search("alpha").isNotEmpty())
    }

    @Test
    fun `remove of an unknown id reports false`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "alpha"))
        assertFalse(rag.remove("nope"))
    }

    @Test
    fun `clear empties the pipeline`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "alpha keyword"))
        rag.clear()
        assertEquals(0, rag.chunkCount)
        assertTrue(rag.search("keyword").isEmpty())
    }

    @Test
    fun `empty content produces no chunks`() {
        val rag = RagPipeline()
        assertEquals(0, rag.ingest(doc("a", "A", "   ")))
    }

    @Test
    fun `chunking respects the configured size`() {
        val rag = RagPipeline(chunkSize = 5, chunkOverlap = 0)
        val text = (1..20).joinToString(" ") { "w$it" }
        rag.ingest(doc("a", "A", text))
        // 20 words / 5 per chunk = 4 chunks with no overlap.
        assertEquals(4, rag.chunkCount)
    }

    @Test
    fun `chunk overlap produces more chunks than a plain split`() {
        val text = (1..20).joinToString(" ") { "w$it" }
        val plain = RagPipeline(chunkSize = 5, chunkOverlap = 0)
        val overlapped = RagPipeline(chunkSize = 5, chunkOverlap = 2)
        plain.ingest(doc("a", "A", text))
        overlapped.ingest(doc("a", "A", text))
        assertTrue(overlapped.chunkCount > plain.chunkCount)
    }

    @Test
    fun `buildContext formats hits with title and source`() {
        val rag = RagPipeline()
        rag.ingest(doc("cats", "Cats", "The domestic cat is a carnivorous mammal kept as a pet."))
        val context = rag.buildContext("domestic cat")
        assertTrue(context.contains("Cats"))
        assertTrue(context.contains("cats.md"))
    }

    @Test
    fun `buildContext is empty when nothing matches`() {
        val rag = RagPipeline()
        rag.ingest(doc("a", "A", "alpha"))
        assertEquals("", rag.buildContext("zzz"))
    }

    @Test
    fun `ingestAll ingests every document`() {
        val rag = RagPipeline()
        rag.ingestAll(listOf(doc("a", "A", "alpha"), doc("b", "B", "beta"), doc("c", "C", "gamma")))
        assertEquals(3, rag.documentCount)
    }

    @Test
    fun `snippet is bounded for long chunks`() {
        val rag = RagPipeline(chunkSize = 500, chunkOverlap = 0)
        val long = (1..2000).joinToString(" ") { "term$it" }
        rag.ingest(doc("a", "A", long))
        val hit = rag.search("term1999").first()
        assertTrue("snippet should be truncated", hit.snippet.length <= 300)
        assertTrue(hit.snippet.startsWith("…"))
    }
}