package com.screenbuddy.android.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.google.gson.JsonObject
import com.screenbuddy.android.data.abp.AbpRepository
import com.screenbuddy.android.data.abp.AbpResult
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * Backs the Fleet screen (master plan v2, Pattern F): the phone's window
 * on the desktop fleet. Every state is explicit — loading, the fleet's
 * answer, or the reason there isn't one — so the screen never shows a
 * placeholder number it did not actually receive.
 */
class FleetViewModel(
    private val abpRepository: AbpRepository,
) : ViewModel() {

    private val _status = MutableStateFlow<AbpResult<JsonObject>?>(null)
    val status: StateFlow<AbpResult<JsonObject>?> = _status.asStateFlow()

    private val _recommendation = MutableStateFlow<String?>(null)
    val recommendation: StateFlow<String?> = _recommendation.asStateFlow()

    private val _finetune = MutableStateFlow<AbpResult<JsonObject>?>(null)
    val finetune: StateFlow<AbpResult<JsonObject>?> = _finetune.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    /** Pull the creature's live snapshot from the fleet. */
    fun refreshStatus() = call {
        _status.value = abpRepository.status()
    }

    /** Ask the router which model it would pick for [task]. */
    fun recommend(task: String) = call {
        when (val r = abpRepository.recommend(task)) {
            is AbpResult.Ok -> _recommendation.value = r.data.toString()
            is AbpResult.Error -> _recommendation.value = "fleet error: ${r.message}"
        }
    }

    /** Read the fine-tune pipeline's state (datasets, backend, adapter). */
    fun refreshFinetune() = call {
        _finetune.value = abpRepository.finetuneStatus()
    }

    /** Run one call with the busy flag set, so the UI can disable itself. */
    private fun call(block: suspend () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            try {
                block()
            } catch (exc: Exception) {
                _status.value = AbpResult.Error(-1, exc.message ?: "unexpected error")
            } finally {
                _busy.value = false
            }
        }
    }
}
