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
        "list_agents",
        "get_agent_info",
        "activate_agent",
        "save_agent",
        "duplicate_agent",
        "delete_agent",
    )

    private val ANIMATION_STATES = setOf("idle", "walk", "fly", "sleep", "celebrate")

    /** A creature id used when the app has none yet, so commands still act. */
    private const val DEFAULT_ID = "companion-bird-01"
    private const val DEFAULT_X = 120f
    private const val DEFAULT_Y = 240f

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
            // Read the engine, not the status snapshot: the old version reported
            // a hardcoded x and y of zero, so no move was ever observable.
            val engine = ControlBus.creatures
            val creatures = engine.all()
            ControlResponse.ok(
                mapOf(
                    "creatures" to creatures.associate { c ->
                        c.id to mapOf(
                            "id" to c.id,
                            "visible" to c.visible,
                            "state" to c.animation,
                            "x" to c.x,
                            "y" to c.y,
                            "pinned" to c.pinned
                        )
                    },
                    "count" to engine.count(),
                    "visible_count" to engine.visibleCount(),
                    "auto_cycle" to engine.isAutoCycle(),
                    "elapsed" to engine.elapsed
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
            // Applied to the engine so the creature visibly changes, and held
            // so the next physics step does not immediately overwrite it.
            val engine = ControlBus.creatures
            // Operate on the selected creature, or the first one present, adding
            // one if the app has none yet.
            val id = ControlBus.status.value.selectedCreatureId
                ?: engine.all().firstOrNull()?.id
                ?: engine.add(DEFAULT_ID, DEFAULT_X, DEFAULT_Y).id
            engine.setAnimation(id, state)
            ControlBus.updateStatus { it.copy(animationState = state, autoCycle = false) }
            ControlResponse.ok(mapOf("animation" to state))
        }

        ControlBus.register("move_creature") { req ->
            val id = req.id
                ?: return@register ControlResponse.error("move_creature requires 'id'")
            if (req.x == null || req.y == null) {
                return@register ControlResponse.error("move_creature requires 'x' and 'y'")
            }
            val engine = ControlBus.creatures
            val known = engine.get(id) ?: engine.add(id, req.x, req.y)
            // moveTo pins the creature, so physics will not drift it away from
            // where the agent put it.
            engine.moveTo(id, req.x, req.y)
            ControlResponse.ok(
                mapOf("id" to id, "x" to engine.get(known.id)?.x, "y" to engine.get(known.id)?.y)
            )
        }

        ControlBus.register("set_creature_visible") { req ->
            val visible = req.visible
                ?: return@register ControlResponse.error("set_creature_visible requires 'visible'")
            // Applied to the engine so the creature actually appears or vanishes.
            ControlBus.creatures.all().forEach { ControlBus.creatures.setVisible(it.id, visible) }
            ControlBus.updateStatus { it.copy(visible = visible) }
            ControlResponse.ok(mapOf("visible" to visible))
        }

        ControlBus.register("set_auto_cycle") { req ->
            val enabled = req.enabled
                ?: return@register ControlResponse.error("set_auto_cycle requires 'enabled'")
            ControlBus.creatures.setAutoCycle(enabled)
            ControlBus.updateStatus { it.copy(autoCycle = enabled) }
            ControlResponse.ok(mapOf("auto_cycle" to enabled))
        }

        ControlBus.register("select_creature") { req ->
            val id = req.id
                ?: return@register ControlResponse.error("select_creature requires 'id'")
            val engine = ControlBus.creatures
            if (engine.get(id) == null) {
                engine.add(id, DEFAULT_X, DEFAULT_Y)
            }
            engine.setAnimation(id, com.screenbuddy.android.data.creature.CreatureEngine.ANIMATION_IDLE)
            ControlBus.updateStatus {
                it.copy(selectedCreatureId = id, animationState = "idle")
            }
            ControlResponse.ok(mapOf("selected" to id))
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

        ControlBus.register("list_agents") {
            val profiles = app?.agentProfiles
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            ControlResponse.ok(
                mapOf(
                    "agents" to profiles.summaries().map { summary ->
                        mapOf(
                            "id" to summary.id,
                            "name" to summary.name,
                            "persona" to summary.persona,
                            "creature" to summary.creature,
                            "builtin" to summary.builtin,
                            "active" to summary.active
                        )
                    }
                )
            )
        }

        ControlBus.register("get_agent_info") {
            val runtime = app?.agentRuntime
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            // Reports what the active agent is actually configured with, so a
            // caller can confirm activation changed something rather than taking
            // "queued" as proof.
            ControlResponse.ok(runtime.describe())
        }

        ControlBus.register("activate_agent") { req ->
            val runtime = app?.agentRuntime
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            val id = req.id
                ?: return@register ControlResponse.error("activate_agent requires 'id'")
            if (runtime.activate(id)) {
                ControlResponse.ok(
                    mapOf("active" to runtime.describe())
                )
            } else {
                ControlResponse.error("unknown agent: $id", 404)
            }
        }

        ControlBus.register("save_agent") { req ->
            val profiles = app?.agentProfiles
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            val name = req.name
                ?: return@register ControlResponse.error("save_agent requires 'name'")
            val existing = req.id?.let { profiles.get(it) }
            val saved = profiles.save(
                com.screenbuddy.android.data.agent.AgentProfile(
                    id = req.id ?: "",
                    name = name,
                    model = req.state.orEmpty().ifBlank { existing?.model.orEmpty() },
                    systemPrompt = req.content.orEmpty()
                        .ifBlank { existing?.systemPrompt.orEmpty() },
                    persona = com.screenbuddy.android.data.agent.Persona
                        .fromId(existing?.persona?.id)
                )
            )
            // A save must reach the runtime too, or the edit would not apply
            // until the next launch.
            app?.agentRuntime?.reload()
            ControlResponse.ok(mapOf("agent" to saved.sanitized().toMap()))
        }

        ControlBus.register("duplicate_agent") { req ->
            val profiles = app?.agentProfiles
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            val id = req.id
                ?: return@register ControlResponse.error("duplicate_agent requires 'id'")
            val copy = profiles.duplicate(id)
                ?: return@register ControlResponse.error("unknown agent: $id", 404)
            ControlResponse.ok(mapOf("agent" to copy.name))
        }

        ControlBus.register("delete_agent") { req ->
            val profiles = app?.agentProfiles
                ?: return@register ControlResponse.error("agents are unavailable", 503)
            val id = req.id
                ?: return@register ControlResponse.error("delete_agent requires 'id'")
            val target = profiles.get(id)
                ?: return@register ControlResponse.error("unknown agent: $id", 404)
            if (target.builtin) {
                return@register ControlResponse.error(
                    "'${target.name}' is built in and cannot be deleted; duplicate it instead",
                    403
                )
            }
            profiles.delete(id)
            app?.agentRuntime?.reload()
            ControlResponse.ok(mapOf("deleted" to id))
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
