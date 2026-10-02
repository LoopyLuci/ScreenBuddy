package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import com.screenbuddy.android.ui.theme.ScreenBuddyTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.screenbuddy.android.ScreenBuddyApp

sealed class Screen(val route: String, val title: String, val icon: ImageVector) {
    object Chat : Screen("chat", "Chat", Icons.Filled.Chat)
    object Providers : Screen("providers", "Providers", Icons.Filled.Cloud)
    object Creatures : Screen("creatures", "Creatures", Icons.Filled.Pets)
    object Settings : Screen("settings", "Settings", Icons.Filled.Settings)
}

/**
 * Root navigation.
 *
 * Shared state that both Chat and Settings need (enabled models, the selected
 * model, TTS preference) is hoisted here and collected once, so the two screens
 * cannot disagree about which model is active.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen(versionName: String) {
    val navController = rememberNavController()
    val items = listOf(Screen.Chat, Screen.Providers, Screen.Creatures, Screen.Settings)

    val context = LocalContext.current
    val app = remember { context.applicationContext as ScreenBuddyApp }
    val settings by app.settingsRepository.settings.collectAsState(initial = null)
    val darkMode = settings?.darkMode ?: false

    // Model enablement lives in Room, not in settings, so read it from the same
    // source the Providers tab writes to.
    val enabledModels by produceState(
        initialValue = emptyList<com.screenbuddy.android.data.model.AiModel>()
    ) {
        val repo = com.screenbuddy.android.data.repository.ProviderRepository(
            app.database.apiKeyDao(),
            app.database.modelDao()
        )
        repo.seedIfEmpty()
        repo.providers.collect { providers ->
            value = providers.flatMap { p ->
                p.models.filter { it.isEnabled }.map { it.copy(providerId = p.id) }
            }
        }
    }

    var selectedModelId by rememberSaveable { mutableStateOf(settings?.selectedModelId ?: "") }
    val ttsEnabled = settings?.ttsEnabled ?: false

    ScreenBuddyTheme(darkTheme = darkMode) {
        Scaffold(
            bottomBar = {
                NavigationBar {
                    val navBackStackEntry by navController.currentBackStackEntryAsState()
                    val currentDestination = navBackStackEntry?.destination
                    items.forEach { screen ->
                        NavigationBarItem(
                            icon = { Icon(screen.icon, contentDescription = screen.title) },
                            label = { Text(screen.title) },
                            selected = currentDestination?.hierarchy?.any { it.route == screen.route } == true,
                            onClick = {
                                navController.navigate(screen.route) {
                                    popUpTo(navController.graph.findStartDestination().id) {
                                        saveState = true
                                    }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            }
                        )
                    }
                }
            }
        ) { innerPadding ->
            NavHost(
                navController = navController,
                startDestination = Screen.Chat.route,
                modifier = Modifier.padding(innerPadding)
            ) {
                composable(Screen.Chat.route) {
                    ChatScreen(
                        enabledModels = enabledModels,
                        selectedModelId = selectedModelId,
                        onModelSelected = { selectedModelId = it },
                        ttsEnabled = ttsEnabled
                    )
                }
                composable(Screen.Providers.route) { ProvidersScreen() }
                composable(Screen.Creatures.route) { CreaturesScreen() }
                composable(Screen.Settings.route) { SettingsScreen(versionName) }
            }
        }
    }
}