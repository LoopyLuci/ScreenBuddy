package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.viewmodel.ProvidersUiState
import com.screenbuddy.android.viewmodel.ProvidersViewModel

@Composable
private fun rememberProvidersViewModel(): ProvidersViewModel {
    val context = androidx.compose.ui.platform.LocalContext.current
    val app = context.applicationContext as com.screenbuddy.android.ScreenBuddyApp
    return remember {
        ProvidersViewModel(
            apiKeyDao = app.database.apiKeyDao(),
            modelDao = app.database.modelDao()
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProvidersScreen(viewModel: ProvidersViewModel = rememberProvidersViewModel()) {
    val uiState by viewModel.uiState.collectAsState()

    Scaffold(
        topBar = { TopAppBar(title = { Text("Providers") }) }
    ) { padding ->
        ProvidersContent(
            uiState = uiState,
            modifier = Modifier.padding(padding),
            onToggleModel = viewModel::toggleModel,
            onSetApiKey = viewModel::setApiKey,
            onRemoveApiKey = viewModel::removeApiKey,
            onEnableAll = viewModel::enableAllModels,
            onDisableAll = viewModel::disableAllModels,
            onErrorShown = viewModel::clearError
        )
    }
}

@Composable
internal fun ProvidersContent(
    uiState: ProvidersUiState,
    modifier: Modifier = Modifier,
    onToggleModel: (String, Boolean) -> Unit,
    onSetApiKey: (String, String) -> Unit,
    onRemoveApiKey: (String) -> Unit,
    onEnableAll: (String) -> Unit,
    onDisableAll: (String) -> Unit,
    onErrorShown: () -> Unit
) {
    Column(modifier = modifier.fillMaxSize()) {
        if (uiState.error != null) {
            ErrorBanner(uiState.error, onDismiss = onErrorShown)
        }

        // An early `return@Column` here swapped the whole subtree out from under
        // the composer as isLoading flipped true->false, which corrupts the slot
        // table (ArrayIndexOutOfBoundsException in SlotTableKt.key). Branching on
        // isLoading instead keeps the group structure stable across recompositions.
        if (uiState.isLoading) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
        } else {
        LazyColumn(contentPadding = PaddingValues(16.dp)) {
            item {
                Text(
                    "${uiState.providers.size} providers available",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(8.dp))
            }
            items(uiState.providers, key = { it.id }) { provider ->
                ProviderCard(
                    provider = provider,
                    hasKey = provider.id in uiState.providersWithKeys,
                    endpoint = uiState.endpoints[provider.id],
                    onToggleModel = onToggleModel,
                    onSetApiKey = { key -> onSetApiKey(provider.id, key) },
                    onRemoveApiKey = { onRemoveApiKey(provider.id) },
                    onEnableAll = { onEnableAll(provider.id) },
                    onDisableAll = { onDisableAll(provider.id) }
                )
                Spacer(Modifier.height(12.dp))
            }
        }
        }
    }
}

@Composable
private fun ErrorBanner(message: String, onDismiss: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth().padding(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                Icons.Filled.Error,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onErrorContainer
            )
            Spacer(Modifier.width(8.dp))
            Text(
                message,
                modifier = Modifier.weight(1f),
                color = MaterialTheme.colorScheme.onErrorContainer
            )
            IconButton(onClick = onDismiss) {
                Icon(Icons.Filled.Close, contentDescription = "Dismiss")
            }
        }
    }
}

@Composable
private fun ProviderCard(
    provider: Provider,
    hasKey: Boolean,
    endpoint: String?,
    onToggleModel: (String, Boolean) -> Unit,
    onSetApiKey: (String) -> Unit,
    onRemoveApiKey: () -> Unit,
    onEnableAll: () -> Unit,
    onDisableAll: () -> Unit
) {
    var expanded by remember { mutableStateOf(false) }

    Card(shape = MaterialTheme.shapes.medium) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(provider.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                    Text(
                        provider.description,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                if (hasKey) {
                    AssistChip(
                        onClick = {},
                        label = { Text("Key set") },
                        leadingIcon = { Icon(Icons.Filled.Check, null, Modifier.size(16.dp)) }
                    )
                    Spacer(Modifier.width(4.dp))
                }
                IconButton(onClick = { expanded = !expanded }) {
                    Icon(
                        if (expanded) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                        contentDescription = if (expanded) "Collapse" else "Expand"
                    )
                }
            }

            Text(
                "${provider.models.count { it.isEnabled }}/${provider.models.size} models enabled",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp)
            )

            if (expanded) {
                Spacer(Modifier.height(12.dp))
                if (!endpoint.isNullOrBlank()) {
                    Text(
                        "Endpoint: $endpoint",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(4.dp))
                }

                Row {
                    TextButton(onClick = onEnableAll) { Text("Enable all") }
                    TextButton(onClick = onDisableAll) { Text("Disable all") }
                }

                if (provider.requiresApiKey()) {
                    Spacer(Modifier.height(8.dp))
                    ApiKeyRow(
                        hasKey = hasKey,
                        apiKeyName = provider.apiKeyName,
                        onSetApiKey = onSetApiKey,
                        onRemoveApiKey = onRemoveApiKey
                    )
                }

                Divider(Modifier.padding(vertical = 8.dp))

                provider.models.forEach { model ->
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(model.displayName, style = MaterialTheme.typography.bodyMedium)
                            Text(
                                model.description,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                        if (model.isFree) {
                            Icon(
                                Icons.Filled.Favorite,
                                contentDescription = "Free tier",
                                tint = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.size(14.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                        }
                        Switch(
                            checked = model.isEnabled,
                            onCheckedChange = { onToggleModel(model.id, it) }
                        )
                    }
                }
            }
        }
    }
}

/** True when the provider needs a credential; mirrors AiService's routing table. */
private fun Provider.requiresApiKey(): Boolean = id !in setOf(
    "ollama", "llamacpp", "lmstudio", "llmstudio", "vllm", "koboldcpp", "textgen", "opencode-go"
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ApiKeyRow(
    hasKey: Boolean,
    apiKeyName: String,
    onSetApiKey: (String) -> Unit,
    onRemoveApiKey: () -> Unit
) {
    var showDialog by remember { mutableStateOf(false) }

    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text("API key", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Bold)
            Text(
                if (hasKey) "Configured ($apiKeyName)" else "Not set ($apiKeyName)",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        TextButton(onClick = { showDialog = true }) {
            Text(if (hasKey) "Replace" else "Add")
        }
        if (hasKey) {
            IconButton(onClick = onRemoveApiKey) {
                Icon(Icons.Filled.Delete, contentDescription = "Remove key")
            }
        }
    }

    if (showDialog) {
        var entry by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { showDialog = false },
            title = { Text("Set API key") },
            text = {
                Column {
                    Text(
                        apiKeyName,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = entry,
                        onValueChange = { entry = it },
                        label = { Text("Key") },
                        singleLine = true
                    )
                }
            },
            confirmButton = {
                TextButton(
                    enabled = entry.isNotBlank(),
                    onClick = {
                        onSetApiKey(entry)
                        showDialog = false
                    }
                ) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { showDialog = false }) { Text("Cancel") }
            }
        )
    }
}