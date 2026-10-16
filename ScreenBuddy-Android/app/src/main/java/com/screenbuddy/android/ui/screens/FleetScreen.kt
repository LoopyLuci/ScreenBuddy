package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalContext
import com.google.gson.JsonObject
import com.screenbuddy.android.ScreenBuddyApp
import com.screenbuddy.android.data.abp.AbpResult
import com.screenbuddy.android.viewmodel.FleetViewModel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * The phone's remote control for the desktop fleet (master plan v2,
 * Pattern F). The extension routes are app-agnostic, so everything here
 * is driven by the same scoped key the desktop uses: the creature's live
 * status, the router's recommendation, and the fine-tune pipeline's state.
 *
 * An unconfigured fleet (no URL or token in settings) is a screen that
 * says so — never a fake number.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FleetScreen(viewModel: FleetViewModel) {
    val context = LocalContext.current
    val app = context.applicationContext as ScreenBuddyApp
    val settingsRepository = app.settingsRepository
    val scope = rememberCoroutineScope()

    var fleetUrl by remember { mutableStateOf("") }
    var fleetToken by remember { mutableStateOf("") }

    // Seed the fields from settings once, so a configured fleet just works.
    LaunchedEffect(Unit) {
        val s = settingsRepository.settings.first()
        fleetUrl = s.fleetBaseUrl
        fleetToken = s.fleetToken
    }

    val status by viewModel.status.collectAsState()
    val recommendation by viewModel.recommendation.collectAsState()
    val finetune by viewModel.finetune.collectAsState()
    val busy by viewModel.busy.collectAsState()

    val configured = fleetUrl.isNotBlank() && fleetToken.isNotBlank()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Fleet", style = MaterialTheme.typography.headlineMedium)

        // --- connection ---------------------------------------------------------
        Card(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Fleet connection", style = MaterialTheme.typography.titleMedium)
                OutlinedTextField(
                    value = fleetUrl,
                    onValueChange = { fleetUrl = it },
                    label = { Text("Dashboard URL") },
                    placeholder = { Text("http://192.168.1.10:8787") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
                OutlinedTextField(
                    value = fleetToken,
                    onValueChange = { fleetToken = it },
                    label = { Text("Token / integration key") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
                Button(
                    onClick = {
                        scope.launch {
                            settingsRepository.setFleetBaseUrl(fleetUrl.trim())
                            settingsRepository.setFleetToken(fleetToken.trim())
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Save connection") }
            }
        }

        if (!configured) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(16.dp),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Not connected", style = MaterialTheme.typography.titleMedium)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "Enter the desktop dashboard's URL and a token (or a minted " +
                            "'screenbuddy' integration key) to control the fleet from your phone.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            return@Column
        }

        // --- creature status ----------------------------------------------------
        Card(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Creature", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                    if (!busy) {
                        TextButton(onClick = { viewModel.refreshStatus() }) { Text("Refresh") }
                    }
                }
                when (val s = status) {
                    null -> Text("—", style = MaterialTheme.typography.bodyMedium)
                    is AbpResult.Error -> Text(
                        "fleet error: ${s.message}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    is AbpResult.Ok -> StatusRows(s.data)
                }
            }
        }

        // --- router recommendation ------------------------------------------------
        var task by remember { mutableStateOf("") }
        Card(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Ask the router", style = MaterialTheme.typography.titleMedium)
                OutlinedTextField(
                    value = task,
                    onValueChange = { task = it },
                    label = { Text("Task") },
                    placeholder = { Text("write a haiku") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
                Button(
                    onClick = { if (task.isNotBlank()) viewModel.recommend(task) },
                    enabled = task.isNotBlank() && !busy,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Recommend a model") }
                if (recommendation != null) {
                    Text(recommendation!!, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }

        // --- fine-tune -------------------------------------------------------------
        Card(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Fine-tune", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                    if (!busy) {
                        TextButton(onClick = { viewModel.refreshFinetune() }) { Text("Refresh") }
                    }
                }
                when (val f = finetune) {
                    null -> Text("—", style = MaterialTheme.typography.bodyMedium)
                    is AbpResult.Error -> Text(
                        "fleet error: ${f.message}",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    is AbpResult.Ok -> {
                        val backend = f.data.getAsJsonObject("backend")
                        val trainable = backend?.get("trainable")?.asBoolean == true
                        Text(
                            if (trainable) "backend: trainable" else "backend: ${backend?.get("reason")?.asString ?: "unknown"}",
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        val datasets = f.data.getAsJsonArray("datasets")
                        Text("datasets: ${datasets?.size() ?: 0}", style = MaterialTheme.typography.bodyMedium)
                        val adapter = f.data.get("adapter")
                        Text(
                            if (adapter != null && !adapter.isJsonNull) "adapter: ${adapter.asString}" else "adapter: none yet",
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            }
        }
    }
}

/** The creature's live snapshot as label: value rows. */
@Composable
private fun StatusRows(status: JsonObject) {
    val rows = listOf(
        "fps" to status.get("fps"),
        "uptime" to status.get("uptime_secs"),
        "creatures" to status.get("creature_count"),
        "models" to status.get("model_count"),
        "tools" to status.get("tool_count"),
        "memory chunks" to status.get("memory_chunks"),
    )
    for ((label, value) in rows) {
        Row(modifier = Modifier.fillMaxWidth()) {
            Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Text(
                value?.let { if (it.isJsonNull) "?" else it.asString } ?: "?",
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}
