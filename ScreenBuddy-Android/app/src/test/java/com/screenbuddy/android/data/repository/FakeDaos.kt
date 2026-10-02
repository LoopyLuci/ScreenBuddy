package com.screenbuddy.android.data.repository

import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.ApiKeyEntity
import com.screenbuddy.android.data.local.ModelDao
import com.screenbuddy.android.data.local.ModelEntity
import com.screenbuddy.android.data.model.ProviderData
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map

/**
 * In-memory stand-ins for the Room DAOs.
 *
 * Unit tests run on the JVM without instrumentation, and `ApplicationProvider`
 * requires a registered instrumentation (or Robolectric, which is a heavy
 * dependency for what these fakes cover). These implement the same contract the
 * DAO queries express, including the `Flow` updates that Room would emit.
 *
 * The equivalent queries are exercised against a real in-memory database by the
 * instrumented tests under `src/androidTest`.
 */
class FakeApiKeyDao : ApiKeyDao {
    private val rows = MutableStateFlow<Map<String, ApiKeyEntity>>(emptyMap())

    override fun getAllApiKeys(): Flow<List<ApiKeyEntity>> =
        rows.map { it.values.toList() }

    override fun getApiKey(providerId: String): Flow<ApiKeyEntity?> =
        rows.map { it[providerId] }

    override fun getSetApiKeys(): Flow<List<ApiKeyEntity>> =
        rows.map { it.values.filter { e -> e.isSet }.toList() }

    override suspend fun insert(apiKey: ApiKeyEntity) {
        rows.value = rows.value + (apiKey.providerId to apiKey)
    }

    override suspend fun insertAll(apiKeys: List<ApiKeyEntity>) {
        rows.value = rows.value + apiKeys.associateBy { it.providerId }
    }

    override suspend fun update(apiKey: ApiKeyEntity) {
        insert(apiKey)
    }

    override suspend fun delete(apiKey: ApiKeyEntity) {
        rows.value = rows.value - apiKey.providerId
    }

    override suspend fun deleteByProvider(providerId: String) {
        rows.value = rows.value - providerId
    }

    override suspend fun clearAllKeys() {
        rows.value = rows.value.mapValues { (_, e) -> e.copy(isSet = false) }
    }

    override suspend fun deleteAll() {
        rows.value = emptyMap()
    }
}

class FakeModelDao : ModelDao {
    private val rows = MutableStateFlow<Map<String, ModelEntity>>(emptyMap())

    override fun getAllModels(): Flow<List<ModelEntity>> =
        rows.map { it.values.toList() }

    override fun getModelsByProvider(providerId: String): Flow<List<ModelEntity>> =
        rows.map { m -> m.values.filter { it.providerId == providerId }.toList() }

    override fun getFreeModels(): Flow<List<ModelEntity>> =
        rows.map { m -> m.values.filter { it.isFree }.toList() }

    override fun getEnabledModels(): Flow<List<ModelEntity>> =
        rows.map { m -> m.values.filter { it.isEnabled }.toList() }

    override fun getEnabledModelsByProvider(providerId: String): Flow<List<ModelEntity>> =
        rows.map { m -> m.values.filter { it.providerId == providerId && it.isEnabled }.toList() }

    override suspend fun insertAll(models: List<ModelEntity>) {
        rows.value = rows.value + models.associateBy { it.id }
    }

    override suspend fun update(model: ModelEntity) {
        rows.value = rows.value + (model.id to model)
    }

    override suspend fun setAllModelsForProvider(providerId: String, enabled: Boolean) {
        rows.value = rows.value.mapValues { (_, e) ->
            if (e.providerId == providerId) e.copy(isEnabled = enabled) else e
        }
    }

    override suspend fun setAllPaidModels(enabled: Boolean) {
        rows.value = rows.value.mapValues { (_, e) ->
            if (!e.isFree) e.copy(isEnabled = enabled) else e
        }
    }

    override suspend fun setAllModels(enabled: Boolean) {
        rows.value = rows.value.mapValues { (_, e) -> e.copy(isEnabled = enabled) }
    }

    override suspend fun count(): Int = rows.value.size

    /** Test helper: the raw persisted rows. */
    fun snapshot(): List<ModelEntity> = rows.value.values.toList()
}

/** Convenience: the full expected seed, matching what seedIfEmpty writes. */
fun expectedSeedSize(): Int = ProviderData.providers.sumOf { it.models.size }