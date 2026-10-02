package com.screenbuddy.android.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.ApiKey
import com.screenbuddy.android.ui.theme.*
import com.screenbuddy.android.viewmodel.ProvidersViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProvidersScreen(viewModel: ProvidersViewModel = remember { ProvidersViewModel() }) {
    val uiState by viewModel.uiState.collectAsState()
    var selectedProvider by remember { mutableStateOf<Provider?>(null) }
    var showApiKeyDialog by remember { mutableStateOf(false) }
    var apiKeyInput by remember { mutableStateOf("") }

    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("AI Providers", style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(bottom = 16.dp))

        if (uiState.isLoading) {
            CircularProgressIndicator()
            return@Column
        }

        LazyColumn {
            items(uiState.providers) { provider ->
                ProviderCard(
                    provider = provider,
                    onToggleModel = { modelId, enabled -> viewModel.toggleModel(modelId, enabled) },
                    onToggleAllPaid = { providerId -> viewModel.disableAllPaidModels(providerId) },
                    onEnableAllPaid = { providerId -> viewModel.enableAllModels(providerId) },
                    onSetApiKey = { providerId, key -> viewModel.setApiKey(providerId, key) },
                    onRemoveApiKey = { providerId -> viewModel.removeApiKey(providerId) },
                    onClick = { selectedProvider = provider }
                )
                Spacer(modifier = Modifier.height(8.dp))
            }
        }
    }

    // API Key Dialog
    if (showApiKeyDialog && selectedProvider != null) {
        AlertDialog(
            onDismissRequest = { showApiKeyDialog = false },
            title = { Text("Set API Key for ${selectedProvider!!.name}") },
            text = {
                Column {
                    Text("Enter your API key:", modifier = Modifier.padding(bottom = 8.dp))
                    OutlinedTextField(
                        value = apiKeyInput,
                        onValueChange = { apiKeyInput = it },
                        label = { Text(selectedProvider!!.apiKeyName) },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.setApiKey(selectedProvider!!.id, apiKeyInput)
                    showApiKeyDialog = false
                    apiKeyInput = ""
                }) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { showApiKeyDialog = false }) { Text("Cancel") }
            }
        )
    }
}

@Composable
fun ProviderCard(
    provider: Provider,
    onToggleModel: (String, Boolean) -> Unit,
    onToggleAllPaid: (String) -> Unit,
    onEnableAllPaid: (String) -> Unit,
    onSetApiKey: (String, String) -> Unit,
    onRemoveApiKey: (String) -> Unit,
    onClick: () -> Unit
) {
    var expanded by remember { mutableStateOf(false) }
    val freeModels = provider.models.filter { it.isFree }
    val paidModels = provider.models.filter { !it.isFree }

    Card(
        modifier = Modifier.fillMaxWidth().clickable { onClick() },
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(provider.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
                    Text(provider.description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                // API Key status indicator
                if (provider.apiKey?.isSet == true) {
                    Icon(Icons.Filled.CheckCircle, contentDescription = "API Key Set", tint = ApiKeySetColor)
                } else {
                    Icon(Icons.Filled.Warning, contentDescription = "API Key Missing", tint = ApiKeyMissingColor)
                }
                IconButton(onClick = { expanded = !expanded }) {
                    Icon(if (expanded) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore, contentDescription = "Expand")
                }
            }

            // API Key buttons
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                if (provider.apiKey?.isSet == true) {
                    TextButton(onClick = { onRemoveApiKey(provider.id) }) { Text("Remove Key") }
                    TextButton(onClick = { /* Show dialog to edit */ }) { Text("Edit") }
                } else {
                    TextButton(onClick = { /* Show dialog to add */ }) { Text("Add API Key") }
                }
            }

            AnimatedVisibility(visible = expanded) {
                Column(modifier = Modifier.padding(top = 8.dp)) {
                    // Free models section
                    if (freeModels.isNotEmpty()) {
                        Text("Free Models", style = MaterialTheme.typography.labelLarge, color = FreeModelColor, modifier = Modifier.padding(vertical = 4.dp))
                        freeModels.forEach { model ->
                            ModelRow(model = model, onToggle = onToggleModel)
                        }
                    }

                    // Paid models section
                    if (paidModels.isNotEmpty()) {
                        Row(modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                            Text("Paid Models", style = MaterialTheme.typography.labelLarge, color = PaidModelColor)
                            Row {
                                TextButton(onClick = { onEnableAllPaid(provider.id) }) { Text("Turn On All") }
                                TextButton(onClick = { onToggleAllPaid(provider.id) }) { Text("Turn Off All") }
                            }
                        }
                        paidModels.forEach { model ->
                            ModelRow(model = model, onToggle = onToggleModel)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun ModelRow(model: AiModel, onToggle: (String, Boolean) -> Unit) {
    Row(modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(modifier = Modifier.weight(1f)) {
            Text(model.displayName, style = MaterialTheme.typography.bodyMedium)
            if (model.description.isNotEmpty()) {
                Text(model.description, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (!model.isFree) {
            Text("Paid", style = MaterialTheme.typography.labelSmall, color = PaidModelColor, modifier = Modifier.padding(end = 8.dp))
        }
        Switch(checked = model.isEnabled, onCheckedChange = { onToggle(model.id, it) })
    }
}
