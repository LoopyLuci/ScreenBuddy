package com.screenbuddy.android.data.repository

import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.ProviderData
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

class ProviderRepository {
    
    private val _providers = MutableStateFlow(ProviderData.providers)
    val providers: Flow<List<Provider>> = _providers.asStateFlow()

    fun getProvider(providerId: String): Provider? = ProviderData.providers.find { it.id == providerId }

    fun getFreeModels(): List<AiModel> = ProviderData.providers.flatMap { it.models.filter { m -> m.isFree } }

    fun getEnabledModels(): List<AiModel> = ProviderData.providers.flatMap { it.models.filter { m -> m.isEnabled } }

    fun toggleModel(modelId: String, enabled: Boolean) {
        _providers.value = _providers.value.map { provider ->
            provider.copy(
                models = provider.models.map { model ->
                    if (model.id == modelId) model.copy(isEnabled = enabled) else model
                }
            )
        }
    }

    fun enableAllFreeModels(providerId: String) {
        _providers.value = _providers.value.map { provider ->
            if (provider.id == providerId) {
                provider.copy(
                    models = provider.models.map { model ->
                        if (model.isFree) model.copy(isEnabled = true) else model
                    }
                )
            } else provider
        }
    }

    fun disableAllPaidModels(providerId: String) {
        _providers.value = _providers.value.map { provider ->
            if (provider.id == providerId) {
                provider.copy(
                    models = provider.models.map { model ->
                        if (!model.isFree) model.copy(isEnabled = false) else model
                    }
                )
            } else provider
        }
    }

    fun enableAllModels(providerId: String) {
        _providers.value = _providers.value.map { provider ->
            if (provider.id == providerId) {
                provider.copy(
                    models = provider.models.map { model -> model.copy(isEnabled = true) }
                )
            } else provider
        }
    }

    fun disableAllModels(providerId: String) {
        _providers.value = _providers.value.map { provider ->
            if (provider.id == providerId) {
                provider.copy(
                    models = provider.models.map { model -> model.copy(isEnabled = false) }
                )
            } else provider
        }
    }

    fun setApiKey(providerId: String, apiKey: String) {
        // In a real app, this would be encrypted and stored
    }

    fun removeApiKey(providerId: String) {
        // In a real app, this would remove the key
    }
}
