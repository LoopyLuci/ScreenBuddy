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

class AiService(
    private val ollamaBaseUrl: String = "http://10.0.2.2:11434"
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
        try {
            val response = when {
                model.providerId.contains("ollama") -> callOllama(messages, model, systemPrompt)
                model.providerId.contains("openai") || model.providerId.contains("grok") -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callOpenAiCompatible(messages, model, apiKey, systemPrompt, "https://api.openai.com/v1")
                }
                model.providerId.contains("anthropic") -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callAnthropic(messages, model, apiKey, systemPrompt)
                }
                model.providerId.contains("openrouter") -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callOpenRouter(messages, model, apiKey, systemPrompt)
                }
                model.providerId.contains("together") -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callTogether(messages, model, apiKey, systemPrompt)
                }
                model.providerId.contains("deepseek") -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callDeepSeek(messages, model, apiKey, systemPrompt)
                }
                else -> {
                    if (apiKey.isNullOrBlank()) {
                        return@withContext Result.failure(IllegalArgumentException("API key required for ${model.providerId}. Please set your API key in Settings."))
                    }
                    callGeneric(messages, model, apiKey, systemPrompt)
                }
            }
            Result.success(response)
        } catch (e: Exception) {
            Result.failure(e)
        }
    }

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
            obj.addProperty("role", msg.role)
            obj.addProperty("content", msg.content)
            array.add(obj)
        }
        return array
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

    private fun callOllama(messages: List<ChatMessage>, model: AiModel, systemPrompt: String?): ChatMessage {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.add("messages", buildMessagesJson(messages, systemPrompt))
        json.addProperty("stream", false)

        val request = Request.Builder()
            .url("$ollamaBaseUrl/api/chat")
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        val response = client.newCall(request).execute()

        if (!response.isSuccessful) {
            val errorBody = response.body?.string() ?: "HTTP ${response.code}"
            throw IOException("Ollama error (${response.code}): ${parseErrorResponse(errorBody)}")
        }

        val body = response.body?.string() ?: throw IOException("Empty response from Ollama")

        return try {
            val parsed = gson.fromJson(body, JsonObject::class.java)
            val content = parsed.getAsJsonObject("message")?.get("content")?.asString
                ?: throw IOException("Unexpected response format from Ollama")

            ChatMessage(
                id = System.currentTimeMillis().toString(),
                role = "assistant",
                content = content,
                timestamp = System.currentTimeMillis(),
                providerUsed = model.providerId
            )
        } catch (e: JsonSyntaxException) {
            throw IOException("Failed to parse Ollama response: ${e.message}")
        }
    }

    private fun callOpenAiCompatible(
        messages: List<ChatMessage>,
        model: AiModel,
        apiKey: String,
        systemPrompt: String?,
        baseUrl: String
    ): ChatMessage {
        val json = JsonObject()
        json.addProperty("model", model.name)
        json.add("messages", buildMessagesJson(messages, systemPrompt))
        json.addProperty("stream", false)

        val request = Request.Builder()
            .url("$baseUrl/chat/completions")
            .addHeader("Authorization", "Bearer $apiKey")
            .addHeader("Content-Type", "application/json")
            .post(json.toString().toRequestBody("application/json".toMediaType()))
            .build()

        val response = client.newCall(request).execute()

        if (!response.isSuccessful) {
            val errorBody = response.body?.string() ?: "HTTP ${response.code}"
            throw IOException("API error (${response.code}): ${parseErrorResponse(errorBody)}")
        }

        val body = response.body?.string() ?: throw IOException("Empty response from API")

        return try {
            val parsed = gson.fromJson(body, JsonObject::class.java)
            val choices = parsed.getAsJsonArray("choices")

            if (choices == null || choices.size() == 0) {
                throw IOException("No choices in response from API")
            }

            val message = choices[0].asJsonObject.getAsJsonObject("message")
                ?: throw IOException("No message in response from API")

            val content = message.get("content")?.asString
                ?: throw IOException("No content in response from API")

            ChatMessage(
                id = System.currentTimeMillis().toString(),
                role = "assistant",
                content = content,
                timestamp = System.currentTimeMillis(),
                providerUsed = model.providerId
            )
        } catch (e: JsonSyntaxException) {
            throw IOException("Failed to parse API response: ${e.message}")
        }
    }

    private fun callAnthropic(messages: List<ChatMessage>, model: AiModel, apiKey: String, systemPrompt: String?): ChatMessage {
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

        val response = client.newCall(request).execute()

        if (!response.isSuccessful) {
            val errorBody = response.body?.string() ?: "HTTP ${response.code}"
            throw IOException("Anthropic error (${response.code}): ${parseErrorResponse(errorBody)}")
        }

        val body = response.body?.string() ?: throw IOException("Empty response from Anthropic")

        return try {
            val parsed = gson.fromJson(body, JsonObject::class.java)
            val contentArray = parsed.getAsJsonArray("content")

            if (contentArray == null || contentArray.size() == 0) {
                throw IOException("No content in response from Anthropic")
            }

            val content = contentArray[0].asJsonObject?.get("text")?.asString
                ?: throw IOException("Unexpected response format from Anthropic")

            ChatMessage(
                id = System.currentTimeMillis().toString(),
                role = "assistant",
                content = content,
                timestamp = System.currentTimeMillis(),
                providerUsed = model.providerId
            )
        } catch (e: JsonSyntaxException) {
            throw IOException("Failed to parse Anthropic response: ${e.message}")
        }
    }

    private fun callOpenRouter(messages: List<ChatMessage>, model: AiModel, apiKey: String, systemPrompt: String?): ChatMessage {
        return callOpenAiCompatible(messages, model, apiKey, systemPrompt, "https://openrouter.ai/api/v1")
    }

    private fun callTogether(messages: List<ChatMessage>, model: AiModel, apiKey: String, systemPrompt: String?): ChatMessage {
        return callOpenAiCompatible(messages, model, apiKey, systemPrompt, "https://api.together.xyz/v1")
    }

    private fun callDeepSeek(messages: List<ChatMessage>, model: AiModel, apiKey: String, systemPrompt: String?): ChatMessage {
        return callOpenAiCompatible(messages, model, apiKey, systemPrompt, "https://api.deepseek.com/v1")
    }

    private fun callGeneric(messages: List<ChatMessage>, model: AiModel, apiKey: String, systemPrompt: String?): ChatMessage {
        return callOpenAiCompatible(messages, model, apiKey, systemPrompt, "https://api.openai.com/v1")
    }
}
