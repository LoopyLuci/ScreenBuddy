package com.screenbuddy.android.data.repository

import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.KeyCipher
import com.screenbuddy.android.data.local.ApiKeyEntity
import com.screenbuddy.android.data.local.ModelDao
import com.screenbuddy.android.data.local.ModelEntity
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.data.model.ProviderData
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

/**
 * Single source of truth for providers, models, and API keys.
 *
 * [ProviderData] is the static catalogue and the seed for the database. User
 * changes live in Room so they survive process death: only the mutable flag
 * (model enablement) and API keys are persisted. Reads merge the persisted
 * enablement back onto the catalogue, so providers keep the static metadata
 * (descriptions, costs) that we never need to store.
 */
class ProviderRepository(
    private val apiKeyDao: ApiKeyDao,
    private val modelDao: ModelDao,
    /**
     * Encrypts keys at rest. Defaults to an unavailable cipher, which stores
     * plaintext, so unit tests on the JVM work without a Keystore.
     */
    private val cipher: KeyCipher = KeyCipher()
) {
    /** Providers with live enable/disable state merged in from the database. */
    val providers: Flow<List<Provider>> =
        modelDao.getAllModels().map { persisted -> overlayEnabled(persisted) }

    /** Populate the model table on first run. No-op once seeded. */
    suspend fun seedIfEmpty() {
        if (modelDao.count() == 0) {
            modelDao.insertAll(
                ProviderData.providers.flatMap { provider ->
                    provider.models.map { it.toEntity(provider.id) }
                }
            )
        }
    }

    fun getProvider(providerId: String): Provider? =
        ProviderData.providers.find { it.id == providerId }

    fun getFreeModels(): List<AiModel> =
        ProviderData.providers.flatMap { it.models.filter { m -> m.isFree } }

    /** Models enabled by default in the catalogue, for first-run selection. */
    fun getEnabledModels(): List<AiModel> =
        ProviderData.providers.flatMap { provider ->
            provider.models.filter { it.isEnabled }.map { it.copy(providerId = provider.id) }
        }

    /** True when the provider has a non-blank key stored. */
    suspend fun hasApiKey(providerId: String): Boolean =
        apiKeyDao.getApiKey(providerId).first()?.keyValue?.isNotBlank() == true

    /**
     * The stored key in plaintext, or null when unset or blank.
     *
     * Decryption is transparent: rows written before encryption was enabled are
     * returned as-is, and rows written by a device without a usable Keystore
     * still read back correctly.
     */
    suspend fun getApiKey(providerId: String): String? =
        apiKeyDao.getApiKey(providerId).first()
            ?.keyValue
            ?.takeIf { it.isNotBlank() }
            ?.let { cipher.decrypt(it) }
            ?.takeIf { it.isNotBlank() }

    /** True when [providerId] has a usable stored key. */
    suspend fun hasStoredKey(providerId: String): Boolean = getApiKey(providerId) != null

    suspend fun setApiKey(providerId: String, apiKey: String) {
        val trimmed = apiKey.trim()
        // Encrypt when the platform allows it; fall back to plaintext otherwise
        // so a missing Keystore degrades rather than losing the key.
        val stored = if (cipher.isAvailable) cipher.encrypt(trimmed) ?: trimmed else trimmed
        apiKeyDao.insert(
            ApiKeyEntity(
                providerId = providerId,
                keyValue = stored,
                isSet = trimmed.isNotEmpty()
            )
        )
    }

    suspend fun removeApiKey(providerId: String) {
        apiKeyDao.deleteByProvider(providerId)
    }

    suspend fun clearAllApiKeys() {
        apiKeyDao.deleteAll()
    }

    /** Provider ids that currently have a key set, for the settings screen. */
    fun providersWithKeys(): Flow<List<String>> =
        apiKeyDao.getSetApiKeys()
            .map { rows -> rows.filter { it.keyValue.isNotBlank() }.map { it.providerId } }

    suspend fun toggleModel(modelId: String, enabled: Boolean) {
        val entity = modelDao.getAllModels().first().firstOrNull { it.id == modelId } ?: return
        modelDao.update(entity.copy(isEnabled = enabled))
    }

    suspend fun enableAllFreeModels(providerId: String) {
        ProviderData.providers.find { it.id == providerId }?.models
            ?.filter { it.isFree }
            ?.forEach { toggleModel(it.id, true) }
    }

    suspend fun disableAllPaidModels(providerId: String) {
        ProviderData.providers.find { it.id == providerId }?.models
            ?.filterNot { it.isFree }
            ?.forEach { toggleModel(it.id, false) }
    }

    suspend fun enableAllModels(providerId: String) {
        modelDao.setAllModelsForProvider(providerId, true)
    }

    suspend fun disableAllModels(providerId: String) {
        modelDao.setAllModelsForProvider(providerId, false)
    }

    /** Merge persisted enablement onto the static catalogue. */
    private fun overlayEnabled(persisted: List<ModelEntity>): List<Provider> {
        if (persisted.isEmpty()) return ProviderData.providers
        val byId = persisted.associateBy { it.id }
        return ProviderData.providers.map { provider ->
            provider.copy(
                models = provider.models.map { model ->
                    byId[model.id]?.let { model.copy(isEnabled = it.isEnabled) } ?: model
                }
            )
        }
    }
}

/**
 * The provider that owns a catalogue model may differ from the model's own
 * `providerId` field, so the owning provider is passed explicitly when seeding.
 */
private fun AiModel.toEntity(owningProviderId: String) = ModelEntity(
    id = id,
    providerId = owningProviderId,
    name = name,
    displayName = displayName,
    isFree = isFree,
    isEnabled = isEnabled,
    description = description,
    maxTokens = maxTokens,
    costPer1k = costPer1k
)