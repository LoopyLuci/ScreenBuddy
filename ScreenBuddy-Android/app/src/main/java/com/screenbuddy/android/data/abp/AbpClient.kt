package com.screenbuddy.android.data.abp

import com.google.gson.Gson
import com.google.gson.JsonObject
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * The fleet's app-agnostic control surface (master plan v2, Pattern F).
 *
 * The extension routes were built to be driven by anything — they carry
 * no desktop-only assumptions — so the phone is a first-class remote:
 * creature status, asking the fleet, the router's recommendation, and the
 * fine-tune pipeline's state, all over the same scoped key the desktop
 * uses. The token is the dashboard token or a minted `screenbuddy`
 * integration key; the client never asks for more than it is given.
 *
 * Every call is honest about failure: a fleet that is unreachable (wrong
 * URL, desktop off, phone off the LAN) surfaces as a typed result the
 * screen shows, never a crash and never a fabricated answer.
 */
class AbpClient(
    private val baseUrl: String,
    private val token: String,
    private val client: OkHttpClient = defaultClient,
) {
    private val gson = Gson()
    private val json = "application/json; charset=utf-8".toMediaType()

    /** The creature's live snapshot: fps, uptime, creatures, models, tools, memory. */
    suspend fun status(): AbpResult<JsonObject> = get("/api/screenbuddy/status")

    /** The router's pick for a task, before the creature spends the turn. */
    suspend fun recommend(task: String): AbpResult<JsonObject> =
        post("/api/screenbuddy/recommend", mapOf("task" to task))

    /** Record one model-call outcome into the learning router. */
    suspend fun outcome(ref: String, ok: Boolean, taskClass: String? = null): AbpResult<JsonObject> =
        post(
            "/api/screenbuddy/outcome",
            buildMap {
                put("ref", ref)
                put("ok", ok)
                taskClass?.let { put("task_class", it) }
            },
        )

    /** Build the training dataset; trains only where the hardware allows. */
    suspend fun finetune(): AbpResult<JsonObject> = post("/api/screenbuddy/finetune", emptyMap())

    /** Dataset artifacts, training backend, adapter state. */
    suspend fun finetuneStatus(): AbpResult<JsonObject> = get("/api/screenbuddy/finetune/status")

    private suspend fun get(path: String): AbpResult<JsonObject> = withContext(Dispatchers.IO) {
        val request = Request.Builder()
            .url(baseUrl.trimEnd('/') + path)
            .header("X-Dashboard-Token", token)
            .get()
            .build()
        execute(request)
    }

    private suspend fun post(path: String, body: Map<String, Any?>): AbpResult<JsonObject> =
        withContext(Dispatchers.IO) {
            val payload = gson.toJson(body)
            val request = Request.Builder()
                .url(baseUrl.trimEnd('/') + path)
                .header("X-Dashboard-Token", token)
                .header("Content-Type", "application/json")
                .post(payload.toRequestBody(json))
                .build()
            execute(request)
        }

    private fun execute(request: Request): AbpResult<JsonObject> {
        try {
            client.newCall(request).execute().use { response ->
                val text = response.body?.string().orEmpty()
                val parsed = runCatching {
                    gson.fromJson(text, JsonObject::class.java)
                }.getOrNull()
                return if (response.isSuccessful && parsed != null) {
                    AbpResult.Ok(parsed)
                } else {
                    AbpResult.Error(
                        code = response.code,
                        message = parsed?.get("error")?.asString?.take(200)
                            ?: "fleet answered ${response.code}",
                    )
                }
            }
        } catch (exc: Exception) {
            // Wrong URL, desktop off, phone off the LAN — the screen shows
            // the reason; the app carries on.
            return AbpResult.Error(code = -1, message = exc.message ?: "unreachable")
        }
    }

    companion object {
        /** Generous but bounded: the fleet is a local network away. */
        val defaultClient: OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .writeTimeout(10, TimeUnit.SECONDS)
            .build()
    }
}

/** A fleet answer: parsed JSON, or the reason there isn't one. */
sealed class AbpResult<out T> {
    data class Ok<T>(val data: T) : AbpResult<T>()
    data class Error(val code: Int, val message: String) : AbpResult<Nothing>()
}
