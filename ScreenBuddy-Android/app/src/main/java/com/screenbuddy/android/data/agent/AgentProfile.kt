package com.screenbuddy.android.data.agent

/**
 * Agent profiles, mirroring the desktop's `agent_profile.rs`.
 *
 * Kept field-for-field compatible with the desktop so a profile written on one
 * client reads sensibly on the other, and so the same vocabulary appears in both
 * UIs. Persona guidance strings are copied verbatim from the desktop: a profile
 * should not behave differently because it was edited on a phone.
 */

enum class AgentProvider(val id: String, val label: String) {
    /** Follow whatever the app settings currently say. */
    Auto("auto", "Automatic"),
    Ollama("ollama", "Ollama"),
    OpenAi("openai", "OpenAI"),
    Anthropic("anthropic", "Anthropic"),
    NineRouter("9router", "9Router"),
    LlamaCpp("llamacpp", "llama.cpp"),
    Custom("custom", "Custom endpoint");

    companion object {
        fun fromId(id: String?): AgentProvider =
            entries.firstOrNull { it.id.equals(id, ignoreCase = true) } ?: Auto

        /** The provider ids this app can actually route to. */
        val selectable: List<AgentProvider>
            get() = listOf(Auto, Ollama, OpenAi, Anthropic, NineRouter, LlamaCpp, Custom)
    }
}

enum class Persona(val id: String, val label: String, val guidance: String) {
    Assistant(
        "assistant", "Assistant",
        "Be helpful and direct."
    ),
    Friendly(
        "friendly", "Friendly",
        "Be warm and encouraging. Keep replies brief and upbeat, like a friend chatting on a desktop."
    ),
    Terse(
        "terse", "Terse",
        "Answer in as few words as possible. No preamble, no restating the question, " +
            "no summary of what you just said."
    ),
    Explainer(
        "explainer", "Explainer",
        "Explain your reasoning as you go. Prefer detail over brevity unless asked otherwise."
    ),
    Wry(
        "wry", "Wry",
        "Be dry and witty. A little humour is fine, but never at the expense of being correct."
    ),
    Professional(
        "professional", "Professional",
        "Be formal and precise. Avoid contractions and filler."
    );

    companion object {
        val DEFAULT = Assistant

        fun fromId(id: String?): Persona =
            entries.firstOrNull { it.id.equals(id, ignoreCase = true) } ?: DEFAULT
    }
}

enum class MovementStyle(val id: String, val label: String) {
    Idle("idle", "Idle"),
    Wander("wander", "Wander"),
    Follow("follow", "Follow the cursor"),
    Perch("perch", "Perch in place"),
    Playful("playful", "Playful");

    companion object {
        fun fromId(id: String?): MovementStyle =
            entries.firstOrNull { it.id.equals(id, ignoreCase = true) } ?: Wander
    }
}

/**
 * One saved agent.
 *
 * [model] is empty to mean "whatever the app settings provide", which keeps a
 * profile usable when a configured model disappears. [systemPrompt] is the escape
 * hatch for anything the typed fields do not cover, so a user is never blocked by
 * a missing setting.
 */
data class AgentProfile(
    val id: String,
    val name: String,
    val provider: AgentProvider = AgentProvider.Auto,
    val model: String = "",
    val creature: String = "",
    val movement: MovementStyle = MovementStyle.Wander,
    val persona: Persona = Persona.DEFAULT,
    val systemPrompt: String = "",
    val toolsEnabled: Boolean = true,
    val maxIterations: Int = 6,
    val timeoutSeconds: Int = 120,
    val temperature: Float = 0.7f,
    /** Built-ins are always present and cannot be deleted or edited in place. */
    val builtin: Boolean = false
) {
    /**
     * The system prompt this agent runs with.
     *
     * Persona guidance first, then the agent's own name, then any free-text
     * instructions: the user's own words come last so they are not diluted.
     */
    fun systemPrompt(): String = buildString {
        append(persona.guidance)
        if (name.isNotBlank()) {
            append("\n\nYou are ").append(name).append(", a ScreenBuddy companion.")
        }
        if (systemPrompt.isNotBlank()) {
            append("\n\n").append(systemPrompt.trim())
        }
    }

    /**
     * Validate and repair a profile before it is stored or used.
     *
     * Returns the corrected copy rather than throwing: a profile the user typed
     * by hand should be made usable, not rejected.
     */
    fun sanitized(): AgentProfile = copy(
        id = id.ifBlank { slug(name) },
        name = name.trim().ifBlank { "Unnamed agent" },
        model = model.trim(),
        creature = creature.trim(),
        systemPrompt = systemPrompt.trim(),
        // A blank budget would end the loop immediately; a huge one could hang.
        maxIterations = maxIterations.coerceIn(1, 50),
        timeoutSeconds = timeoutSeconds.coerceIn(5, 1800),
        temperature = temperature.coerceIn(0f, 2f)
    )

    /**
     * A flat map for IPC replies.
     *
     * Mirrors the desktop's agent payload so one set of instructions drives
     * either client.
     */
    fun toMap(): Map<String, Any?> = mapOf(
        "id" to id,
        "name" to name,
        "provider" to provider.id,
        "model" to model,
        "creature" to creature,
        "movement" to movement.id,
        "persona" to persona.id,
        "system_prompt" to systemPrompt,
        "tools_enabled" to toolsEnabled,
        "max_iterations" to maxIterations,
        "timeout_seconds" to timeoutSeconds,
        "temperature" to temperature,
        "builtin" to builtin
    )

    /** Whether [providerId]'s models are usable by this agent. */
    fun servesProvider(providerId: String): Boolean =
        provider == AgentProvider.Auto || provider.id.equals(providerId, ignoreCase = true)

    companion object {
        /** A filesystem- and id-safe name derived from the display name. */
        fun slug(name: String): String = name.trim().lowercase()
            .replace(Regex("[^a-z0-9]+"), "-")
            .trim('-')
            .ifBlank { "agent" }

        /**
         * The agents shipped with the app.
         *
         * Mirrors the desktop presets so a fresh install behaves the same on
         * either client.
         */
        val BUILT_INS: List<AgentProfile> = listOf(
            AgentProfile(
                id = "assistant",
                name = "Assistant",
                provider = AgentProvider.Auto,
                creature = "robo-cat-01",
                movement = MovementStyle.Wander,
                persona = Persona.Assistant,
                toolsEnabled = true,
                maxIterations = 8,
                builtin = true
            ),
            AgentProfile(
                id = "coder",
                name = "Coder",
                provider = AgentProvider.Auto,
                creature = "pixel-wizard-01",
                movement = MovementStyle.Idle,
                persona = Persona.Professional,
                systemPrompt = "When writing code, state the language, keep examples runnable, " +
                    "and say what you assumed.",
                toolsEnabled = true,
                maxIterations = 12,
                temperature = 0.2f,
                builtin = true
            ),
            AgentProfile(
                id = "study-buddy",
                name = "Study Buddy",
                provider = AgentProvider.Auto,
                creature = "cosmic-jellyfish-01",
                movement = MovementStyle.Perch,
                persona = Persona.Explainer,
                systemPrompt = "Check understanding with a question rather than just re-explaining.",
                toolsEnabled = false,
                builtin = true
            ),
            AgentProfile(
                id = "quick-ask",
                name = "Quick Ask",
                provider = AgentProvider.Auto,
                creature = "slime-king-01",
                movement = MovementStyle.Idle,
                persona = Persona.Terse,
                toolsEnabled = false,
                builtin = true
            ),
            AgentProfile(
                id = "companion",
                name = "Companion",
                provider = AgentProvider.Auto,
                creature = "companion-bird-01",
                movement = MovementStyle.Playful,
                persona = Persona.Friendly,
                systemPrompt = "Keep replies short and warm. It is a desktop pet, not a helpdesk.",
                toolsEnabled = false,
                builtin = true
            )
        )
    }
}

/** One profile as drawn in the list. */
data class AgentSummary(
    val id: String,
    val name: String,
    val persona: String,
    val creature: String,
    val builtin: Boolean,
    val active: Boolean = false
)