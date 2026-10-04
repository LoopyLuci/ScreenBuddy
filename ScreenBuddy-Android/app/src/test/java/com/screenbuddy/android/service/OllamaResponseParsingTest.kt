package com.screenbuddy.android.service

import com.google.gson.Gson
import com.google.gson.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Ollama response parser.
 *
 * Ollama returns newline-delimited JSON whenever streaming is on, which the
 * stream_responses setting allows. Parsing that body as one object threw, so
 * every streamed request failed with a MalformedJsonException and the user saw
 * nothing. This was found by talking to a real model; no stub had reproduced it.
 */
class OllamaResponseParsingTest {

    private val gson = Gson()

    /** Mirrors AiService.parseOllamaBody, which is private. */
    private fun parse(body: String): String {
        val parts = mutableListOf<String>()
        for (line in body.trim().lines()) {
            val candidate = line.trim()
            if (candidate.isEmpty()) continue
            val obj = gson.fromJson(candidate, JsonObject::class.java)
            obj.getAsJsonObject("message")?.get("content")?.asString?.let(parts::add)
        }
        return parts.joinToString("")
    }

    @Test
    fun `a single object is read directly`() {
        val body = """{"model":"qwen2.5:0.5b","message":{"role":"assistant",""" +
            """"content":"Hello there"},"done":true}"""
        assertEquals("Hello there", parse(body))
    }

    @Test
    fun `newline delimited JSON is reassembled in order`() {
        // The exact shape a streamed Ollama reply arrives in.
        val body = """
            {"message":{"role":"assistant","content":"Hello"},"done":false}
            {"message":{"role":"assistant","content":", how"},"done":false}
            {"message":{"role":"assistant","content":" can I help?"},"done":true}
        """.trimIndent()
        assertEquals("Hello, how can I help?", parse(body))
    }

    @Test
    fun `a trailing newline does not add stray content`() {
        val body = "{\"message\":{\"content\":\"Hi\"},\"done\":true}\n"
        assertEquals("Hi", parse(body))
    }

    @Test
    fun `a real streamed body parses to the full reply`() {
        // Captured from a live request to qwen2.5:0.5b.
        val body = """
            {"model":"qwen2.5:0.5b","created_at":"2026-10-04T02:17:35.2919673Z","message":{"role":"assistant","content":"Hello"},"done":false}
            {"model":"qwen2.5:0.5b","created_at":"2026-10-04T02:17:35.3021274Z","message":{"role":"assistant","content":"! How can"},"done":false}
            {"model":"qwen2.5:0.5b","created_at":"2026-10-04T02:17:35.3180444Z","message":{"role":"assistant","content":" I help you today?"},"done":true,"done_reason":"stop"}
        """.trimIndent()
        assertEquals("Hello! How can I help you today?", parse(body))
    }

    @Test
    fun `blank lines between chunks are tolerated`() {
        val body = "{\"message\":{\"content\":\"a\"},\"done\":false}\n\n" +
            "{\"message\":{\"content\":\"b\"},\"done\":true}"
        assertEquals("ab", parse(body))
    }
}