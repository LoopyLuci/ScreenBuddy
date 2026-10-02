package com.screenbuddy.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.screenbuddy.android.ui.screens.MainScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Read once from the manifest-backed BuildConfig so the About screen and
        // the shipped artifact cannot drift apart.
        val versionName = BuildConfig.VERSION_NAME
        setContent {
            MainScreen(versionName = versionName)
        }
    }
}