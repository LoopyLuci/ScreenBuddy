package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.ScreenBuddyApp
import com.screenbuddy.android.data.agent.AgentProfile
import com.screenbuddy.android.data.agent.AgentProfileStore
import com.screenbuddy.android.data.agent.MovementStyle
import com.screenbuddy.android.data.agent.Persona

/**
 * Editor for saved agents.
 *
 * Two screens: a list with the active agent marked, and an editor for one profile.
 * The active agent's row is obvious at a glance, because which agent answers is
 * the single most important thing on this page.
 *
 * Every field here reaches a request. Nothing is saved without being applied:
 * [applyDraft] writes the profile and reloads the runtime in one step, since a
 * save that does not take effect until next launch is the defect this whole
 * feature exists to avoid.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentsScreen(store: AgentProfileStore = rememberAgentStore()) {
    var profiles by remember { mutableStateOf(store.summaries()) }
    var editing by remember { mutableStateOf<AgentProfile?>(null) }
    var confirmDelete by remember { mutableStateOf<AgentProfile?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        profiles = store.summaries()
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("Agents") }) }
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
        ) {
            if (notice != null) {
                Text(
                    notice!!,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)
                )
            }
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                item {
                    Text(
                        "The active agent's persona, model and tool budget apply to every reply.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(8.dp))
                }
                items(profiles, key = { it.id }) { summary ->
                    AgentRow(
                        name = summary.name,
                        persona = summary.persona,
                        creature = summary.creature,
                        active = summary.active,
                        builtin = summary.builtin,
                        onEdit = { editing = store.get(summary.id) },
                        onActivate = {
                            store.setActive(summary.id)
                            refresh()
                            notice = "${summary.name} is now answering"
                        },
                        onDuplicate = {
                            store.duplicate(summary.id)
                            refresh()
                        },
                        onDelete = { confirmDelete = store.get(summary.id) }
                    )
                }
                item {
                    Spacer(Modifier.height(8.dp))
                    Button(
                        onClick = { editing = AgentProfile(id = "", name = "") },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("New agent")
                    }
                }
            }
        }
    }

    editing?.let { draft ->
        AgentEditorDialog(
            initial = draft,
            onDismiss = { editing = null },
            onApply = { profile, activate ->
                val saved = store.save(profile)
                if (activate) store.setActive(saved.id)
                editing = null
                refresh()
                notice = if (activate) "${saved.name} saved and active" else "${saved.name} saved"
            }
        )
    }

    confirmDelete?.let { target ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text("Delete ${target.name}?") },
            text = { Text("This cannot be undone. Built-in agents cannot be deleted.") },
            confirmButton = {
                TextButton(onClick = {
                    val removed = store.delete(target.id)
                    confirmDelete = null
                    refresh()
                    notice = if (removed) "${target.name} deleted" else
                        "${target.name} is built in and was kept"
                }) { Text("Delete") }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = null }) { Text("Cancel") }
            }
        )
    }
}

@Composable
private fun AgentRow(
    name: String,
    persona: String,
    creature: String,
    active: Boolean,
    builtin: Boolean,
    onEdit: () -> Unit,
    onActivate: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit
) {
    Card(
        colors = CardDefaults.cardColors(
            containerColor = if (active) MaterialTheme.colorScheme.primaryContainer
            else MaterialTheme.colorScheme.surfaceVariant
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = if (active) FontWeight.Bold else FontWeight.Normal
                    )
                    Text(
                        buildString {
                            append(persona)
                            if (creature.isNotBlank()) append(" · ").append(creature)
                            if (builtin) append(" · built-in")
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                if (active) {
                    AssistChip(
                        onClick = onActivate,
                        label = { Text("Active") }
                    )
                }
            }
            Row(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                modifier = Modifier.padding(top = 4.dp)
            ) {
                TextButton(onClick = onEdit) { Text("Edit") }
                if (!active) {
                    TextButton(onClick = onActivate) { Text("Use this") }
                }
                TextButton(onClick = onDuplicate) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = null)
                    Spacer(Modifier.width(4.dp))
                    Text("Duplicate")
                }
                if (!builtin) {
                    TextButton(onClick = onDelete) {
                        Icon(Icons.Filled.Delete, contentDescription = null)
                        Spacer(Modifier.width(4.dp))
                        Text("Delete")
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AgentEditorDialog(
    initial: AgentProfile,
    onDismiss: () -> Unit,
    onApply: (AgentProfile, Boolean) -> Unit
) {
    var name by remember { mutableStateOf(initial.name) }
    var model by remember { mutableStateOf(initial.model) }
    var creature by remember { mutableStateOf(initial.creature) }
    var systemPrompt by remember { mutableStateOf(initial.systemPrompt) }
    var persona by remember { mutableStateOf(initial.persona) }
    var movement by remember { mutableStateOf(initial.movement) }
    var toolsEnabled by remember { mutableStateOf(initial.toolsEnabled) }
    var temperature by remember { mutableStateOf(initial.temperature) }
    var iterations by remember { mutableStateOf(initial.maxIterations.toFloat()) }
    var applyNow by remember { mutableStateOf(!initial.builtin) }
    var showError by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (initial.id.isBlank()) "New agent" else "Edit ${initial.name}") },
        text = {
            LazyColumn(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                item {
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        label = { Text("Name") },
                        singleLine = true,
                        isError = showError && name.isBlank(),
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                item {
                    OutlinedTextField(
                        value = model,
                        onValueChange = { model = it },
                        label = { Text("Model (blank = automatic)") },
                        singleLine = true,
                        supportingText = { Text("Pin a model to use it every time.") },
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                item {
                    OutlinedTextField(
                        value = creature,
                        onValueChange = { creature = it },
                        label = { Text("Creature") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                item {
                    Text("Persona", style = MaterialTheme.typography.labelLarge)
                    // Chips wrap rather than scroll: six options fit two rows on a
                    // phone, and a hidden option reads as a missing feature.
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Persona.entries.chunked(2).forEach { pair ->
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                pair.forEach { option ->
                                    FilterChip(
                                        selected = persona == option,
                                        onClick = { persona = option },
                                        label = { Text(option.label) }
                                    )
                                }
                            }
                        }
                    }
                    Text(
                        persona.guidance,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                item {
                    Text("Movement", style = MaterialTheme.typography.labelLarge)
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        MovementStyle.entries.chunked(2).forEach { pair ->
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                pair.forEach { option ->
                                    FilterChip(
                                        selected = movement == option,
                                        onClick = { movement = option },
                                        label = { Text(option.label) }
                                    )
                                }
                            }
                        }
                    }
                }
                item {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            "Allow tools",
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f)
                        )
                        Switch(checked = toolsEnabled, onCheckedChange = { toolsEnabled = it })
                    }
                }
                item {
                    Text(
                        "Tool budget: ${iterations.toInt()}",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Slider(
                        value = iterations,
                        onValueChange = { iterations = it },
                        valueRange = 1f..25f,
                        steps = 23,
                        enabled = toolsEnabled
                    )
                }
                item {
                    Text(
                        "Temperature: ${"%.1f".format(temperature)}",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Slider(
                        value = temperature,
                        onValueChange = { temperature = it },
                        valueRange = 0f..2f
                    )
                    Text(
                        "0.7 means \"use the app's own setting\".",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                item {
                    OutlinedTextField(
                        value = systemPrompt,
                        onValueChange = { systemPrompt = it },
                        label = { Text("Extra instructions") },
                        supportingText = { Text("Added after the persona.") },
                        minLines = 2,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                item {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            "Make this the active agent",
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f)
                        )
                        Switch(checked = applyNow, onCheckedChange = { applyNow = it })
                    }
                }
            }
        },
        confirmButton = {
            Button(onClick = {
                if (name.isBlank()) {
                    // Show the error on the field rather than silently ignoring
                    // an empty name.
                    showError = true
                    return@Button
                }
                onApply(
                    initial.copy(
                        name = name,
                        model = model,
                        creature = creature,
                        systemPrompt = systemPrompt,
                        persona = persona,
                        movement = movement,
                        toolsEnabled = toolsEnabled,
                        maxIterations = iterations.toInt(),
                        temperature = temperature
                    ),
                    applyNow
                )
            }) { Text("Save") }
        },
        dismissButton = {
            OutlinedButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

@Composable
private fun rememberAgentStore(): AgentProfileStore {
    val context = LocalContext.current
    val app = context.applicationContext as ScreenBuddyApp
    return remember(app) { app.agentProfiles }
}