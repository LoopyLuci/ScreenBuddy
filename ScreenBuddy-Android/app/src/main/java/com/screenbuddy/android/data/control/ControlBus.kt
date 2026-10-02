package com.screenbuddy.android.data.control

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * A control request, mirroring the desktop IPC command set.
 *
 * Command names match the desktop `ControlRequest` exactly so an agent can drive
 * either client with the same vocabulary.
 */
data class ControlRequest(
    val cmd: String,
    val id: String? = null,
    val name: String? = null,
    val state: String? = null,
    val x: Float? = null,
    val y: Float? = null,
    val visible: Boolean? = null,
    val enabled: Boolean? = null,
    val message: String? = null,
    val prompt: String? = null,
    val key: String? = null,
    val value: String? = null,
    val source: String? = null,
    val content: String? = null,
    val query: String? = null,
    val limit: Int? = null,
)

/** The reply shape, matching the desktop `Response` envelope. */
data class ControlResponse(
    val status: String,
    val message: String? = null,
    val data: Map<String, Any?>? = null,
) {
    val ok: Boolean get() = status == "success"

    companion object {
        fun ok(data: Map<String, Any?> = emptyMap()) = ControlResponse("success", data = data)
        fun queued(what: String) = ControlResponse("success", data = mapOf("queued" to what))
        fun error(message: String, code: Int = 400) =
            ControlResponse("error", message, mapOf("code" to code))
    }
}

/** Observable runtime state, the Android equivalent of the desktop RuntimeStatus. */
data class ControlStatus(
    val selectedCreatureId: String? = null,
    val visible: Boolean = true,
    val animationState: String = "idle",
    val autoCycle: Boolean = true,
    val chatHistory: List<String> = emptyList(),
    val settings: Map<String, String> = emptyMap(),
    val memoryChunks: Int = 0,
    val agentToolCount: Int = 0,
)

/**
 * In-process control bus.
 *
 * The desktop exposes TCP because another process drives it. On Android the
 * equivalent is an in-process bus plus [com.screenbuddy.android.receiver.ControlReceiver],
 * which accepts the same commands over `adb shell am broadcast` and deep links.
 * That keeps a single command vocabulary across both clients without opening a
 * listening socket on a device that is usually on a personal network.
 */
object ControlBus {

    private val _requests = MutableSharedFlow<ControlRequest>(replay = 0, extraBufferCapacity = 64)
    val requests: SharedFlow<ControlRequest> = _requests.asSharedFlow()

    private val _responses = MutableSharedFlow<ControlResponse>(replay = 0, extraBufferCapacity = 64)
    val responses: SharedFlow<ControlResponse> = _responses.asSharedFlow()

    private val _status = MutableStateFlow(ControlStatus())
    val status: StateFlow<ControlStatus> = _status.asStateFlow()

    /** Registered handlers, invoked when a request arrives. */
    private val handlers = mutableMapOf<String, (ControlRequest) -> ControlResponse>()

    @Synchronized
    fun register(cmd: String, handler: (ControlRequest) -> ControlResponse) {
        handlers[cmd] = handler
    }

    @Synchronized
    fun unregister(cmd: String) {
        handlers.remove(cmd)
    }

    /** Dispatch a request to its handler and publish the response. */
    fun dispatch(request: ControlRequest): ControlResponse {
        val handler = synchronized(this) { handlers[request.cmd] }
        val response = if (handler == null) {
            ControlResponse.error("unsupported command: ${request.cmd}", 404)
        } else {
            try {
                handler(request)
            } catch (t: Throwable) {
                ControlResponse.error("${request.cmd} failed: ${t.message}", 500)
            }
        }
        _responses.tryEmit(response)
        return response
    }

    /** Convenience for callers that only want to fire and forget. */
    fun submit(request: ControlRequest): ControlResponse = dispatch(request)

    fun updateStatus(transform: (ControlStatus) -> ControlStatus) {
        _status.value = transform(_status.value)
    }

    /** Test seam: drop handlers and reset status between tests. */
    @Synchronized
    fun reset() {
        handlers.clear()
        _status.value = ControlStatus()
    }
}
