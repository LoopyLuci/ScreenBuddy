package com.screenbuddy.android

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import com.screenbuddy.android.data.local.AppDatabase
import com.screenbuddy.android.data.rag.RagPipeline
import com.screenbuddy.android.data.repository.SettingsRepository
import com.screenbuddy.android.service.AiService
import com.screenbuddy.android.service.TtsEngine

/**
 * Manual dependency container.
 *
 * Everything is process-scoped and cheap to build, so a DI framework would add
 * a dependency without buying anything. Constructed lazily because `TtsEngine`
 * touches the platform TTS service, which must not be created during `attachBaseContext`.
 */
class ScreenBuddyApp : Application() {

    /** Outlives any one screen: settings must reach the AI service whenever they change. */
    private val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    val database: AppDatabase by lazy { AppDatabase.getDatabase(this) }

    val settingsRepository: SettingsRepository by lazy { SettingsRepository(this) }

    /**
     * Shared across the app so the configured Ollama endpoint applies everywhere.
     * Built once; the base URL comes from settings, read at call time by callers
     * that need an override.
     */
    val aiService: AiService by lazy { AiService() }

    /** RAG memory, mirroring the desktop pipeline. Empty until documents are ingested. */
    val ragPipeline: RagPipeline by lazy { RagPipeline() }

    val ttsEngine: TtsEngine by lazy { TtsEngine(this) }

    /**
     * Notifications for replies and finished tasks.
     *
     * The channel was never created, so the notifications setting controlled
     * nothing; this is what makes it capable of doing anything.
     */
    val notifier: com.screenbuddy.android.service.Notifier by lazy {
        com.screenbuddy.android.service.Notifier(this)
    }

    /**
     * Creature and UI sounds.
     *
     * Separate from TTS so the effects settings have their own output to
     * control, which is what those two switches mean.
     */
    val soundEngine: com.screenbuddy.android.service.SoundEngine by lazy {
        com.screenbuddy.android.service.SoundEngine(this).apply {
            // Loaded up front: the engine existed with nothing to play, so the
            // volume slider and the sound switches had no effect at all.
            val loaded = loadBundled()
            android.util.Log.i("ScreenBuddyApp", "loaded ${loaded.size} sound(s)")
        }
    }

    /** Saved agents, always including the built-in presets. */
    val agentProfiles: com.screenbuddy.android.data.agent.AgentProfileStore by lazy {
        com.screenbuddy.android.data.agent.AgentProfileStore.inAppFiles(this)
    }

    /**
     * The agent in force.
     *
     * Process-scoped because an agent's persona and tool budget must apply to
     * every request path, not just the one that happened to be open.
     */
    val agentRuntime: com.screenbuddy.android.data.agent.AgentRuntime by lazy {
        com.screenbuddy.android.data.agent.AgentRuntime(agentProfiles)
    }

    /**
     * Control command handlers, installed once so agents can drive the app.
     *
     * Touched from onCreate rather than left lazy: a control broadcast can arrive
     * while the process is cold, and nothing else would ever force this to be
     * initialised, so the commands would silently do nothing.
     */
    val controlCommands: com.screenbuddy.android.data.control.ControlCommands by lazy {
        com.screenbuddy.android.data.control.ControlCommands.also { it.install(this) }
    }

    /**
     * Push current settings into the shared AI service.
     *
     * Called from a coroutine scope so a settings change reaches every request
     * path at once. Without this the temperature and response-length controls
     * only ever reached storage.
     */
    fun syncAiSettings(settings: com.screenbuddy.android.data.repository.AppSettings) {
        aiService.update(
            AiService.GenerationSettings(
                temperature = settings.temperature,
                maxTokens = settings.responseLength,
                stream = settings.streamResponses
            )
        )
        // The endpoint too: it was a constructor value, so the configured
        // address never reached a request even though the UI saved it.
        aiService.updateEndpoints(settings.ollamaBaseUrl, emptyMap())
        android.util.Log.i(
            "ScreenBuddyApp",
            "settings applied: base=${settings.ollamaBaseUrl} " +
                "temp=${settings.temperature} max=${settings.responseLength}"
        )
        // The three audio switches, which had no consumer at all.
        soundEngine.uiSoundsEnabled = settings.soundEffectsEnabled
        soundEngine.creatureSoundsEnabled = settings.creatureSoundsEnabled
        soundEngine.volume =
            (settings.masterVolume * settings.effectsVolume).coerceIn(0f, 1f)
        // Master and TTS are separate sliders, so the level spoken is their
        // product. Previously both did nothing: TTS never set a volume.
        // Inside its own failure boundary: TtsEngine construction touches the
        // platform speech service, and an exception here would abort the
        // collector and leave every AI setting unapplied.
        runCatching {
            ttsEngine.setAudioSettings(
                enabled = settings.ttsEnabled,
                volume = (settings.masterVolume * settings.ttsVolume).coerceIn(0.0f, 1.0f)
            )
        }.onFailure { android.util.Log.w("ScreenBuddyApp", "TTS unavailable: ${it.message}") }
    }

    override fun onCreate() {
        super.onCreate()

        // Apply the stored settings once at startup, so the first request after
        // launch already uses them rather than the defaults.
        applicationScope.launch {
            settingsRepository.settings.collect { syncAiSettings(it) }
        }
        // Force the control handlers to register now. A broadcast can start the
        // process without the UI, and an unregistered handler would make every
        // command look accepted while doing nothing.
        controlCommands
    }

    override fun onTerminate() {
        super.onTerminate()
        // Only invoked on emulators, but release the TTS engine where possible.
        ttsEngine.shutdown()
    }
}