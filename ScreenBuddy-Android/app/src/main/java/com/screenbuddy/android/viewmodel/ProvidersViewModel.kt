package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.ModelDao
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.data.repository.ProviderRepository
import com.screenbuddy.android.service.AiService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch

data class ProvidersUiState(
    val providers: List<Provider> = emptyList(),
    val isLoading: Boolean = true,
    val error: String? = null,
    val selectedProvider: Provider? = null,
    /** Provider ids with a stored key, so the UI can show a "configured" badge. */
    val providersWithKeys: Set<String> = emptySet(),
    /** Base URL each provider will actually be called on. */
    val endpoints: Map<String, String> = emptyMap()
)

class ProvidersViewModel(
    private val apiKeyDao: ApiKeyDao,
    modelDao: ModelDao,
    private val aiService: AiService = AiService()
) : ViewModel() {

    private val repository = ProviderRepository(apiKeyDao, modelDao)

    private val _uiState = MutableStateFlow(ProvidersUiState())
    val uiState: StateFlow<ProvidersUiState> = _uiState.asStateFlow()

    init {
        viewModelScope.launch {
            runCatching { repository.seedIfEmpty() }
                .onFailure { e ->
                    _uiState.value = _uiState.value.copy(
                        error = "Could not initialise the model catalogue: ${e.message}",
                        isLoading = false
                    )
                }
        }

        combine(repository.providers, repository.providersWithKeys()) { providers, withKeys ->
            providers to withKeys
        }
            .onEach { (providers, withKeys) ->
                _uiState.value = _uiState.value.copy(
                    providers = providers,
                    providersWithKeys = withKeys.toSet(),
                    endpoints = providers.associate { provider ->
                        provider.id to (aiService.endpointFor(provider.id) ?: provider.name)
                    },
                    isLoading = false
                )
            }
            .launchIn(viewModelScope)
    }

    fun selectProvider(provider: Provider) {
        _uiState.value = _uiState.value.copy(selectedProvider = provider)
    }

    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    fun toggleModel(modelId: String, enabled: Boolean) = mutate {
        repository.toggleModel(modelId, enabled)
    }

    fun enableAllFreeModels(providerId: String) = mutate {
        repository.enableAllFreeModels(providerId)
    }

    fun disableAllPaidModels(providerId: String) = mutate {
        repository.disableAllPaidModels(providerId)
    }

    fun enableAllModels(providerId: String) = mutate {
        repository.enableAllModels(providerId)
    }

    fun disableAllModels(providerId: String) = mutate {
        repository.disableAllModels(providerId)
    }

    fun setApiKey(providerId: String, keyValue: String) = mutate {
        repository.setApiKey(providerId, keyValue)
    }

    fun removeApiKey(providerId: String) = mutate {
        repository.removeApiKey(providerId)
    }

    /**
     * Run a repository mutation, surfacing any failure in [ProvidersUiState.error]
     * instead of letting it escape into the coroutine scope.
     */
    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch {
            runCatching { block() }.onFailure { e ->
                _uiState.value = _uiState.value.copy(error = e.message ?: "Operation failed")
            }
        }
    }
}