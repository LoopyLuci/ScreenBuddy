package com.screenbuddy.android.data.model

data class ApiKey(
    val providerId: String,
    val keyValue: String,
    val isSet: Boolean = false,
    val lastValidated: Long? = null
)
