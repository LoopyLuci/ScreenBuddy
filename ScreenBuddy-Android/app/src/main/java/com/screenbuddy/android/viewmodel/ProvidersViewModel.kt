package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.data.repository.ProviderRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class ProvidersUiState(
    val providers: List<Provider> = emptyList(),
    val isLoading: Boolean = true,
    val error: String? = null,
    val selectedProvider: Provider? = null
)

class ProvidersViewModel(
    private val repository: ProviderRepository = ProviderRepository()
) : ViewModel() {

    private val _uiState = MutableStateFlow(ProvidersUiState())
    val uiState: StateFlow<ProvidersUiState> = _uiState.asStateFlow()

    init {
        loadProviders()
    }

    private fun loadProviders() {
        viewModelScope.launch {
            repository.providers.collect { providers ->
                _uiState.value = _uiState.value.copy(
                    providers = providers,
                    isLoading = false
                )
            }
        }
    }

    fun selectProvider(provider: Provider) {
        _uiState.value = _uiState.value.copy(selectedProvider = provider)
    }

    fun toggleModel(modelId: String, enabled: Boolean) {
        repository.toggleModel(modelId, enabled)
    }

    fun enableAllFreeModels(providerId: String) {
        repository.enableAllFreeModels(providerId)
    }

    fun disableAllPaidModels(providerId: String) {
        repository.disableAllPaidModels(providerId)
    }

    fun enableAllModels(providerId: String) {
        repository.enableAllModels(providerId)
    }

    fun disableAllModels(providerId: String) {
        repository.disableAllModels(providerId)
    }

    fun setApiKey(providerId: String, keyValue: String) {
        repository.setApiKey(providerId, keyValue)
    }

    fun removeApiKey(providerId: String) {
        repository.removeApiKey(providerId)
    }
}
