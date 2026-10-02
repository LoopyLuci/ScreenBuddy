package com.screenbuddy.android.data.control

import android.content.Context
import com.screenbuddy.android.ScreenBuddyApp

/**
 * Registers the control command handlers.
 *
 * Command names and response shapes match the desktop IPC surface so the same
 * agent instructions work against either client.
 */
object ControlCommands {

    /** Commands an agent may send. Kept explicit so typos fail loudly. */
    val SUPPORTED = listOf(
        "ping",
        "get_status",
        "list_creature_state",
        "set_animation",
        "move_creature",
        "set_creature_visible",
        "set_auto_cycle",
        "select_creature",
        "send_chat",
        "get_chat_history",
        "get_agent_info",
        "play_sound",
        "speak",
        "get_setting",
        "set_setting",
        "memory_ingest",
        "memory_search",
    )

    private val ANIMATION_STATES = setOf("idle", "walk", "fly", "sleep", "celebrate")

    fun install(context: Context) {
        val app = context.applicationContext as? ScreenBuddyApp

        ControlBus.register("ping") {
            ControlResponse.ok(mapOf("status" to "pong"))
        }

        ControlBus.register("get_status") {
            val s = ControlBus.status.value
            ControlResponse.ok(
                mapOf(
                    "selected_creature" to s.selectedCreatureId,
                    "visible" to s.visible,
                    "animation_state" to s.animationState,
                    "auto_cycle" to s.autoCycle,
                    "chat_messages" to s.chatHistory.size,
                    "memory_chunks" to s.memoryChunks,
                    "agent_tool_count" to s.agentToolCount,
                    "settings_count" to s.settings.size,
                )
            )
        }

        ControlBus.register("list_creature_state") {
            val s = ControlBus.status.value
            val creature = s.selectedCreatureId ?: "none"
            ControlResponse.ok(
                mapOf(
                    "creatures" to mapOf(
                        creature to mapOf(
                            "id" to creature,
                            "visible" to s.visible,
                            "state" to s.animationState,
                            "x" to 0.0,
                            "y" to 0.0,
                        )
                    )
                )
            )
        }

        ControlBus.register("set_animation") { req ->
            val state = req.state
                ?: return@register ControlResponse.error("set_animation requires 'state'")
            if (state !in ANIMATION_STATES) {
                return@register ControlResponse.error(
                    "state must be one of ${ANIMATION_STATES.sorted()}, got '$state'"
                )
            }
            // An externally requested state must not be overwritten by the
            // rotation timer, so holding one implies pausing auto-cycle.
            ControlBus.updateStatus { it.copy(animationState = state, autoCycle = false) }
            ControlResponse.queued("animation")
        }

        ControlBus.register("move_creature") { req ->
            val id = req.id
                ?: return@register ControlResponse.error("move_creature requires 'id'")
            if (req.x == null || req.y == null) {
                return@register ControlResponse.error("move_creature requires 'x' and 'y'")
            }
            val current = ControlBus.status.value.selectedCreatureId
            if (current != null && current != id) {
                return@register ControlResponse.error("unknown creature: $id", 404)
            }
            ControlResponse.queued("move")
        }

        ControlBus.register("set_creature_visible") { req ->
            val visible = req.visible
                ?: return@register ControlResponse.error("set_creature_visible requires 'visible'")
            ControlBus.updateStatus { it.copy(visible = visible) }
            ControlResponse.queued("visibility")
        }

        ControlBus.register("set_auto_cycle") { req ->
            val enabled = req.enabled
                ?: return@register ControlResponse.error("set_auto_cycle requires 'enabled'")
            ControlBus.updateStatus { it.copy(autoCycle = enabled) }
            ControlResponse.queued("auto_cycle")
        }

        ControlBus.register("select_creature") { req ->
            val id = req.id
                ?: return@register ControlResponse.error("select_creature requires 'id'")
            ControlBus.updateStatus {
                it.copy(selectedCreatureId = id, animationState = "idle")
            }
            ControlResponse.queued("selection")
        }

        ControlBus.register("send_chat") { req ->
            val message = req.message
                ?: return@register ControlResponse.error("send_chat requires 'message'")
            if (message.isBlank()) {
                return@register ControlResponse.error("message must not be blank")
            }
            ControlBus.updateStatus { it.copy(chatHistory = it.chatHistory + "user: $message") }
            ControlResponse.queued("chat")
        }

        ControlBus.register("get_chat_history") {
            ControlResponse.ok(mapOf("messages" to ControlBus.status.value.chatHistory))
        }

        ControlBus.register("get_agent_info") {
            val tools = app?.let { listOf("list_creatures", "read_memory", "play_sound") } ?: emptyList()
            ControlResponse.ok(
                mapOf(
                    "tool_count" to tools.size,
                    "tools" to tools,
                )
            )
        }

        ControlBus.register("play_sound") { req ->
            val name = req.name ?: return@register ControlResponse.error("play_sound requires 'name'")
            ControlResponse.queued("sound:$name")
        }

        ControlBus.register("speak") { req ->
            val text = req.message ?: req.content
                ?: return@register ControlResponse.error("speak requires 'text'")
            ControlResponse.queued("speech")
        }

        ControlBus.register("get_setting") { req ->
            val settings = ControlBus.status.value.settings
            val key = req.key
            if (key.isNullOrBlank()) {
                ControlResponse.ok(mapOf("settings" to settings))
            } else {
                val value = settings[key]
                    ?: return@register ControlResponse.error("unknown setting: $key", 404)
                ControlResponse.ok(mapOf("key" to key, "value" to value))
            }
        }

        ControlBus.register("set_setting") { req ->
            val key = req.key
                ?: return@register ControlResponse.error("set_setting requires 'key'")
            val value = req.value
                ?: return@register ControlResponse.error("set_setting requires 'value'")
            ControlBus.updateStatus { it.copy(settings = it.settings + (key to value)) }
            ControlResponse.queued("setting:$key")
        }

        ControlBus.register("memory_ingest") { req ->
            val source = req.source
                ?: return@register ControlResponse.error("memory_ingest requires 'source'")
            val content = req.content
                ?: return@register ControlResponse.error("memory_ingest requires 'content'")
            val pipeline = app?.ragPipeline
                ?: return@register ControlResponse.error("memory is not available", 503)
            val document = com.screenbuddy.android.data.rag.Document(
                id = source,
                title = source,
                content = content,
                source = source,
            )
            val chunks = runCatching { pipeline.ingest(document) }.getOrElse {
                return@register ControlResponse.error("ingest failed: ${it.message}", 500)
            }
            ControlBus.updateStatus { it.copy(memoryChunks = it.memoryChunks + chunks) }
            ControlResponse.ok(mapOf("chunks" to chunks))
        }

        ControlBus.register("memory_search") { req ->
            val query = req.query
                ?: return@register ControlResponse.error("memory_search requires 'query'")
            val pipeline = app?.ragPipeline
                ?: return@register ControlResponse.error("memory is not available", 503)
            val limit = (req.limit ?: 5).coerceIn(1, 50)
            val hits = runCatching { pipeline.search(query, limit) }.getOrElse {
                return@register ControlResponse.error("search failed: ${it.message}", 500)
            }
            ControlResponse.ok(
                mapOf(
                    "query" to query,
                    "results" to hits.map { hit ->
                        mapOf(
                            "score" to hit.score,
                            "text" to hit.snippet,
                            "source" to hit.document.source,
                        )
                    },
                )
            )
        }
    }
}
