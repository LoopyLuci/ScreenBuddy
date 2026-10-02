package com.screenbuddy.android.data.agent

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.screenbuddy.android.data.model.ChatMessage
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression tests for the Android agent loop.
 *
 * The desktop loop could never iterate because the response type could not carry
 * a tool call; these tests pin the equivalent behaviour here — the loop must
 * actually advance past a tool-use turn.
 */
class AgentLoopTest {

    private fun obj(json: String): JsonObject = JsonParser.parseString(json).asJsonObject

    private fun userMsg(text: String) = ChatMessage(
        id = "u",
        role = "user",
        content = text,
        timestamp = 0
    )

    private fun call(name: String, args: String = "{}", id: String = "call_1") =
        ToolCall(id, name, obj(args))

    @Test
    fun `text only answer finishes on the first iteration`() = runTest {
        var turns = 0
        val loop = AgentLoop(tools = AgentLoop.defaultTools())
        val out = loop.run(listOf(userMsg("hi")), model = { _, _ ->
            turns++
            AgentLoop.Turn("hello", emptyList())
        })
        assertEquals("hello", out)
        assertEquals(1, turns)
    }

    @Test
    fun `loop iterates after a tool call and returns the final text`() = runTest {
        var turns = 0
        val loop = AgentLoop(maxIterations = 5, tools = AgentLoop.defaultTools())
        val out = loop.run(
            listOf(userMsg("what time is it")),
            model = { _, _ ->
                turns++
                if (turns == 1) {
                    AgentLoop.Turn("", listOf(call("time", id = "call_t")))
                } else {
                    AgentLoop.Turn("It is now.", emptyList())
                }
            }
        )
        assertEquals("It is now.", out)
        assertEquals("must take two turns", 2, turns)
    }

    @Test
    fun `tool result is fed back into the conversation`() = runTest {
        val seen = mutableListOf<List<ChatMessage>>()
        val loop = AgentLoop(maxIterations = 4, tools = AgentLoop.defaultTools())
        var turns = 0
        loop.run(listOf(userMsg("echo hi")), model = { convo, _ ->
            seen += convo
            turns++
            if (turns == 1) {
                AgentLoop.Turn("", listOf(call("echo", """{"text":"hi"}""", id = "c1")))
            } else {
                AgentLoop.Turn("done", emptyList())
            }
        })
        // The second request must contain the user turn, an assistant turn, and a
        // tool result carrying the matching call id.
        val second = seen[1]
        assertTrue(second.any { it.role == "tool" && it.toolCallId == "c1" })
        assertTrue(second.any { it.role == "assistant" })
    }

    @Test
    fun `unknown tool is reported rather than throwing`() = runTest {
        var turns = 0
        val loop = AgentLoop(maxIterations = 3, tools = AgentLoop.defaultTools())
        var toolOutput: String? = null
        loop.run(listOf(userMsg("go")), model = { convo, _ ->
            turns++
            if (turns == 1) {
                AgentLoop.Turn("", listOf(call("nonexistent")))
            } else {
                convo.filter { it.role == "tool" }.firstOrNull()?.let { toolOutput = it.content }
                AgentLoop.Turn("finished", emptyList())
            }
        })
        assertTrue("unknown tool should be reported: $toolOutput", toolOutput.orEmpty().contains("unknown tool"))
    }

    @Test
    fun `handler failure is captured as an error string`() = runTest {
        val boom = Tool(
            spec = ToolSpec("boom", "always fails", obj("""{"type":"object"}""")),
            handler = { throw IllegalStateException("kaboom") }
        )
        var turns = 0
        var toolOutput: String? = null
        val loop = AgentLoop(maxIterations = 3, tools = listOf(boom))
        loop.run(listOf(userMsg("go")), model = { convo, _ ->
            turns++
            if (turns == 1) {
                AgentLoop.Turn("", listOf(call("boom")))
            } else {
                convo.filter { it.role == "tool" }.firstOrNull()?.let { toolOutput = it.content }
                AgentLoop.Turn("ok", emptyList())
            }
        })
        assertTrue(toolOutput.orEmpty().contains("kaboom"))
    }

    @Test
    fun `iteration cap is respected`() = runTest {
        var turns = 0
        // Model always asks for a tool, so only the cap can stop it.
        val loop = AgentLoop(maxIterations = 3, tools = AgentLoop.defaultTools())
        loop.run(
            listOf(userMsg("loop forever")),
            model = { _, _ ->
                turns++
                AgentLoop.Turn("", listOf(call("time", id = "c$turns")))
            }
        )
        assertEquals(3, turns)
    }

    @Test
    fun `zero max iterations still runs at least once`() = runTest {
        var turns = 0
        val loop = AgentLoop(maxIterations = 0)
        loop.run(
            listOf(userMsg("hi")),
            model = { _, _ ->
                turns++
                AgentLoop.Turn("hi", emptyList())
            }
        )
        assertEquals(1, turns)
    }

    @Test
    fun `model failure surfaces as a failure event and stops`() = runTest {
        val events = mutableListOf<AgentLoop.Event>()
        val loop = AgentLoop(maxIterations = 3)
        val out = loop.run(
            listOf(userMsg("hi")),
            model = { _, _ -> throw java.io.IOException("network down") },
            events = { events += it }
        )
        assertEquals("", out)
        assertTrue(events.any { it is AgentLoop.Event.Failure })
    }

    @Test
    fun `events report thinking and tool progress`() = runTest {
        val events = mutableListOf<AgentLoop.Event>()
        var turns = 0
        val loop = AgentLoop(maxIterations = 4, tools = AgentLoop.defaultTools())
        loop.run(listOf(userMsg("echo hi")), model = { _, _ ->
            turns++
            if (turns == 1) {
                AgentLoop.Turn("", listOf(call("echo", """{"text":"hi"}""", id = "c1")))
            } else {
                AgentLoop.Turn("done", emptyList())
            }
        }, events = { events += it })

        assertTrue(events.any { it is AgentLoop.Event.Thinking })
        assertTrue(events.any { it is AgentLoop.Event.ToolUse && it.name == "echo" })
        assertTrue(events.any { it is AgentLoop.Event.ToolResult })
        assertTrue(events.any { it is AgentLoop.Event.Final })
    }

    @Test
    fun `echo tool returns its input`() {
        val echo = AgentLoop.defaultTools().first { it.spec.name == "echo" }
        assertEquals("hi", echo.handler(obj("""{"text":"hi"}""")))
    }

    @Test
    fun `tool schemas use the openai function shape`() {
        val schemas = AgentLoop(tools = AgentLoop.defaultTools()).toolSchemas()
        assertEquals(2, schemas.size())
        val first = schemas[0].asJsonObject
        assertEquals("function", first.get("type").asString)
        assertEquals("echo", first.getAsJsonObject("function").get("name").asString)
    }

    @Test
    fun `no tools yields an empty schema array`() {
        assertEquals(0, AgentLoop().toolSchemas().size())
    }

    @Test
    fun `multiple tool calls in one turn are all executed`() = runTest {
        var turns = 0
        val outputs = mutableListOf<String>()
        val loop = AgentLoop(maxIterations = 3, tools = AgentLoop.defaultTools())
        loop.run(listOf(userMsg("go")), model = { convo, _ ->
            turns++
            if (turns == 1) {
                AgentLoop.Turn(
                    "",
                    listOf(
                        call("echo", """{"text":"a"}""", id = "c1"),
                        call("echo", """{"text":"b"}""", id = "c2")
                    )
                )
            } else {
                convo.filter { it.role == "tool" }.forEach { outputs += it.content }
                AgentLoop.Turn("done", emptyList())
            }
        })
        assertTrue(outputs.containsAll(listOf("a", "b")))
    }

    @Test
    fun `tool results carry distinct ids`() = runTest {
        var turns = 0
        val ids = mutableListOf<String?>()
        val loop = AgentLoop(maxIterations = 3, tools = AgentLoop.defaultTools())
        loop.run(listOf(userMsg("go")), model = { convo, _ ->
            turns++
            if (turns == 1) {
                AgentLoop.Turn(
                    "",
                    listOf(
                        call("echo", """{"text":"a"}""", id = "c1"),
                        call("echo", """{"text":"b"}""", id = "c2")
                    )
                )
            } else {
                convo.filter { it.role == "tool" }.forEach { ids += it.toolCallId }
                AgentLoop.Turn("done", emptyList())
            }
        })
        assertTrue(ids.containsAll(listOf("c1", "c2")))
        assertFalse(ids.any { it.isNullOrBlank() })
    }
}