package com.screenbuddy.android.data.model

data class ChatMessage(
    val id: String,
    val role: String, // "user", "assistant", "system"
    val content: String,
    val timestamp: Long,
    val providerUsed: String? = null
)
