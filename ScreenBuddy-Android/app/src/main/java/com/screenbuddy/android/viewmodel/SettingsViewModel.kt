package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.ModelDao
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.repository.ProviderRepository
import com.screenbuddy.android.data.repository.SettingsRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

data class SettingsUiState(
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
    val ollamaBaseUrl: String = "http://10.0.2.2:11434",
    val enabledModels: List<AiModel> = emptyList(),
    val selectedModelId: String = "",
    val providersWithKeys: Set<String> = emptySet(),
    val versionName: String = "0.1.0"
) {
    val selectedModel: AiModel?
        get() = enabledModels.firstOrNull { it.id == selectedModelId } ?: enabledModels.firstOrNull()
}

/**
 * Backs the settings screen. Every control writes through to
 * [SettingsRepository] (DataStore) so changes survive process death; the previous
 * version used no-op lambdas that discarded everything.
 */
class SettingsViewModel(
    private val settingsRepository: SettingsRepository,
    apiKeyDao: ApiKeyDao,
    modelDao: ModelDao,
    versionName: String
) : ViewModel() {

    private val providerRepository = ProviderRepository(apiKeyDao, modelDao)

    private val _uiState = MutableStateFlow(SettingsUiState(versionName = versionName))
    val uiState: StateFlow<SettingsUiState> = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            providerRepository.seedIfEmpty()
            // Seed enablement comes from the model table, not the static defaults,
            // so the picker reflects whatever the user toggled in Providers.
            val enabled = providerRepository.providers.first()
                .flatMap { provider -> provider.models.filter { it.isEnabled }.map { it.copy(providerId = provider.id) } }
            val withKeys = providerRepository.providersWithKeys().first().toSet()
            _uiState.value = _uiState.value.copy(
                enabledModels = enabled,
                providersWithKeys = withKeys
            )
        }

        viewModelScope.launch {
            settingsRepository.settings.collect { s ->
                _uiState.value = _uiState.value.copy(
                    darkMode = s.darkMode,
                    notificationsEnabled = s.notificationsEnabled,
                    soundEffectsEnabled = s.soundEffectsEnabled,
                    autoStart = s.autoStart,
                    streamResponses = s.streamResponses,
                    showCreatures = s.showCreatures,
                    animationsEnabled = s.animationsEnabled,
                    creatureSoundsEnabled = s.creatureSoundsEnabled,
                    ttsEnabled = s.ttsEnabled,
                    masterVolume = s.masterVolume,
                    effectsVolume = s.effectsVolume,
                    ttsVolume = s.ttsVolume,
                    animationSpeed = s.animationSpeed,
                    temperature = s.temperature,
                    responseLength = s.responseLength,
                    ollamaBaseUrl = s.ollamaBaseUrl,
                    selectedModelId = s.selectedModelId.ifEmpty { _uiState.value.selectedModel?.id ?: "" }
                )
            }
        }
    }

    fun setDarkMode(v: Boolean) = mutate { settingsRepository.setDarkMode(v) }
    fun setNotifications(v: Boolean) = mutate { settingsRepository.setNotifications(v) }
    fun setSoundEffects(v: Boolean) = mutate { settingsRepository.setSoundEffects(v) }
    fun setAutoStart(v: Boolean) = mutate { settingsRepository.setAutoStart(v) }
    fun setStreamResponses(v: Boolean) = mutate { settingsRepository.setStreamResponses(v) }
    fun setShowCreatures(v: Boolean) = mutate { settingsRepository.setShowCreatures(v) }
    fun setAnimations(v: Boolean) = mutate { settingsRepository.setAnimations(v) }
    fun setCreatureSounds(v: Boolean) = mutate { settingsRepository.setCreatureSounds(v) }
    fun setTtsEnabled(v: Boolean) = mutate { settingsRepository.setTtsEnabled(v) }
    fun setMasterVolume(v: Float) = mutate { settingsRepository.setMasterVolume(v) }
    fun setEffectsVolume(v: Float) = mutate { settingsRepository.setEffectsVolume(v) }
    fun setTtsVolume(v: Float) = mutate { settingsRepository.setTtsVolume(v) }
    fun setAnimationSpeed(v: Float) = mutate { settingsRepository.setAnimationSpeed(v) }
    fun setTemperature(v: Float) = mutate { settingsRepository.setTemperature(v) }
    fun setResponseLength(v: Int) = mutate { settingsRepository.setResponseLength(v) }
    fun setSelectedModel(v: String) = mutate { settingsRepository.setSelectedModel(v) }
    fun setOllamaBaseUrl(v: String) = mutate { settingsRepository.setOllamaBaseUrl(v.trim()) }

    fun clearAllApiKeys() = mutate { providerRepository.clearAllApiKeys() }

    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch { runCatching { block() } }
    }
}