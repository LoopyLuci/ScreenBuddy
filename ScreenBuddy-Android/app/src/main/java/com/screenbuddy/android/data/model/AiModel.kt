package com.screenbuddy.android.data.model

data class AiModel(
    val id: String,
    val providerId: String,
    val name: String,
    val displayName: String,
    val isFree: Boolean = false,
    val isEnabled: Boolean = false,
    val description: String = "",
    val maxTokens: Int = 4096,
    val costPer1k: Double = 0.0
)
