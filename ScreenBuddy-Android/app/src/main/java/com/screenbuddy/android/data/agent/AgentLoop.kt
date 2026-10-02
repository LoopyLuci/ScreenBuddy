package com.screenbuddy.android.data.agent

import com.google.gson.JsonArray
import com.google.gson.JsonObject
import com.screenbuddy.android.data.model.ChatMessage

/** A tool the model may call, described with a JSON Schema. */
data class ToolSpec(
    val name: String,
    val description: String,
    val inputSchema: JsonObject
)

/** A tool invocation requested by the model. */
data class ToolCall(
    val id: String,
    val name: String,
    val arguments: JsonObject
)

/** A registered tool implementation. */
class Tool(
    val spec: ToolSpec,
    val handler: (JsonObject) -> String
)

/**
 * Minimal tool-calling agent, mirroring the desktop `AgentRuntime` loop.
 *
 * A tool-use turn is parsed from the model response, executed, and fed back as
 * a native tool-result message; the loop repeats until the model answers with
 * text only or [maxIterations] is reached. Kept free of Android APIs so it is
 * unit-testable on the JVM.
 */
class AgentLoop(
    private val maxIterations: Int = 5,
    private val tools: List<Tool> = emptyList()
) {
    /** What the loop observed, for UI feedback. */
    sealed interface Event {
        data class Thinking(val step: Int, val total: Int) : Event
        data class ToolUse(val name: String, val arguments: JsonObject) : Event
        data class ToolResult(val name: String, val output: String) : Event
        data class Final(val text: String) : Event
        data class Failure(val reason: String) : Event
    }

    /** One turn from the model: either text or tool calls. */
    data class Turn(
        val text: String,
        val toolCalls: List<ToolCall>
    )

    /** Requests the next turn given the conversation so far. */
    fun interface Model {
        suspend fun next(messages: List<ChatMessage>, toolSchemas: JsonArray): Turn
    }

    /**
     * Run until a text-only answer, a failure, or the iteration cap.
     *
     * [events] receives progress updates; the return value is the final text.
     */
    suspend fun run(initial: List<ChatMessage>, model: Model, events: (Event) -> Unit = {}): String {
        val conversation = initial.toMutableList()
        val schemas = toolSchemas()
        var latest = ""

        for (step in 1..maxIterations.coerceAtLeast(1)) {
            events(Event.Thinking(step, maxIterations))

            val turn = try {
                model.next(conversation.toList(), schemas)
            } catch (e: Exception) {
                events(Event.Failure(e.message ?: "model call failed"))
                return latest
            }

            latest = turn.text

            if (turn.toolCalls.isEmpty()) {
                events(Event.Final(turn.text))
                return turn.text
            }

            // Assistant turn carrying the tool calls.
            conversation += ChatMessage(
                id = nextId(),
                role = "assistant",
                content = turn.text,
                timestamp = System.currentTimeMillis()
            )

            for (call in turn.toolCalls) {
                events(Event.ToolUse(call.name, call.arguments))
                val output = execute(call)
                events(Event.ToolResult(call.name, output))
                // The id links the result to its call, which OpenAI-shaped
                // endpoints require via tool_call_id.
                conversation += ChatMessage(
                    id = nextId(),
                    role = "tool",
                    content = output,
                    timestamp = System.currentTimeMillis(),
                    toolCallId = call.id,
                    toolName = call.name
                )
            }
        }

        // Iteration cap reached: report whatever text we accumulated.
        events(Event.Final(latest))
        return latest
    }

    /** Tool schemas in OpenAI function-calling shape. */
    fun toolSchemas(): JsonArray {
        val arr = JsonArray()
        tools.forEach { tool ->
            arr.add(
                JsonObject().apply {
                    addProperty("type", "function")
                    add(
                        "function",
                        JsonObject().apply {
                            addProperty("name", tool.spec.name)
                            addProperty("description", tool.spec.description)
                            add("parameters", tool.spec.inputSchema)
                        }
                    )
                }
            )
        }
        return arr
    }

    /** Run a tool by name. Unknown tools and handler failures are reported, not thrown. */
    private fun execute(call: ToolCall): String {
        val tool = tools.firstOrNull { it.spec.name == call.name }
            ?: return "error: unknown tool '${call.name}'"
        return try {
            tool.handler(call.arguments)
        } catch (e: Exception) {
            "error: ${e.message}"
        }
    }

    private var counter = 0L

    private fun nextId(): String = "msg_${System.currentTimeMillis()}_${counter++}"

    companion object {
        /** Tools that need no Android APIs, so they work anywhere. */
        fun defaultTools(): List<Tool> = listOf(
            Tool(
                spec = ToolSpec(
                    name = "echo",
                    description = "Echo the supplied text back to the caller",
                    inputSchema = JsonObject().apply {
                        addProperty("type", "object")
                        add(
                            "properties",
                            JsonObject().apply {
                                add(
                                    "text",
                                    JsonObject().apply { addProperty("type", "string") }
                                )
                            }
                        )
                    }
                ),
                handler = { args -> args.get("text")?.asString ?: "" }
            ),
            Tool(
                spec = ToolSpec(
                    name = "time",
                    description = "Return the current time as an ISO-8601 timestamp",
                    inputSchema = JsonObject().apply { addProperty("type", "object") }
                ),
                handler = { java.time.LocalDateTime.now().toString() }
            )
        )
    }
}