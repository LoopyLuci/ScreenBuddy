package com.screenbuddy.android.service

import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.ChatMessage
import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import com.google.gson.JsonSyntaxException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

/** Wire format a provider speaks. */
enum class ApiStyle { OLLAMA, OPENAI_COMPATIBLE, ANTHROPIC }

/**
 * How each provider is reached.
 *
 * [baseUrl] is the exact host that receives the credential, so routing is
 * explicit per provider and a key can never be sent to a different vendor.
 */
enum class ProviderRoute(
    val providerId: String,
    val baseUrl: String,
    val apiStyle: ApiStyle,
    val requiresKey: Boolean = true
) {
    OLLAMA("ollama", "http://10.0.2.2:11434", ApiStyle.OLLAMA, requiresKey = false),
    OPENAI("openai", "https://api.openai.com/v1", ApiStyle.OPENAI_COMPATIBLE),
    ANTHROPIC("anthropic", "https://api.anthropic.com", ApiStyle.ANTHROPIC),
    OPENROUTER("openrouter", "https://openrouter.ai/api/v1", ApiStyle.OPENAI_COMPATIBLE),
    TOGETHER("together", "https://api.together.xyz/v1", ApiStyle.OPENAI_COMPATIBLE),
    DEEPSEEK("deepseek", "https://api.deepseek.com/v1", ApiStyle.OPENAI_COMPATIBLE),
    MISTRAL("mistral", "https://api.mistral.ai/v1", ApiStyle.OPENAI_COMPATIBLE),
    GROK("grok", "https://api.x.ai/v1", ApiStyle.OPENAI_COMPATIBLE),
    UNSLOTH("unsloth", "https://api.unsloth.ai/v1", ApiStyle.OPENAI_COMPATIBLE),
    OPENCODE_ZEN("opencode-zen", "https://opencode.ai/zen/v1", ApiStyle.OPENAI_COMPATIBLE),
    COHERE("cohere", "https://api.cohere.ai/v1", ApiStyle.OPENAI_COMPATIBLE),

    /** Perplexity serves an OpenAI-shaped /chat/completions endpoint. */
    PERPLEXITY("perplexity", "https://api.perplexity.ai", ApiStyle.OPENAI_COMPATIBLE),

    /** Gemini via Google's OpenAI-compatibility layer. */
    GOOGLE("google", "https://generativelanguage.googleapis.com/v1beta/openai", ApiStyle.OPENAI_COMPATIBLE),

    /** Hugging Face Inference router, OpenAI-compatible. */
    HUGGINGFACE("huggingface", "https://router.huggingface.co/v1", ApiStyle.OPENAI_COMPATIBLE),

    REPLICATE("replicate", "https://api.replicate.com/v1", ApiStyle.OPENAI_COMPATIBLE),

    /**
     * Self-hosted OpenAI-compatible servers. [baseUrl] is empty on purpose:
     * these must be configured by the user, never guessed, and they normally
     * need no credential.
     */
    LLAMACPP("llamacpp", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    LMSTUDIO("lmstudio", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    LLMSTUDIO("llmstudio", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    VLLM("vllm", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    KOBOLDCPP("koboldcpp", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    TEXTGEN("textgen", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false),
    OPENCODE_GO("opencode-go", "", ApiStyle.OPENAI_COMPATIBLE, requiresKey = false);

    companion object {
        private val byId = entries.associateBy { it.providerId }

        /** Exact match on the provider id. */
        fun forProvider(providerId: String): ProviderRoute? = byId[providerId]

        /**
         * Resolve a provider id, falling back to the longest id that is a
         * substring of it so compound ids ("opencode-zen") beat shorter ones.
         */
        fun resolve(providerId: String): ProviderRoute? =
            byId[providerId] ?: entries
                .filter { it.providerId.isNotEmpty() }
                .sortedByDescending { it.providerId.length }
                .firstOrNull { providerId.contains(it.providerId) }
    }
}

class AiService(
    private val ollamaBaseUrl: String = "http://10.0.2.2:11434",
    /** Per-provider base URL overrides, used for self-hosted endpoints. */
    private val baseUrlOverrides: Map<String, String> = emptyMap()
) {
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .build()

    private val gson = Gson()

    suspend fun sendMessage(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String?,
        systemPrompt: String? = null
    ): Result<ChatMessage> = withContext(Dispatchers.IO) {
        val route = ProviderRoute.resolve(model.providerId)
            ?: return@withContext Result.failure(
                IllegalArgumentException("Unknown provider '${model.providerId}'.")
            )

        val baseUrl = baseUrlOverrides[route.providerId]
            ?: route.baseUrl.takeIf { it.isNotEmpty() }
            ?: ollamaBaseUrl

        if (route.requiresKey && apiKey.isNullOrBlank()) {
            return@withContext Result.failure(
                IllegalArgumentException(
                    "API key required for ${model.providerId}. Add one in the Providers tab."
                )
            )
        }

        try {
            val response = when (route.apiStyle) {
                ApiStyle.OLLAMA -> callOllama(messages, model, systemPrompt, baseUrl)
                ApiStyle.ANTHROPIC -> callAnthropic(messages, model, apiKey!!, systemPrompt)
                ApiStyle.OPENAI_COMPATIBLE ->
                    callOpenAiCompatible(messages, model, apiKey, systemPrompt, baseUrl)
            }
            Result.success(response)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    /**
     * One tool-aware turn: either text or tool calls.
     *
     * This is what [com.screenbuddy.android.data.agent.AgentLoop] consumes. Tool
     * calls are only meaningful on the OpenAI-shaped endpoints; Ollama and
     * Anthropic reply with text here and the loop finishes on the first turn.
     */
    suspend fun sendToolTurn(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String?,
        toolSchemas: com.google.gson.JsonArray,
        systemPrompt: String? = null
    ): Result<com.screenbuddy.android.data.agent.AgentLoop.Turn> = withContext(Dispatchers.IO) {
        val route = ProviderRoute.resolve(model.providerId)
            ?: return@withContext Result.failure(
                IllegalArgumentException("Unknown provider '${model.providerId}'.")
            )
        val baseUrl = baseUrlOverrides[route.providerId]
            ?: route.baseUrl.takeIf { it.isNotEmpty() }
            ?: ollamaBaseUrl

        if (route.requiresKey && apiKey.isNullOrBlank()) {
            return@withContext Result.failure(
                IllegalArgumentException(
                    "API key required for ${model.providerId}. Add one in the Providers tab."
                )
            )
        }

        try {
            // Only the OpenAI-compatible dialect can express tool calls here.
            val turn = if (route.apiStyle == ApiStyle.OPENAI_COMPATIBLE) {
                callOpenAiToolTurn(messages, model, apiKey, systemPrompt, baseUrl, toolSchemas)
            } else {
                com.screenbuddy.android.data.agent.AgentLoop.Turn(
                    text = when (route.apiStyle) {
                        ApiStyle.OLLAMA -> callOllama(messages, model, systemPrompt, baseUrl).content
                        else -> callAnthropic(messages, model, apiKey!!, systemPrompt).content
                    },
                    toolCalls = emptyList()
                )
            }
            Result.success(turn)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

    private fun callOpenAiToolTurn(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String?,
        systemPrompt: String?,
        baseUrl: String,
        toolSchemas: com.google.gson.JsonArray
    ): com.screenbuddy.android.data.agent.AgentLoop.Turn {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.add("messages", buildMessagesJson(messages, systemPrompt))
        json.addProperty("stream", false)
        if (model.maxTokens > 0) {
            json.addProperty("max_tokens", model.maxTokens)
        }
        if (toolSchemas.size() > 0) {
            json.add("tools", toolSchemas)
        }

        val builder = Request.Builder()
            .url("${baseUrl.trimEnd('/')}/chat/completions")
            .addHeader("Content-Type", "application/json")
        if (!apiKey.isNullOrBlank()) {
            builder.addHeader("Authorization", "Bearer $apiKey")
        }

        val request = builder
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                val errorBody = response.body?.string() ?: "HTTP ${response.code}"
                throw IOException(
                    "${model.providerId} error (${response.code}): ${parseErrorResponse(errorBody)}"
                )
            }
            val body = response.body?.string()
                ?: throw IOException("Empty response from ${model.providerId}")
            val parsed = gson.fromJson(body, JsonObject::class.java)
            val message = parsed.getAsJsonArray("choices")
                ?.get(0)?.asJsonObject
                ?.getAsJsonObject("message")
                ?: throw IOException("No message in response from ${model.providerId}")

            val calls = parseOpenAiToolCalls(parsed)
            val text = message.get("content")?.takeIf { !it.isJsonNull }?.asString ?: ""
            return com.screenbuddy.android.data.agent.AgentLoop.Turn(text, calls)
        }
    }

    /** The endpoint [providerId] would be called on, for display in the UI. */
    fun endpointFor(providerId: String): String? = ProviderRoute.resolve(providerId)?.let {
        baseUrlOverrides[it.providerId]?.takeIf(String::isNotEmpty) ?: it.baseUrl
    }

    /** True when [providerId] needs a credential before it can be used. */
    fun requiresApiKey(providerId: String): Boolean =
        ProviderRoute.resolve(providerId)?.requiresKey != false

    private fun buildMessagesJson(messages: List<ChatMessage>, systemPrompt: String?): JsonArray {
        val array = JsonArray()
        systemPrompt?.let {
            val sysMsg = JsonObject()
            sysMsg.addProperty("role", "system")
            sysMsg.addProperty("content", it)
            array.add(sysMsg)
        }
        messages.forEach { msg ->
            val obj = JsonObject()
            when (msg.role) {
                // A tool result must carry tool_call_id to bind it to its call.
                "tool" -> {
                    obj.addProperty("role", "tool")
                    obj.addProperty("tool_call_id", msg.toolCallId ?: "")
                    obj.addProperty("content", msg.content)
                }
                else -> {
                    obj.addProperty("role", msg.role)
                    obj.addProperty("content", msg.content)
                }
            }
            array.add(obj)
        }
        return array
    }

    /**
     * Parse `choices[0].message.tool_calls`.
     *
     * Arguments arrive as a JSON string on OpenAI and an object on Ollama, so
     * both are accepted. Returns an empty list for text-only responses.
     */
    internal fun parseOpenAiToolCalls(
        parsed: JsonObject
    ): List<com.screenbuddy.android.data.agent.ToolCall> {
        val message = parsed.getAsJsonArray("choices")
            ?.get(0)?.asJsonObject
            ?.getAsJsonObject("message")
            ?: return emptyList()
        val raw = message.getAsJsonArray("tool_calls") ?: return emptyList()

        return raw.mapIndexedNotNull { index, element ->
            val call = element.asJsonObject
            val function = call.getAsJsonObject("function") ?: return@mapIndexedNotNull null
            val name = function.get("name")?.asString ?: return@mapIndexedNotNull null
            val arguments = when (val rawArgs = function.get("arguments")) {
                null -> JsonObject()
                // OpenAI sends a JSON string; Ollama sends an object.
                is com.google.gson.JsonPrimitive -> runCatching {
                    com.google.gson.JsonParser.parseString(rawArgs.asString).asJsonObject
                }.getOrElse { JsonObject() }
                else -> rawArgs.asJsonObject
            }
            com.screenbuddy.android.data.agent.ToolCall(
                id = call.get("id")?.asString ?: "call_$index",
                name = name,
                arguments = arguments
            )
        }
    }

    private fun parseErrorResponse(body: String): String {
        return try {
            val parsed = gson.fromJson(body, JsonObject::class.java)
            parsed.getAsJsonObject("error")?.get("message")?.asString
                ?: parsed.get("error")?.asString
                ?: parsed.get("message")?.asString
                ?: body.take(200)
        } catch (e: JsonSyntaxException) {
            body.take(200)
        }
    }

    private fun assistantMessage(content: String, model: AiModel) = ChatMessage(
        id = System.currentTimeMillis().toString(),
        role = "assistant",
        content = content,
        timestamp = System.currentTimeMillis(),
        providerUsed = model.providerId
    )

    private fun callOllama(
        messages: List<ChatMessage>,
        model: AiModel,
        systemPrompt: String?,
        baseUrl: String
    ): ChatMessage {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.add("messages", buildMessagesJson(messages, systemPrompt))
        json.addProperty("stream", false)

        val request = Request.Builder()
            .url("${baseUrl.trimEnd('/')}/api/chat")
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                val errorBody = response.body?.string() ?: "HTTP ${response.code}"
                throw IOException("Ollama error (${response.code}): ${parseErrorResponse(errorBody)}")
            }
            val body = response.body?.string() ?: throw IOException("Empty response from Ollama")
            try {
                val parsed = gson.fromJson(body, JsonObject::class.java)
                val content = parsed.getAsJsonObject("message")?.get("content")?.asString
                    ?: throw IOException("Unexpected response format from Ollama")
                return assistantMessage(content, model)
            } catch (e: JsonSyntaxException) {
                throw IOException("Failed to parse Ollama response: ${e.message}")
            }
        }
    }

    private fun callOpenAiCompatible(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String?,
        systemPrompt: String?,
        baseUrl: String
    ): ChatMessage {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.add("messages", buildMessagesJson(messages, systemPrompt))
        json.addProperty("stream", false)
        if (model.maxTokens > 0) {
            json.addProperty("max_tokens", model.maxTokens)
        }

        val builder = Request.Builder()
            .url("${baseUrl.trimEnd('/')}/chat/completions")
            .addHeader("Content-Type", "application/json")
        // Self-hosted servers normally need no auth; only send the header when a
        // key exists so we never emit an empty "Bearer " to a local endpoint.
        if (!apiKey.isNullOrBlank()) {
            builder.addHeader("Authorization", "Bearer $apiKey")
        }

        val request = builder
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                val errorBody = response.body?.string() ?: "HTTP ${response.code}"
                throw IOException(
                    "${model.providerId} error (${response.code}): ${parseErrorResponse(errorBody)}"
                )
            }
            val body = response.body?.string()
                ?: throw IOException("Empty response from ${model.providerId}")
            try {
                val parsed = gson.fromJson(body, JsonObject::class.java)
                val choices = parsed.getAsJsonArray("choices")
                if (choices == null || choices.size() == 0) {
                    throw IOException("No choices in response from ${model.providerId}")
                }
                val message = choices[0].asJsonObject.getAsJsonObject("message")
                    ?: throw IOException("No message in response from ${model.providerId}")
                val content = message.get("content")?.asString
                    ?: throw IOException("No content in response from ${model.providerId}")
                return assistantMessage(content, model)
            } catch (e: JsonSyntaxException) {
                throw IOException("Failed to parse ${model.providerId} response: ${e.message}")
            }
        }
    }

    private fun callAnthropic(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String,
        systemPrompt: String?
    ): ChatMessage {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.addProperty("max_tokens", model.maxTokens.coerceAtMost(4096))
        systemPrompt?.let { json.addProperty("system", it) }

        val userMessages = JsonArray()
        messages.forEach { msg ->
            val obj = JsonObject()
            obj.addProperty("role", if (msg.role == "user") "user" else "assistant")
            obj.addProperty("content", msg.content)
            userMessages.add(obj)
        }
        json.add("messages", userMessages)

        val request = Request.Builder()
            .url("https://api.anthropic.com/v1/messages")
            .addHeader("x-api-key", apiKey)
            .addHeader("anthropic-version", "2023-06-01")
            .addHeader("Content-Type", "application/json")
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                val errorBody = response.body?.string() ?: "HTTP ${response.code}"
                throw IOException("Anthropic error (${response.code}): ${parseErrorResponse(errorBody)}")
            }
            val body = response.body?.string() ?: throw IOException("Empty response from Anthropic")
            try {
                val parsed = gson.fromJson(body, JsonObject::class.java)
                val contentArray = parsed.getAsJsonArray("content")
                if (contentArray == null || contentArray.size() == 0) {
                    throw IOException("No content in response from Anthropic")
                }
                // Thinking blocks may precede the text blocks; take all text.
                val content = contentArray
                    .mapNotNull { it.asJsonObject?.get("text")?.asString }
                    .joinToString("\n")
                    .ifBlank { throw IOException("Unexpected response format from Anthropic") }
                return assistantMessage(content, model)
            } catch (e: JsonSyntaxException) {
                throw IOException("Failed to parse Anthropic response: ${e.message}")
            }
        }
    }
}