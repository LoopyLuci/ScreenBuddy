package com.screenbuddy.android.data.agent

/**
 * The agent currently in force.
 *
 * The desktop equivalent had this defect: the runtime was built from defaults and
 * never told what the active profile said, so a saved persona, model or tool
 * budget was stored and ignored. Here every field that affects a request comes
 * from the profile, and [apply] is the only way it changes.
 *
 * Held in memory and re-read from the store by the view model, which keeps a
 * profile edit from mutating a request already in flight.
 */
class AgentRuntime(private val store: AgentProfileStore) {

    @Volatile
    private var profile: AgentProfile = store.activeProfile()
        // No active agent is a legitimate state, so fall back to a default rather
        // than refusing to run: the app has to work before anyone picks an agent.
        ?: AgentProfile.BUILT_INS.first()

    /** The profile now in force. */
    fun active(): AgentProfile = profile

    /**
     * Make [id] the active agent.
     *
     * Persisted, so it survives a restart: an agent the user chose should still
     * be in charge next launch.
     */
    fun activate(id: String?): Boolean {
        if (id == null) {
            store.setActive(null)
            profile = AgentProfile.BUILT_INS.first()
            return true
        }
        if (!store.setActive(id)) return false
        profile = store.get(id) ?: return false
        return true
    }

    /** Re-read the active profile from storage, after an edit. */
    fun reload(): AgentProfile {
        profile = store.activeProfile() ?: AgentProfile.BUILT_INS.first()
        return profile
    }

    /**
     * The system prompt to send.
     *
     * Taken from the active profile rather than a constant, so a persona actually
     * changes how the agent answers.
     */
    fun systemPrompt(): String = profile.systemPrompt()

    /** Tool-call budget for one request. */
    fun maxIterations(): Int = profile.maxIterations

    /** Whether this agent may use tools at all. */
    fun toolsEnabled(): Boolean = profile.toolsEnabled

    /** Whether [providerId] is one this agent will answer through. */
    fun serves(providerId: String): Boolean = profile.servesProvider(providerId)

    /**
     * The provider this agent prefers, or null to use whatever the app settings
     * say.
     */
    fun providerOverride(): String? =
        profile.provider.takeIf { it != AgentProvider.Auto }?.id

    /**
     * The model this agent pins, or null to let the app choose.
     *
     * Empty means automatic, which is what keeps a profile working when a
     * configured model disappears.
     */
    fun pinnedModel(): String? = profile.model.takeIf { it.isNotBlank() }

    /** Per-request timeout in seconds. */
    fun timeoutSeconds(): Int = profile.timeoutSeconds

    /**
     * The temperature this agent runs with, or null to use the app setting.
     *
     * A profile that does not choose a temperature must not silently override
     * the user's own slider.
     */
    fun temperatureOverride(): Float? =
        profile.temperature.takeIf { it != DEFAULT_TEMPERATURE }

    /** A snapshot for the UI and for status replies. */
    fun describe(): Map<String, Any?> = mapOf(
        "id" to profile.id,
        "name" to profile.name,
        "provider" to profile.provider.id,
        "model" to profile.model,
        "creature" to profile.creature,
        "movement" to profile.movement.id,
        "persona" to profile.persona.id,
        "tools_enabled" to profile.toolsEnabled,
        "max_iterations" to profile.maxIterations,
        "timeout_seconds" to profile.timeoutSeconds,
        "temperature" to profile.temperature,
        "builtin" to profile.builtin
    )

    companion object {
        /**
         * The temperature a profile carries when the user did not choose one.
         *
         * Treated as "unset" rather than as a real value, so it does not override
         * the app's own setting.
         */
        const val DEFAULT_TEMPERATURE = 0.7f
    }
}