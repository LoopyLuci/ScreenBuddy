package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.AppDatabase
import com.screenbuddy.android.data.model.ChatMessage
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.service.AiService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

data class ChatUiState(
    val messages: List<ChatMessage> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
    val selectedModel: AiModel? = null,
    val inputText: String = ""
)

class ChatViewModel(
    private val aiService: AiService,
    private val apiKeyDao: ApiKeyDao
) : ViewModel() {

    private val _uiState = MutableStateFlow(ChatUiState())
    val uiState: StateFlow<ChatUiState> = _uiState.asStateFlow()

    fun setModel(model: AiModel) {
        _uiState.value = _uiState.value.copy(selectedModel = model)
    }

    fun updateInput(text: String) {
        _uiState.value = _uiState.value.copy(inputText = text)
    }

    fun sendMessage() {
        val text = _uiState.value.inputText.trim()
        if (text.isEmpty()) return
        val model = _uiState.value.selectedModel ?: return

        val userMessage = ChatMessage(
            id = System.currentTimeMillis().toString(),
            role = "user",
            content = text,
            timestamp = System.currentTimeMillis(),
            providerUsed = model.providerId
        )

        val newMessages = _uiState.value.messages + userMessage
        _uiState.value = _uiState.value.copy(
            messages = newMessages,
            inputText = "",
            isLoading = true,
            error = null
        )

        viewModelScope.launch {
            val apiKey = getApiKeyForProvider(model.providerId)
            val result = aiService.sendMessage(
                messages = newMessages,
                model = model,
                apiKey = apiKey,
                systemPrompt = "You are ScreenBuddy, a friendly AI assistant on Android. Keep responses concise and helpful."
            )

            result.fold(
                onSuccess = { response ->
                    _uiState.value = _uiState.value.copy(
                        messages = _uiState.value.messages + response,
                        isLoading = false
                    )
                },
                onFailure = { error ->
                    _uiState.value = _uiState.value.copy(
                        error = error.message ?: "An unknown error occurred",
                        isLoading = false
                    )
                }
            )
        }
    }

    fun clearMessages() {
        _uiState.value = _uiState.value.copy(messages = emptyList())
    }

    fun dismissError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    private suspend fun getApiKeyForProvider(providerId: String): String? {
        return if (providerId.contains("ollama")) {
            null // Ollama does not require an API key
        } else {
            val entity = apiKeyDao.getApiKey(providerId).first()
            entity?.keyValue?.takeIf { it.isNotEmpty() }
        }
    }
}
