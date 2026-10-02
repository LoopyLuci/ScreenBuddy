package com.screenbuddy.android.data.model

/**
 * One turn in a conversation.
 *
 * `toolCallId`/`toolName` are set on `role == "tool"` messages so a result can
 * be matched to the call that produced it, which OpenAI-shaped endpoints require
 * via `tool_call_id`.
 */
data class ChatMessage(
    val id: String,
    val role: String, // "user", "assistant", "system", "tool"
    val content: String,
    val timestamp: Long,
    val providerUsed: String? = null,
    val toolCallId: String? = null,
    val toolName: String? = null
)