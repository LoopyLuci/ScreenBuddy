package com.screenbuddy.android.data.abp

import com.google.gson.JsonObject
import com.screenbuddy.android.data.repository.SettingsRepository
import kotlinx.coroutines.flow.first

/**
 * The phone's window on the fleet (master plan v2, Pattern F).
 *
 * The URL and token come from settings — the user points the phone at
 * the desktop's LAN address and pastes a dashboard token or a minted
 * `screenbuddy` integration key (the same one the desktop extension
 * README describes). Nothing is hardcoded: an unconfigured fleet is a
 * screen that says so, not a broken app.
 *
 * Every call is one-shot: it reads the current settings, builds a client
 * scoped to them, and returns the fleet's honest answer — parsed JSON or
 * the reason there isn't one.
 */
class AbpRepository(private val settingsRepository: SettingsRepository) {

    /** The creature's live snapshot: fps, uptime, creatures, models, tools, memory. */
    suspend fun status(): AbpResult<JsonObject> =
        client().status()

    /** The router's recommendation for [task]. */
    suspend fun recommend(task: String): AbpResult<JsonObject> =
        client().recommend(task)

    /** Record an outcome into the learning router. */
    suspend fun outcome(ref: String, ok: Boolean, taskClass: String? = null): AbpResult<JsonObject> =
        client().outcome(ref, ok, taskClass)

    /** The fine-tune pipeline's state (datasets, backend, adapter). */
    suspend fun finetuneStatus(): AbpResult<JsonObject> =
        client().finetuneStatus()

    private suspend fun client(): AbpClient {
        val settings = settingsRepository.settings.first()
        return AbpClient(baseUrl = settings.fleetBaseUrl, token = settings.fleetToken)
    }
}
