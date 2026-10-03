package com.screenbuddy.android.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.screenbuddy.android.ScreenBuddyApp
import com.screenbuddy.android.data.control.ControlBus

/**
 * Brings the app back after a reboot when the user asked for it.
 *
 * The `auto_start` setting existed and `RECEIVE_BOOT_COMPLETED` was declared in
 * the manifest, but no receiver was registered, so the toggle did nothing. This
 * is the receiver that makes it true.
 *
 * Two things it deliberately does not do:
 *
 * - It starts the main activity. Android 10+ blocks background activity starts,
 *   and a desktop companion has no business stealing focus at boot. Instead it
 *   restores the state that matters: the control handlers and the creature, so
 *   an agent can drive the app without the user having opened it.
 * - It ignores the broadcast when the setting is off. A receiver that started
 *   regardless would make the toggle a lie in the other direction.
 */
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED &&
            intent.action != Intent.ACTION_MY_PACKAGE_REPLACED
        ) {
            return
        }

        Log.i(TAG, "boot broadcast received: ${intent.action}")
        // goAsync() returns null when the platform is not driving this receiver,
        // and the NPE that followed was in a worker thread where nothing could
        // see it. Do the work inline in that case, which also makes the receiver
        // safe to call directly from a test.
        val pending = goAsync()
        fun work() {
            try {
                val app = context.applicationContext as? ScreenBuddyApp ?: return
                if (!app.settingsRepository.autoStartEnabledBlocking()) {
                    Log.i(TAG, "auto start is off; not restoring")
                    return
                }
                // Force these: a boot broadcast can arrive while the process is
                // cold, and an unregistered handler would make every control
                // command look accepted while doing nothing.
                app.controlCommands
                val engine = ControlBus.creatures
                if (engine.count() == 0) {
                    engine.add(DEFAULT_CREATURE, DEFAULT_X, DEFAULT_Y)
                }
                Log.i(TAG, "restored: control handlers and ${engine.count()} creature(s)")
            } catch (e: Exception) {
                Log.e(TAG, "could not restore after boot", e)
            } finally {
                // finish() belongs to the thread that did the work.
                pending?.finish()
            }
        }
        if (pending != null) {
            // A receiver has about ten seconds; do not block its thread.
            Thread(::work).start()
        } else {
            work()
        }
    }

    companion object {
        private const val TAG = "ScreenBuddyBoot"
        private const val DEFAULT_CREATURE = "companion-bird-01"
        private const val DEFAULT_X = 120f
        private const val DEFAULT_Y = 260f
    }
}