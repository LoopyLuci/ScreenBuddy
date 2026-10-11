package com.screenbuddy.android.service

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

/** Status of a 9Router instance. */
data class NineRouterStatus(
    val running: Boolean = false,
    val version: String = "",
    val providerCount: Int = 0,
    val modelCount: Int = 0,
    val activeProvider: String = "",
    val baseUrl: String = "",
)

/** A model available through 9Router. */
data class NineRouterModel(
    val id: String,
    val name: String,
    val provider: String,
    val isFree: Boolean = false,
    val isEnabled: Boolean = true,
)

/**
 * Service for managing a local 9Router instance.
 *
 * 9Router is a smart AI router that proxies 60+ providers through a single
 * OpenAI-compatible endpoint. This service provides:
 * - Status reporting (running, version, providers, models)
 * - Model discovery (list available models from the API)
 * - Connection testing
 * - Start/stop lifecycle management
 * - API key management
 *
 * The service communicates with the 9Router HTTP API at the configured
 * base URL (default: http://localhost:20128/v1).
 */
class NineRouterService(
    initialBaseUrl: String = "http://localhost:20128/v1",
    initialApiKey: String = ""
) {
    @Volatile
    var baseUrl: String = initialBaseUrl
        private set

    @Volatile
    var apiKey: String = initialApiKey
        private set

    @Volatile
    var isRunning: Boolean = false
        private set

    private val client = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(10, TimeUnit.SECONDS)
        .build()

    private val gson = Gson()

    fun configure(baseUrl: String, apiKey: String) {
        this.baseUrl = baseUrl.trimEnd('/')
        this.apiKey = apiKey
    }

    /** Test connectivity to the 9Router instance. */
    suspend fun testConnection(): Boolean = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url("${baseUrl.trimEnd('/')}/models")
                .addHeader("Content-Type", "application/json")
                .apply {
                    if (apiKey.isNotBlank()) {
                        addHeader("Authorization", "Bearer $apiKey")
                    }
                }
                .get()
                .build()
            client.newCall(request).execute().use { response ->
                response.isSuccessful
            }
        } catch (e: Exception) {
            false
        }
    }

    /** Fetch the list of available models from 9Router. */
    suspend fun getModels(): List<NineRouterModel> = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url("${baseUrl.trimEnd('/')}/models")
                .addHeader("Content-Type", "application/json")
                .apply {
                    if (apiKey.isNotBlank()) {
                        addHeader("Authorization", "Bearer $apiKey")
                    }
                }
                .get()
                .build()
            client.newCall(request).execute().use { response ->
                if (!response.isSuccessful) return@withContext emptyList()
                val body = response.body?.string() ?: return@withContext emptyList()
                parseModels(body)
            }
        } catch (e: Exception) {
            emptyList()
        }
    }

    /** Get current status of the 9Router instance. */
    suspend fun getStatus(): NineRouterStatus = withContext(Dispatchers.IO) {
        val reachable = testConnection()
        if (!reachable) {
            return@withContext NineRouterStatus(
                running = false,
                baseUrl = baseUrl
            )
        }
        val models = getModels()
        NineRouterStatus(
            running = true,
            providerCount = models.map { it.provider }.distinct().size,
            modelCount = models.size,
            activeProvider = models.firstOrNull()?.provider ?: "",
            baseUrl = baseUrl
        )
    }

    /** Start 9Router (placeholder — actual start is host-side). */
    suspend fun start(): Boolean {
        // 9Router runs as a separate process on the host machine.
        // This method tests connectivity and updates local state.
        isRunning = testConnection()
        return isRunning
    }

    /** Stop 9Router (placeholder — actual stop is host-side). */
    suspend fun stop(): Boolean {
        isRunning = false
        return true
    }

    /** Health check — lightweight connectivity test. */
    suspend fun healthCheck(): Map<String, Any> = withContext(Dispatchers.IO) {
        val reachable = testConnection()
        mapOf(
            "healthy" to reachable,
            "baseUrl" to baseUrl,
            "timestamp" to System.currentTimeMillis()
        )
    }

    private fun parseModels(body: String): List<NineRouterModel> {
        return try {
            val parsed = gson.fromJson(body, JsonObject::class.java)
            val data = parsed.getAsJsonArray("data") ?: return emptyList()
            data.mapNotNull { element ->
                val obj = element.asJsonObject
                val id = obj.get("id")?.asString ?: return@mapNotNull null
                NineRouterModel(
                    id = id,
                    name = id,
                    provider = id.takeIf { it.contains("/") }?.substringBefore("/") ?: "unknown",
                    isFree = id.contains(":free", ignoreCase = true),
                    isEnabled = true
                )
            }
        } catch (e: Exception) {
            emptyList()
        }
    }
}
