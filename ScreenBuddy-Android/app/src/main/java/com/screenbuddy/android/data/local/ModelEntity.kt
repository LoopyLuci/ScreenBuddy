package com.screenbuddy.android.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "models")
data class ModelEntity(
    @PrimaryKey val id: String,
    val providerId: String,
    val name: String,
    val displayName: String,
    val isFree: Boolean = false,
    val isEnabled: Boolean = false,
    val description: String = "",
    val maxTokens: Int = 4096,
    val costPer1k: Double = 0.0
)
