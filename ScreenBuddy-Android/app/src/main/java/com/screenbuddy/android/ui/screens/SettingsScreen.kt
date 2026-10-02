package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen() {
    LazyColumn(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        item {
            Text("Settings", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(bottom = 16.dp))
        }

        // General Settings
        item {
            SettingsSection(title = "General") {
                SettingsToggle(title = "Dark Mode", description = "Use dark theme", icon = Icons.Filled.DarkMode, initialState = false) { }
                SettingsToggle(title = "Notifications", description = "Show notifications", icon = Icons.Filled.Notifications, initialState = true) { }
                SettingsToggle(title = "Sound Effects", description = "Play sounds", icon = Icons.Filled.VolumeUp, initialState = true) { }
                SettingsToggle(title = "Auto-start", description = "Launch on boot", icon = Icons.Filled.Power, initialState = false) { }
            }
        }

        // AI Settings
        item {
            SettingsSection(title = "AI & Models") {
                SettingsItem(title = "Default Model", description = "Select default AI model", icon = Icons.Filled.Psychology) { }
                SettingsItem(title = "Response Length", description = "Control response verbosity", icon = Icons.Filled.ShortText) { }
                SettingsItem(title = "Temperature", description = "AI creativity level", icon = Icons.Filled.Thermostat) { }
                SettingsToggle(title = "Stream Responses", description = "Show responses as they generate", icon = Icons.Filled.Stream, initialState = true) { }
            }
        }

        // Creature Settings
        item {
            SettingsSection(title = "Creatures") {
                SettingsToggle(title = "Show on Desktop", description = "Display creatures on desktop", icon = Icons.Filled.Pets, initialState = true) { }
                SettingsToggle(title = "Animations", description = "Enable creature animations", icon = Icons.Filled.Animation, initialState = true) { }
                SettingsToggle(title = "Sounds", description = "Creature sound effects", icon = Icons.Filled.MusicNote, initialState = true) { }
                SettingsItem(title = "Animation Speed", description = "Adjust animation speed", icon = Icons.Filled.Speed) { }
            }
        }

        // Audio Settings
        item {
            SettingsSection(title = "Audio") {
                SettingsSlider(title = "Master Volume", value = 0.7f) { }
                SettingsSlider(title = "Effects Volume", value = 0.8f) { }
                SettingsSlider(title = "TTS Volume", value = 0.8f) { }
                SettingsToggle(title = "Text-to-Speech", description = "Speak AI responses", icon = Icons.Filled.RecordVoiceOver, initialState = false) { }
            }
        }

        // About
        item {
            SettingsSection(title = "About") {
                SettingsItem(title = "Version", description = "1.0.0", icon = Icons.Filled.Info) { }
                SettingsItem(title = "Open Source", description = "View source code", icon = Icons.Filled.Code) { }
            }
        }
    }
}

@Composable
fun SettingsSection(title: String, content: @Composable () -> Unit) {
    Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold, modifier = Modifier.padding(vertical = 8.dp))
    Card(shape = RoundedCornerShape(12.dp)) {
        Column(modifier = Modifier.fillMaxWidth()) {
            content()
        }
    }
    Spacer(modifier = Modifier.height(16.dp))
}

@Composable
fun SettingsItem(title: String, description: String, icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    Row(modifier = Modifier.fillMaxWidth().clickable { onClick() }.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(Icons.Filled.ChevronRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun SettingsToggle(title: String, description: String, icon: androidx.compose.ui.graphics.vector.ImageVector, initialState: Boolean, onToggle: (Boolean) -> Unit) {
    var checked by remember { mutableStateOf(initialState) }
    Row(modifier = Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = { checked = it; onToggle(it) })
    }
}

@Composable
fun SettingsSlider(title: String, value: Float, onValueChange: (Float) -> Unit) {
    var sliderValue by remember { mutableStateOf(value) }
    Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
        Text(title, style = MaterialTheme.typography.bodyLarge)
        Slider(value = sliderValue, onValueChange = { sliderValue = it; onValueChange(it) }, valueRange = 0f..1f)
    }
}
