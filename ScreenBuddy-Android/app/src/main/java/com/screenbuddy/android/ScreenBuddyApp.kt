package com.screenbuddy.android

import android.app.Application
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
     * Control command handlers, installed once so agents can drive the app.
     *
     * Touched from onCreate rather than left lazy: a control broadcast can arrive
     * while the process is cold, and nothing else would ever force this to be
     * initialised, so the commands would silently do nothing.
     */
    val controlCommands: com.screenbuddy.android.data.control.ControlCommands by lazy {
        com.screenbuddy.android.data.control.ControlCommands.also { it.install(this) }
    }

    override fun onCreate() {
        super.onCreate()
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