package com.screenbuddy.android.data.repository

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.floatPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore("settings")

/** User-facing settings, persisted so they survive process death. */
data class AppSettings(
    val darkMode: Boolean = false,
    val notificationsEnabled: Boolean = true,
    val soundEffectsEnabled: Boolean = true,
    val autoStart: Boolean = false,
    val streamResponses: Boolean = true,
    val showCreatures: Boolean = true,
    val animationsEnabled: Boolean = true,
    val creatureSoundsEnabled: Boolean = true,
    val ttsEnabled: Boolean = false,
    val masterVolume: Float = 0.7f,
    val effectsVolume: Float = 0.8f,
    val ttsVolume: Float = 0.8f,
    val animationSpeed: Float = 1.0f,
    val temperature: Float = 0.7f,
    val responseLength: Int = 512,
    val selectedModelId: String = "",
    val ollamaBaseUrl: String = "http://10.0.2.2:11434"
) {
    /** Effective generation temperature, validated before it reaches an API. */
    val safeTemperature: Float get() = temperature.coerceIn(0.0f, 2.0f)
}

/**
 * DataStore-backed settings.
 *
 * Reads expose a [Flow] so the UI updates live; writes go through `edit`, which
 * suspends and persists. Every key has a default, so a fresh install and a
 * partially-written store both resolve to a usable [AppSettings].
 */
class SettingsRepository(context: Context) {

    private val store = context.applicationContext.dataStore

    val settings: Flow<AppSettings> = store.data.map { prefs ->
        AppSettings(
            darkMode = prefs[KEY_DARK_MODE] ?: false,
            notificationsEnabled = prefs[KEY_NOTIFICATIONS] ?: true,
            soundEffectsEnabled = prefs[KEY_SOUND_EFFECTS] ?: true,
            autoStart = prefs[KEY_AUTO_START] ?: false,
            streamResponses = prefs[KEY_STREAM] ?: true,
            showCreatures = prefs[KEY_SHOW_CREATURES] ?: true,
            animationsEnabled = prefs[KEY_ANIMATIONS] ?: true,
            creatureSoundsEnabled = prefs[KEY_CREATURE_SOUNDS] ?: true,
            ttsEnabled = prefs[KEY_TTS] ?: false,
            masterVolume = prefs[KEY_MASTER_VOLUME] ?: 0.7f,
            effectsVolume = prefs[KEY_EFFECTS_VOLUME] ?: 0.8f,
            ttsVolume = prefs[KEY_TTS_VOLUME] ?: 0.8f,
            animationSpeed = prefs[KEY_ANIMATION_SPEED] ?: 1.0f,
            temperature = prefs[KEY_TEMPERATURE] ?: 0.7f,
            responseLength = (prefs[KEY_RESPONSE_LENGTH] ?: 512f).toInt(),
            selectedModelId = prefs[KEY_SELECTED_MODEL] ?: "",
            ollamaBaseUrl = prefs[KEY_OLLAMA_URL] ?: "http://10.0.2.2:11434"
        )
    }

    suspend fun setDarkMode(value: Boolean) = put(KEY_DARK_MODE, value)
    suspend fun setNotifications(value: Boolean) = put(KEY_NOTIFICATIONS, value)
    suspend fun setSoundEffects(value: Boolean) = put(KEY_SOUND_EFFECTS, value)
    suspend fun setAutoStart(value: Boolean) = put(KEY_AUTO_START, value)
    suspend fun setStreamResponses(value: Boolean) = put(KEY_STREAM, value)
    suspend fun setShowCreatures(value: Boolean) = put(KEY_SHOW_CREATURES, value)
    suspend fun setAnimations(value: Boolean) = put(KEY_ANIMATIONS, value)
    suspend fun setCreatureSounds(value: Boolean) = put(KEY_CREATURE_SOUNDS, value)
    suspend fun setTtsEnabled(value: Boolean) = put(KEY_TTS, value)

    suspend fun setMasterVolume(value: Float) = putFloat(KEY_MASTER_VOLUME, value)
    suspend fun setEffectsVolume(value: Float) = putFloat(KEY_EFFECTS_VOLUME, value)
    suspend fun setTtsVolume(value: Float) = putFloat(KEY_TTS_VOLUME, value)
    suspend fun setAnimationSpeed(value: Float) = putFloat(KEY_ANIMATION_SPEED, value)
    suspend fun setTemperature(value: Float) = putFloat(KEY_TEMPERATURE, value)
    suspend fun setResponseLength(value: Int) = putFloat(KEY_RESPONSE_LENGTH, value.toFloat())

    suspend fun setSelectedModel(modelId: String) = put(KEY_SELECTED_MODEL, modelId)
    suspend fun setOllamaBaseUrl(url: String) = put(KEY_OLLAMA_URL, url)

    /** Apply a batch of changes in one transaction. */
    suspend fun update(transform: (AppSettings) -> AppSettings) {
        store.edit { prefs ->
            val current = AppSettings(
                darkMode = prefs[KEY_DARK_MODE] ?: false,
                notificationsEnabled = prefs[KEY_NOTIFICATIONS] ?: true,
                soundEffectsEnabled = prefs[KEY_SOUND_EFFECTS] ?: true,
                autoStart = prefs[KEY_AUTO_START] ?: false,
                streamResponses = prefs[KEY_STREAM] ?: true,
                showCreatures = prefs[KEY_SHOW_CREATURES] ?: true,
                animationsEnabled = prefs[KEY_ANIMATIONS] ?: true,
                creatureSoundsEnabled = prefs[KEY_CREATURE_SOUNDS] ?: true,
                ttsEnabled = prefs[KEY_TTS] ?: false,
                masterVolume = prefs[KEY_MASTER_VOLUME] ?: 0.7f,
                effectsVolume = prefs[KEY_EFFECTS_VOLUME] ?: 0.8f,
                ttsVolume = prefs[KEY_TTS_VOLUME] ?: 0.8f,
                animationSpeed = prefs[KEY_ANIMATION_SPEED] ?: 1.0f,
                temperature = prefs[KEY_TEMPERATURE] ?: 0.7f,
                responseLength = (prefs[KEY_RESPONSE_LENGTH] ?: 512f).toInt(),
                selectedModelId = prefs[KEY_SELECTED_MODEL] ?: "",
                ollamaBaseUrl = prefs[KEY_OLLAMA_URL] ?: "http://10.0.2.2:11434"
            )
            val next = transform(current)
            prefs[KEY_DARK_MODE] = next.darkMode
            prefs[KEY_NOTIFICATIONS] = next.notificationsEnabled
            prefs[KEY_SOUND_EFFECTS] = next.soundEffectsEnabled
            prefs[KEY_AUTO_START] = next.autoStart
            prefs[KEY_STREAM] = next.streamResponses
            prefs[KEY_SHOW_CREATURES] = next.showCreatures
            prefs[KEY_ANIMATIONS] = next.animationsEnabled
            prefs[KEY_CREATURE_SOUNDS] = next.creatureSoundsEnabled
            prefs[KEY_TTS] = next.ttsEnabled
            prefs[KEY_MASTER_VOLUME] = next.masterVolume
            prefs[KEY_EFFECTS_VOLUME] = next.effectsVolume
            prefs[KEY_TTS_VOLUME] = next.ttsVolume
            prefs[KEY_ANIMATION_SPEED] = next.animationSpeed
            prefs[KEY_TEMPERATURE] = next.temperature
            prefs[KEY_RESPONSE_LENGTH] = next.responseLength.toFloat()
            prefs[KEY_SELECTED_MODEL] = next.selectedModelId
            prefs[KEY_OLLAMA_URL] = next.ollamaBaseUrl
        }
    }

    private suspend fun put(key: Preferences.Key<Boolean>, value: Boolean) {
        store.edit { it[key] = value }
    }

    private suspend fun put(key: Preferences.Key<String>, value: String) {
        store.edit { it[key] = value }
    }

    private suspend fun putFloat(key: Preferences.Key<Float>, value: Float) {
        store.edit { it[key] = value }
    }

    private companion object {
        val KEY_DARK_MODE = booleanPreferencesKey("dark_mode")
        val KEY_NOTIFICATIONS = booleanPreferencesKey("notifications")
        val KEY_SOUND_EFFECTS = booleanPreferencesKey("sound_effects")
        val KEY_AUTO_START = booleanPreferencesKey("auto_start")
        val KEY_STREAM = booleanPreferencesKey("stream_responses")
        val KEY_SHOW_CREATURES = booleanPreferencesKey("show_creatures")
        val KEY_ANIMATIONS = booleanPreferencesKey("animations")
        val KEY_CREATURE_SOUNDS = booleanPreferencesKey("creature_sounds")
        val KEY_TTS = booleanPreferencesKey("tts_enabled")
        val KEY_MASTER_VOLUME = floatPreferencesKey("master_volume")
        val KEY_EFFECTS_VOLUME = floatPreferencesKey("effects_volume")
        val KEY_TTS_VOLUME = floatPreferencesKey("tts_volume")
        val KEY_ANIMATION_SPEED = floatPreferencesKey("animation_speed")
        val KEY_TEMPERATURE = floatPreferencesKey("temperature")
        val KEY_RESPONSE_LENGTH = floatPreferencesKey("response_length")
        val KEY_SELECTED_MODEL = stringPreferencesKey("selected_model")
        val KEY_OLLAMA_URL = stringPreferencesKey("ollama_base_url")
    }
}