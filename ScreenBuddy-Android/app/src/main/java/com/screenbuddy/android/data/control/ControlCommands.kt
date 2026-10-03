package com.screenbuddy.android.data.control

import android.content.Context
import com.screenbuddy.android.ScreenBuddyApp
import java.util.UUID

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

            // The reply used to stop here: the message was recorded and reported
            // as queued, but nothing called the AI service, so an agent's message
            // never produced a reply. Send it for real, and report the outcome
            // rather than claiming success.
            val service = app?.aiService
            if (service == null) {
                return@register ControlResponse.error("the AI service is unavailable")
            }
            // The catalogue is the source of truth for what can be sent; the
            // status snapshot does not carry a selected model.
            val model = com.screenbuddy.android.data.model.ProviderData.providers
                .flatMap { it.models }
                .firstOrNull { it.name == req.name || it.id == req.name }
                ?: return@register ControlResponse.error(
                    "no such model: ${req.name ?: "(none given)"}; " +
                        "pass one via 'name'"
                )

            android.util.Log.i(
                "ScreenBuddyControl",
                "send_chat model=${model.id} provider=${model.providerId} " +
                    "base=${service.currentBaseUrl()}"
            )
            ControlBus.launchIo {
                val request = com.screenbuddy.android.data.model.ChatMessage(
                    id = UUID.randomUUID().toString(),
                    role = "user",
                    content = message,
                    timestamp = System.currentTimeMillis()
                )
                service.sendMessage(listOf(request), model, null)
                    .onSuccess { reply ->
                        ControlBus.updateStatus {
                            it.copy(chatHistory = it.chatHistory + "assistant: ${reply.content}")
                        }
                    }
                    .onFailure { error ->
                        android.util.Log.e("ScreenBuddyControl", "send_chat failed", error)
                        // Recording the reason matters: an agent that gets
                        // "queued" and no reply can otherwise only guess.
                        ControlBus.updateStatus {
                            it.copy(chatHistory = it.chatHistory + "error: ${error.message}")
                        }
                    }
            }
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
            val key = req.key
            val settings = ControlBus.status.value.settings
            if (key.isNullOrBlank()) {
                ControlResponse.ok(mapOf("settings" to settings))
            } else {
                // Prefer the persisted value; the snapshot only has what was set
                // through this surface, so it cannot answer for the rest.
                val stored = app?.let { service ->
                    runCatching {
                        kotlinx.coroutines.runBlocking { service.settingsRepository.valueOf(key) }
                    }.getOrNull()
                }
                val value = stored ?: settings[key]
                    ?: return@register ControlResponse.error("unknown setting: $key", 404)
                ControlResponse.ok(mapOf("key" to key, "value" to value))
            }
        }

        ControlBus.register("set_setting") { req ->
            val key = req.key
                ?: return@register ControlResponse.error("set_setting requires 'key'")
            val value = req.value
                ?: return@register ControlResponse.error("set_setting requires 'value'")
            val repo = app?.settingsRepository
                ?: return@register ControlResponse.error("settings are unavailable", 503)

            // The write goes to DataStore, not to the status snapshot: the old
            // path reported success, read back the new value, and lost it on
            // restart without ever reaching the AI service.
            ControlBus.launchIo {
                val applied = runCatching { repo.setByKey(key, value) }.getOrDefault(false)
                if (applied) {
                    ControlBus.updateStatus { it.copy(settings = it.settings + (key to value)) }
                } else {
                    android.util.Log.w(
                        "ScreenBuddyControl",
                        "set_setting rejected: $key=$value"
                    )
                }
            }
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
