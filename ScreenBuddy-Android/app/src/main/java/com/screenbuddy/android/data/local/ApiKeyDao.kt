package com.screenbuddy.android.data.local

import androidx.room.*
import kotlinx.coroutines.flow.Flow

@Dao
interface ApiKeyDao {
    @Query("SELECT * FROM api_keys")
    fun getAllApiKeys(): Flow<List<ApiKeyEntity>>

    @Query("SELECT * FROM api_keys WHERE providerId = :providerId")
    fun getApiKey(providerId: String): Flow<ApiKeyEntity?>

    @Query("SELECT * FROM api_keys WHERE isSet = 1")
    fun getSetApiKeys(): Flow<List<ApiKeyEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(apiKey: ApiKeyEntity)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(apiKeys: List<ApiKeyEntity>)

    @Update
    suspend fun update(apiKey: ApiKeyEntity)

    @Delete
    suspend fun delete(apiKey: ApiKeyEntity)

    @Query("DELETE FROM api_keys WHERE providerId = :providerId")
    suspend fun deleteByProvider(providerId: String)

    @Query("UPDATE api_keys SET isSet = 0")
    suspend fun clearAllKeys()
}
