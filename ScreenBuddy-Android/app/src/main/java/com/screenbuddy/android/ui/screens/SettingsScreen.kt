package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.ui.draw.alpha
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.ScreenBuddyApp
import com.screenbuddy.android.viewmodel.SettingsUiState
import com.screenbuddy.android.viewmodel.SettingsViewModel

@Composable
private fun rememberSettingsViewModel(versionName: String): SettingsViewModel {
    val context = LocalContext.current
    val app = context.applicationContext as ScreenBuddyApp
    return remember(versionName) {
        SettingsViewModel(
            settingsRepository = app.settingsRepository,
            apiKeyDao = app.database.apiKeyDao(),
            modelDao = app.database.modelDao(),
            versionName = versionName
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(versionName: String) {
    val viewModel = rememberSettingsViewModel(versionName)
    val uiState by viewModel.uiState.collectAsState()

    Scaffold(topBar = { TopAppBar(title = { Text("Settings") }) }) { padding ->
        SettingsContent(uiState, Modifier.padding(padding), viewModel)
    }
}

@Composable
private fun SettingsContent(
    uiState: SettingsUiState,
    modifier: Modifier = Modifier,
    viewModel: SettingsViewModel
) {
    LazyColumn(
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp)
    ) {
        item {
            SettingsSection("General") {
                SettingsToggle(
                    "Dark mode",
                    "Use the dark theme",
                    Icons.Filled.DarkMode,
                    uiState.darkMode,
                    viewModel::setDarkMode
                )
                SettingsToggle(
                    "Notifications",
                    "Show notifications",
                    Icons.Filled.Notifications,
                    uiState.notificationsEnabled,
                    viewModel::setNotifications
                )
                SettingsToggle(
                    "Sound effects",
                    "Play UI sounds",
                    Icons.Filled.VolumeUp,
                    uiState.soundEffectsEnabled,
                    viewModel::setSoundEffects
                )
                SettingsToggle(
                    "Auto-start",
                    "Launch ScreenBuddy on boot",
                    Icons.Filled.Power,
                    uiState.autoStart,
                    viewModel::setAutoStart
                )
            }
        }

        item {
            SettingsSection("AI & Models") {
                if (uiState.enabledModels.isEmpty()) {
                    Text(
                        "No models enabled. Turn some on in the Providers tab.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp)
                    )
                } else {
                    var pickerOpen by remember { mutableStateOf(false) }
                    SettingsItem(
                        title = "Default model",
                        description = uiState.selectedModel?.displayName ?: "None selected",
                        icon = Icons.Filled.Psychology,
                        onClick = { pickerOpen = true }
                    )
                    if (pickerOpen) {
                        ModelPickerDialog(
                            models = uiState.enabledModels,
                            selectedId = uiState.selectedModel?.id,
                            onSelect = {
                                viewModel.setSelectedModel(it)
                                pickerOpen = false
                            },
                            onDismiss = { pickerOpen = false }
                        )
                    }
                }

                SettingsSlider(
                    "Temperature",
                    uiState.temperature,
                    0f..2f,
                    viewModel::setTemperature
                )
                SettingsSlider(
                    "Response length",
                    uiState.responseLength.toFloat(),
                    64f..4096f,
                    { value -> viewModel.setResponseLength(value.toInt()) }
                )
                SettingsToggle(
                    "Stream responses",
                    "Show responses as they generate",
                    Icons.Filled.Stream,
                    uiState.streamResponses,
                    viewModel::setStreamResponses
                )
                OllamaUrlRow(uiState.ollamaBaseUrl, viewModel::setOllamaBaseUrl)
            }
        }

        item {
            SettingsSection("Creatures") {
                SettingsToggle(
                    "Show creatures",
                    "Display your companions",
                    Icons.Filled.Pets,
                    uiState.showCreatures,
                    viewModel::setShowCreatures
                )
                SettingsToggle(
                    "Animations",
                    "Animate your companions",
                    Icons.Filled.Animation,
                    uiState.animationsEnabled,
                    viewModel::setAnimations
                )
                SettingsToggle(
                    "Creature sounds",
                    "Play creature sound effects",
                    Icons.Filled.MusicNote,
                    uiState.creatureSoundsEnabled,
                    viewModel::setCreatureSounds
                )
                // animation_speed: the Compose UI has no animation system, so
                // there is nothing to scale. Left visible and labelled.
                SettingsSlider(
                    "Animation speed",
                    uiState.animationSpeed,
                    0.25f..3f,
                    viewModel::setAnimationSpeed
                )
            }
        }

        item {
            SettingsSection("Audio") {
                SettingsSlider("Master volume", uiState.masterVolume, 0f..1f, viewModel::setMasterVolume)
                SettingsSlider(
                    "Effects volume",
                    uiState.effectsVolume,
                    0f..1f,
                    viewModel::setEffectsVolume
                )
                SettingsSlider("TTS volume", uiState.ttsVolume, 0f..1f, viewModel::setTtsVolume)
                SettingsToggle(
                    "Text-to-speech",
                    "Speak AI responses aloud",
                    Icons.Filled.RecordVoiceOver,
                    uiState.ttsEnabled,
                    viewModel::setTtsEnabled
                )
            }
        }

        item {
            SettingsSection("Credentials") {
                SettingsItem(
                    title = "Clear all API keys",
                    description = "${uiState.providersWithKeys.size} provider(s) configured",
                    icon = Icons.Filled.KeyOff,
                    onClick = { viewModel.clearAllApiKeys() }
                )
            }
        }

        item {
            SettingsSection("About") {
                SettingsItem(
                    title = "Version",
                    description = uiState.versionName,
                    icon = Icons.Filled.Info,
                    onClick = {}
                )
                SettingsItem(
                    title = "Source code",
                    description = "github.com/LoopyLuci/ScreenBuddy",
                    icon = Icons.Filled.Code,
                    onClick = {}
                )
            }
        }
    }
}

@Composable
private fun OllamaUrlRow(current: String, onSave: (String) -> Unit) {
    var editing by remember { mutableStateOf(false) }
    var entry by remember(current) { mutableStateOf(current) }

    SettingsItem(
        title = "Ollama endpoint",
        description = current,
        icon = Icons.Filled.Dns,
        onClick = { editing = true }
    )

    if (editing) {
        AlertDialog(
            onDismissRequest = { editing = false },
            title = { Text("Ollama endpoint") },
            text = {
                Column {
                    Text(
                        "10.0.2.2 is the host machine as seen from the Android emulator.",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = entry,
                        onValueChange = { entry = it },
                        singleLine = true,
                        placeholder = { Text("http://10.0.2.2:11434") }
                    )
                }
            },
            confirmButton = {
                TextButton(
                    enabled = entry.isNotBlank(),
                    onClick = {
                        onSave(entry)
                        editing = false
                    }
                ) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { editing = false }) { Text("Cancel") }
            }
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModelPickerDialog(
    models: List<com.screenbuddy.android.data.model.AiModel>,
    selectedId: String?,
    onSelect: (String) -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Choose a model") },
        text = {
            LazyColumn {
                items(models, key = { it.id }) { model ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(model.id) }
                            .padding(vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(model.displayName, style = MaterialTheme.typography.bodyLarge)
                            Text(
                                model.providerId,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        if (model.id == selectedId) {
                            Icon(Icons.Filled.Check, contentDescription = "Selected")
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } }
    )
}

@Composable
fun SettingsSection(title: String, content: @Composable () -> Unit) {
    Text(
        title,
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.Bold,
        modifier = Modifier.padding(vertical = 8.dp)
    )
    Card(shape = RoundedCornerShape(12.dp)) {
        Column(Modifier.fillMaxWidth()) { content() }
    }
    Spacer(Modifier.height(16.dp))
}

@Composable
fun SettingsItem(
    title: String,
    description: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() }
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Icon(
            Icons.Filled.ChevronRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

/**
 * Toggle bound to persisted state.
 *
 * The displayed value comes from [state] rather than local `remember`, so the
 * switch reflects what was actually written and survives recomposition.
 */
@Composable
fun SettingsToggle(
    title: String,
    description: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    state: Boolean,
    onToggle: (Boolean) -> Unit,
    /**
     * Why this control does nothing yet, or null when it works.
     *
     * Nine switches on this screen stored a value nothing read. A control that
     * looks live and silently does nothing is the worst outcome, so they are
     * disabled and say so. See tools/android_surface_audit.py, which is what
     * found them.
     */
    unavailableReason: String? = null
) {
    val enabled = unavailableReason == null
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp)
            .then(
                if (enabled) Modifier
                else Modifier.alpha(0.5f)
            ),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = if (enabled) MaterialTheme.colorScheme.primary
            else MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(
                // When unavailable the reason replaces the description, so the
                // user is never left guessing.
                unavailableReason ?: description,
                style = MaterialTheme.typography.bodySmall,
                color = if (enabled) MaterialTheme.colorScheme.onSurfaceVariant
                else MaterialTheme.colorScheme.error
            )
        }
        Switch(
            checked = state,
            onCheckedChange = onToggle,
            enabled = enabled
        )
    }
}

@Composable
fun SettingsSlider(
    title: String,
    value: Float,
    valueRange: ClosedFloatingPointRange<Float>,
    onValueChange: (Float) -> Unit,
    /** Why this control does nothing yet, or null when it works. */
    unavailableReason: String? = null
) {
    val enabled = unavailableReason == null
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp)
            .then(if (enabled) Modifier else Modifier.alpha(0.5f))
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            Text(
                formatSliderValue(value),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        if (unavailableReason != null) {
            Text(
                unavailableReason,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error
            )
        }
        Slider(
            value = value.coerceIn(valueRange),
            onValueChange = onValueChange,
            valueRange = valueRange,
            enabled = enabled
        )
    }
}

/** Render a slider value with sensible precision per magnitude. */
private fun formatSliderValue(value: Float): String =
    if (value >= 10f) value.toInt().toString() else String.format("%.2f", value)