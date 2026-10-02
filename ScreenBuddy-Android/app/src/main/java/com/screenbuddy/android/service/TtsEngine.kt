package com.screenbuddy.android.service

import android.content.Context
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import java.util.Locale
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Text-to-speech for AI replies, mirroring the desktop `tts.rs` engine.
 *
 * Wraps the platform [TextToSpeech] service. The engine initialises
 * asynchronously, so callers must observe [isReady] (or pass an
 * [onReady] callback) before speaking; [speak] on an unready engine is a no-op
 * rather than an error, so a missing TTS engine cannot break the chat flow.
 *
 * Release with [shutdown] when the owning component goes away.
 */
class TtsEngine(context: Context) {

    private val appContext = context.applicationContext
    private val ready = AtomicBoolean(false)

    /** True once the platform engine has initialised successfully. */
    val isReady: Boolean get() = ready.get()

    // The engine calls back on init, and that callback mutates `ready`, so the
    // property must exist before construction starts.
    private var tts: TextToSpeech? = null

    init {
        tts = TextToSpeech(appContext) { status ->
            val ok = status == TextToSpeech.SUCCESS
            if (ok) {
                runCatching { tts?.language = Locale.getDefault() }
            } else {
                Log.w(TAG, "Text-to-speech engine unavailable (status=$status)")
            }
            ready.set(ok)
        }
    }

    /** Speak [text], interrupting anything currently playing. */
    fun speak(text: String) {
        if (!ready.get()) return
        val body = sanitize(text)
        if (body.isEmpty()) return
        runCatching {
            tts?.speak(body, TextToSpeech.QUEUE_FLUSH, null, UTTERANCE_ID)
        }.onFailure { Log.w(TAG, "speak failed: ${it.message}") }
    }

    /** Speak and invoke [onDone] when playback finishes or fails. */
    fun speak(text: String, onDone: () -> Unit) {
        if (!ready.get()) {
            onDone()
            return
        }
        val body = sanitize(text)
        if (body.isEmpty()) {
            onDone()
            return
        }
        tts?.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) = Unit
            override fun onDone(utteranceId: String?) = onDone()
            @Deprecated("Superseded by onError(String, Int)", ReplaceWith(""))
            override fun onError(utteranceId: String?) = onDone()
            override fun onError(utteranceId: String?, errorCode: Int) = onDone()
        })
        runCatching { tts?.speak(body, TextToSpeech.QUEUE_FLUSH, null, UTTERANCE_ID) }
            .onFailure { onDone() }
    }

    /** Stop playback and clear any pending utterance. */
    fun stop() {
        runCatching { tts?.stop() }
    }

    /** Release the engine. Safe to call more than once. */
    fun shutdown() {
        ready.set(false)
        runCatching {
            tts?.stop()
            tts?.shutdown()
        }
        tts = null
    }

    private companion object {
        const val TAG = "TtsEngine"
        const val UTTERANCE_ID = "screenbuddy"

        /**
         * Strip markdown and collapse whitespace. Models emit `**bold**`, list
         * markers, and code fences that read badly when spoken aloud.
         */
        fun sanitize(text: String): String = text
            .replace(Regex("```[\\s\\S]*?```"), " code block ")
            .replace(Regex("`([^`]*)`"), "$1")
            .replace(Regex("\\*\\*([^*]*)\\*\\*"), "$1")
            .replace(Regex("\\*([^*]*)\\*"), "$1")
            .replace(Regex("^\\s*[-*+]\\s+", RegexOption.MULTILINE), "")
            .replace(Regex("\\s+"), " ")
            .trim()
    }
}