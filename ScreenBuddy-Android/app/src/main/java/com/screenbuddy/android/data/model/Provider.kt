package com.screenbuddy.android.data.model

data class Provider(
    val id: String,
    val name: String,
    val description: String,
    val apiKeyName: String,
    val website: String,
    val isFree: Boolean = false,
    val models: List<AiModel> = emptyList(),
    val apiKey: ApiKey? = null
)
