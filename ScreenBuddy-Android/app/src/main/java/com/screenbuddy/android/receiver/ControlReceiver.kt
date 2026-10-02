package com.screenbuddy.android.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.screenbuddy.android.data.control.ControlBus
import com.screenbuddy.android.data.control.ControlRequest
import com.screenbuddy.android.data.control.ControlResponse

/**
 * Accepts control commands from outside the app.
 *
 * The Android counterpart to the desktop's TCP control port. A phone
 * should not listen on a socket, so commands arrive as broadcasts instead.
 * Name the component explicitly, which is the form verified to work:
 *
 *   adb shell am broadcast -a com.screenbuddy.android.CONTROL \
 *       -n com.screenbuddy.android/.receiver.ControlReceiver --es cmd get_status
 *
 * The implicit form (same command without -n) depends on implicit-broadcast
 * resolution and was not delivered on a Nokia 7.2 / Android 11 device, so the
 * explicit component is the documented path.
 *
 * Extras map onto [ControlRequest] by name, so a command reads the same as it
 * does over the desktop socket:
 *
 *   adb shell am broadcast -a com.screenbuddy.android.CONTROL \
 *       --es cmd set_animation --es state celebrate
 *
 * The outcome is written to logcat under [TAG] and mirrored into the broadcast
 * result code, so a caller can tell success from failure without parsing.
 */
class ControlReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        Log.i(TAG, "onReceive action=${intent.action} extras=${intent.extras?.keySet()}")
        if (intent.action != ACTION_CONTROL) return

        // Every extra is read defensively: `--ez`/`--ei` deliver real Booleans
        // and Integers, and getStringExtra throws ClassCastException on those.
        val request = try {
            parse(intent)
        } catch (t: Throwable) {
            Log.e(TAG, "could not parse intent", t)
            setResultCode(RESULT_ERROR)
            setResultData("bad request: ${t.message}")
            return
        }

        if (request.cmd.isBlank()) {
            setResultCode(RESULT_ERROR)
            setResultData("missing 'cmd' extra")
            return
        }

        val response = ControlBus.submit(request)
        Log.i(TAG, "${request.cmd} -> ${response.status} ${ResponseCodec.encode(response)}")

        if (response.ok) {
            setResultCode(RESULT_OK)
        } else {
            setResultCode(RESULT_ERROR)
            setResultData(response.message ?: "error")
        }
    }

    private fun parse(intent: Intent) = ControlRequest(
        cmd = str(intent, "cmd").orEmpty(),
        id = str(intent, "id"),
        name = str(intent, "name"),
        state = str(intent, "state"),
        x = str(intent, "x")?.toFloatOrNull(),
        y = str(intent, "y")?.toFloatOrNull(),
        visible = bool(intent, "visible"),
        enabled = bool(intent, "enabled"),
        message = str(intent, "message"),
        prompt = str(intent, "prompt"),
        key = str(intent, "key"),
        value = str(intent, "value"),
        source = str(intent, "source"),
        content = str(intent, "content"),
        query = str(intent, "query"),
        limit = str(intent, "limit")?.toIntOrNull(),
    )

    /** Read an extra as text whatever type it was sent as. */
    private fun str(intent: Intent, key: String): String? =
        intent.extras?.get(key)?.toString()

    /** Accept both `--ez true` and `--es true`; unparseable means absent. */
    private fun bool(intent: Intent, key: String): Boolean? = when (val raw = intent.extras?.get(key)) {
        null -> null
        is Boolean -> raw
        else -> raw.toString().toBooleanStrictOrNull()
    }

    companion object {
        const val ACTION_CONTROL = "com.screenbuddy.android.CONTROL"
        const val TAG = "ScreenBuddyControl"
        const val RESULT_OK = 0
        const val RESULT_ERROR = 1
    }
}

/** Minimal JSON encoding, so this path needs no serialization dependency. */
object ResponseCodec {
    fun encode(response: ControlResponse): String {
        val data = response.data
            ?.entries
            ?.joinToString(",") { (k, v) -> "\"$k\":${value(v)}" }
            ?.let { ",$it" }
            .orEmpty()
        val message = response.message?.let { ",\"message\":${value(it)}" }.orEmpty()
        return "{\"status\":${value(response.status)}$message$data}"
    }

    private fun value(v: Any?): String = when (v) {
        null -> "null"
        is Number -> v.toString()
        is Boolean -> v.toString()
        is Map<*, *> -> v.entries.joinToString(",", "{", "}") { (k, inner) -> "\"$k\":${value(inner)}" }
        is List<*> -> v.joinToString(",", "[", "]") { value(it) }
        else -> "\"${v.toString().replace("\"", "\\\"")}\""
    }
}
