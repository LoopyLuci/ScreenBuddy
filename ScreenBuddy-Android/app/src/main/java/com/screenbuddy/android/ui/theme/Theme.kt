package com.screenbuddy.android.ui.theme

import android.app.Activity
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val LightColorScheme = lightColorScheme(
    primary = ScreenBuddyPrimary,
    onPrimary = androidx.compose.ui.graphics.Color.White,
    primaryContainer = ScreenBuddyPrimary.copy(alpha = 0.1f),
    onPrimaryContainer = ScreenBuddyPrimary,
    secondary = ScreenBuddySecondary,
    onSecondary = androidx.compose.ui.graphics.Color.White,
    tertiary = ScreenBuddyTertiary,
    onTertiary = androidx.compose.ui.graphics.Color.White,
    background = ScreenBuddyBackground,
    onBackground = ScreenBuddyTextPrimary,
    surface = ScreenBuddySurface,
    onSurface = ScreenBuddyTextPrimary,
    surfaceVariant = ScreenBuddySurfaceVariant,
    onSurfaceVariant = ScreenBuddyTextSecondary,
    outline = ScreenBuddyOutline,
    error = ScreenBuddyError,
    onError = androidx.compose.ui.graphics.Color.White
)

private val DarkColorScheme = darkColorScheme(
    primary = ScreenBuddyPrimary,
    onPrimary = androidx.compose.ui.graphics.Color.White,
    primaryContainer = ScreenBuddyPrimary.copy(alpha = 0.2f),
    onPrimaryContainer = ScreenBuddyPrimary,
    secondary = ScreenBuddySecondary,
    onSecondary = androidx.compose.ui.graphics.Color.White,
    tertiary = ScreenBuddyTertiary,
    onTertiary = androidx.compose.ui.graphics.Color.White,
    background = androidx.compose.ui.graphics.Color(0xFF121212),
    onBackground = androidx.compose.ui.graphics.Color.White,
    surface = androidx.compose.ui.graphics.Color(0xFF1E1E1E),
    onSurface = androidx.compose.ui.graphics.Color.White,
    surfaceVariant = androidx.compose.ui.graphics.Color(0xFF2D2D2D),
    onSurfaceVariant = androidx.compose.ui.graphics.Color(0xFFB0B0B0),
    outline = androidx.compose.ui.graphics.Color(0xFF444444),
    error = ScreenBuddyError,
    onError = androidx.compose.ui.graphics.Color.White
)

@Composable
fun ScreenBuddyTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme
    
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = colorScheme.surface.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !darkTheme
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
