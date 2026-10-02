package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.screenbuddy.android.data.agent.AgentLoop
import com.screenbuddy.android.data.agent.ToolCall
import com.screenbuddy.android.data.local.ApiKeyDao
import com.screenbuddy.android.data.local.ModelDao
import com.screenbuddy.android.data.repository.ProviderRepository
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.ChatMessage
import com.screenbuddy.android.data.rag.RagPipeline
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
    val inputText: String = "",
    /** Human-readable progress while the agent loop runs. */
    val agentStatus: String? = null,
    /** Set when RAG context was injected into the last request. */
    val usedMemory: Boolean = false
)

/** Providers that need no credential, so chat works before any key is set. */
private val KEYLESS_PROVIDERS = setOf(
    "ollama", "llamacpp", "lmstudio", "llmstudio", "vllm", "koboldcpp", "textgen", "opencode-go"
)

class ChatViewModel(
    private val aiService: AiService,
    private val apiKeyDao: ApiKeyDao,
    modelDao: ModelDao? = null,
    private val rag: RagPipeline? = null,
    private val agentTools: List<com.screenbuddy.android.data.agent.Tool> = AgentLoop.defaultTools()
) : ViewModel() {

    /**
     * Keys are read through the repository so they are decrypted; going straight
     * to the DAO would hand ciphertext to the HTTP layer.
     *
     * A null [modelDao] means no catalogue access is possible (unit tests), in
     * which case keys are fetched directly and are known to be plaintext.
     */
    private val repository: ProviderRepository? =
        modelDao?.let { ProviderRepository(apiKeyDao, it) }

    private val _uiState = MutableStateFlow(ChatUiState())
    val uiState: StateFlow<ChatUiState> = _uiState.asStateFlow()

    fun setModel(model: AiModel) {
        _uiState.value = _uiState.value.copy(selectedModel = model, error = null)
    }

    fun updateInput(text: String) {
        _uiState.value = _uiState.value.copy(inputText = text)
    }

    fun sendMessage() {
        val text = _uiState.value.inputText.trim()
        if (text.isEmpty()) return
        val model = _uiState.value.selectedModel
        if (model == null) {
            _uiState.value = _uiState.value.copy(error = "Pick a model before sending a message.")
            return
        }
        if (_uiState.value.isLoading) return

        val userMessage = ChatMessage(
            id = System.currentTimeMillis().toString(),
            role = "user",
            content = text,
            timestamp = System.currentTimeMillis(),
            providerUsed = model.providerId
        )

        val newMessages = _uiState.value.messages + userMessage
        // Retrieved memory is prepended as context so the model can ground its
        // answer; it is not shown as a chat bubble.
        val context = rag?.buildContext(text).orEmpty()
        val outgoing = if (context.isBlank()) {
            newMessages
        } else {
            listOf(
                userMessage.copy(
                    content = "$context\n\n---\n\n$text"
                )
            ) + newMessages.dropLast(1)
        }

        _uiState.value = _uiState.value.copy(
            messages = newMessages,
            inputText = "",
            isLoading = true,
            error = null,
            usedMemory = context.isNotBlank()
        )

        viewModelScope.launch {
            val apiKey = apiKeyFor(model.providerId)
            val result = runAgent(outgoing, model, apiKey)
            result.fold(
                onSuccess = { response ->
                    _uiState.value = _uiState.value.copy(
                        messages = _uiState.value.messages + response,
                        isLoading = false,
                        agentStatus = null
                    )
                },
                onFailure = { error ->
                    _uiState.value = _uiState.value.copy(
                        error = error.message ?: "An unknown error occurred",
                        isLoading = false,
                        agentStatus = null
                    )
                }
            )
        }
    }

    /**
     * Run the tool-calling loop. Providers that cannot express tool calls simply
     * return a text-only turn on the first iteration.
     */
    private suspend fun runAgent(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String?
    ): Result<ChatMessage> {
        val loop = AgentLoop(maxIterations = MAX_ITERATIONS, tools = agentTools)
        var agentOutput: String? = null

        val turn = loop.run(
            initial = messages,
            model = { convo, schemas ->
                val result = aiService.sendToolTurn(
                    messages = convo,
                    model = model,
                    apiKey = apiKey,
                    toolSchemas = schemas,
                    systemPrompt = SYSTEM_PROMPT
                )
                result.getOrElse { throw it }
            },
            events = { event ->
                when (event) {
                    is AgentLoop.Event.Thinking ->
                        setAgentStatus("Thinking (step ${event.step}/${event.total})")
                    is AgentLoop.Event.ToolUse -> setAgentStatus("Using ${event.name}")
                    is AgentLoop.Event.ToolResult -> agentOutput = event.output
                    is AgentLoop.Event.Final -> setAgentStatus(null)
                    is AgentLoop.Event.Failure -> setAgentStatus(null)
                }
            }
        )

        // Surface the tool exchange when the model used a tool, so the user can
        // see what happened rather than only the final text.
        val content = if (agentOutput != null && turn.isBlank()) {
            "[tool] $agentOutput"
        } else {
            turn
        }

        return Result.success(
            ChatMessage(
                id = System.currentTimeMillis().toString(),
                role = "assistant",
                content = content,
                timestamp = System.currentTimeMillis(),
                providerUsed = model.providerId
            )
        )
    }

    /** Surface tool activity as a chat bubble. */
    fun publishToolActivity(name: String, output: String) {
        _uiState.value = _uiState.value.copy(
            messages = _uiState.value.messages + ChatMessage(
                id = System.currentTimeMillis().toString(),
                role = "tool",
                content = output,
                timestamp = System.currentTimeMillis(),
                toolName = name
            )
        )
    }

    private fun setAgentStatus(status: String?) {
        _uiState.value = _uiState.value.copy(agentStatus = status)
    }

    fun clearMessages() {
        _uiState.value = _uiState.value.copy(messages = emptyList(), error = null)
    }

    fun dismissError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    /**
     * Fetch the stored key for a provider. Returns null for providers that do not
     * need one; a stored-but-blank key is treated as absent so the user gets the
     * "API key required" message rather than an auth failure.
     */
    private suspend fun apiKeyFor(providerId: String): String? {
        if (KEYLESS_PROVIDERS.any { providerId.contains(it) }) return null
        repository?.let { return it.getApiKey(providerId) }
        return apiKeyDao.getApiKey(providerId).first()
            ?.keyValue
            ?.takeIf { it.isNotBlank() }
    }

    private companion object {
        const val MAX_ITERATIONS = 4
        const val SYSTEM_PROMPT =
            "You are ScreenBuddy, a friendly AI companion. Keep responses concise and helpful."
    }
}