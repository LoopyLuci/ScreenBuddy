package com.screenbuddy.android.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat

/**
 * Asks for notification permission when the user switches notifications on.
 *
 * POST_NOTIFICATIONS became a runtime permission in Android 13. Without this the
 * notifier caught the SecurityException and did nothing, so the setting looked
 * enabled and no notification ever appeared - the same defect shape as every
 * other control this project has had to fix.
 *
 * The request is made when it is relevant: when the user turns the setting on,
 * not on first launch. Asking at launch, before the user has any idea what
 * ScreenBuddy is, is declined far more often.
 *
 * Deliberately does nothing below Android 13, where the permission does not
 * exist, and nothing if it has already been granted.
 */
@Composable
fun RequestNotificationPermissionIfNeeded(
    /** The user's current choice, so the request follows the setting. */
    enabled: Boolean,
    /** Called when the request finishes, granted or not. */
    onResult: (Boolean) -> Unit = {}
) {
    val context = LocalContext.current

    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return

    val alreadyGranted = ContextCompat.checkSelfPermission(
        context, Manifest.permission.POST_NOTIFICATIONS
    ) == PackageManager.PERMISSION_GRANTED

    // Guards against asking twice: the launcher fires on every recomposition
    // while enabled stays true if the answer is not remembered.
    var askedFor by remember { mutableStateOf(false) }

    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { granted ->
        askedFor = true
        onResult(granted)
    }

    LaunchedEffect(enabled, alreadyGranted) {
        if (enabled && !alreadyGranted && !askedFor) {
            launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}