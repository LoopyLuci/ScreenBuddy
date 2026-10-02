package com.screenbuddy.android.data.local

import androidx.room.*
import kotlinx.coroutines.flow.Flow

@Dao
interface ModelDao {
    @Query("SELECT * FROM models")
    fun getAllModels(): Flow<List<ModelEntity>>

    @Query("SELECT * FROM models WHERE providerId = :providerId")
    fun getModelsByProvider(providerId: String): Flow<List<ModelEntity>>

    @Query("SELECT * FROM models WHERE isFree = 1")
    fun getFreeModels(): Flow<List<ModelEntity>>

    @Query("SELECT * FROM models WHERE isEnabled = 1")
    fun getEnabledModels(): Flow<List<ModelEntity>>

    @Query("SELECT * FROM models WHERE providerId = :providerId AND isEnabled = 1")
    fun getEnabledModelsByProvider(providerId: String): Flow<List<ModelEntity>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertAll(models: List<ModelEntity>)

    @Update
    suspend fun update(model: ModelEntity)

    @Query("UPDATE models SET isEnabled = :enabled WHERE providerId = :providerId")
    suspend fun setAllModelsForProvider(providerId: String, enabled: Boolean)

    @Query("UPDATE models SET isEnabled = :enabled WHERE isFree = 0")
    suspend fun setAllPaidModels(enabled: Boolean)

    @Query("UPDATE models SET isEnabled = :enabled")
    suspend fun setAllModels(enabled: Boolean)

    @Query("SELECT COUNT(*) FROM models")
    suspend fun count(): Int
}
