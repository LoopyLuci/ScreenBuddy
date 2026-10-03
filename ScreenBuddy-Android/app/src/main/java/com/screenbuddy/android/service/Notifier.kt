package com.screenbuddy.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import com.screenbuddy.android.R

/**
 * Notifications for a background reply or a finished task.
 *
 * The `notifications` setting existed and the channel was never created, so the
 * toggle controlled nothing. Creating the channel is what makes a notification
 * possible at all on Android 8 and above.
 */
class Notifier(private val context: Context) {

    /**
     * Create the channel if needed. Safe to call repeatedly.
     *
     * Channels are user-owned once created: changing the importance afterwards
     * has no effect, which is why the channel is only created once and the
     * setting gates posting rather than configuration.
     */
    fun ensureChannel(): Boolean = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = context.getSystemService(NotificationManager::class.java)
            if (manager.getNotificationChannel(CHANNEL_ID) == null) {
                manager.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_ID,
                        "Replies and tasks",
                        // Default, not high: a companion that interrupts is worse
                        // than one that is merely silent.
                        NotificationManager.IMPORTANCE_DEFAULT
                    ).apply {
                        description = "Tells you when ScreenBuddy has something to say."
                        setShowBadge(true)
                    }
                )
            }
        }
        true
    } catch (e: Exception) {
        // A missing channel must not take the app down; notifications are a
        // courtesy, not a requirement.
        Log.w(TAG, "could not create the notification channel: ${e.message}")
        false
    }

    /**
     * Post a notification, if the setting allows it.
     *
     * Returns false when it did not post, and why: no permission, notifications
     * switched off, or the platform refused. Callers surface that rather than
     * assuming delivery.
     */
    fun notify(title: String, body: String): Boolean {
        if (!ensureChannel()) return false
        if (!areNotificationsEnabled()) return false

        return try {
            val notification: Notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_launcher)
                .setContentTitle(title)
                .setContentText(body)
                .setStyle(NotificationCompat.BigTextStyle().bigText(body))
                .setAutoCancel(true)
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .build()
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, notification)
            true
        } catch (e: SecurityException) {
            // POST_NOTIFICATIONS is a runtime permission on Android 13+.
            Log.w(TAG, "notification permission not granted: ${e.message}")
            false
        } catch (e: Exception) {
            Log.w(TAG, "could not post a notification: ${e.message}")
            false
        }
    }

    /** Whether the user has notifications on for this app. */
    fun areNotificationsEnabled(): Boolean = try {
        NotificationManagerCompat.from(context).areNotificationsEnabled()
    } catch (e: Exception) {
        false
    }

    /**
     * Whether a notification would actually appear.
     *
     * Used by the settings screen to disable the toggle with a reason, rather
     * than letting a user enable something that cannot work.
     */
    fun blockReason(): String? = when {
        !areNotificationsEnabled() -> "Notifications are turned off for ScreenBuddy in Android settings"
        else -> null
    }

    companion object {
        const val CHANNEL_ID = "screenbuddy_replies"
        private const val NOTIFICATION_ID = 1001
        private const val TAG = "ScreenBuddyNotify"
    }
}