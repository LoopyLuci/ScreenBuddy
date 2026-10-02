package com.screenbuddy.android.data.local

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "api_keys")
data class ApiKeyEntity(
    @PrimaryKey val providerId: String,
    val keyValue: String = "",
    val isSet: Boolean = false,
    val lastValidated: Long? = null,
    val lastUsed: Long? = null
)
