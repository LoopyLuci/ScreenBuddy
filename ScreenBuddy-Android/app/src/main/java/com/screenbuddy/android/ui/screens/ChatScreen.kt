package com.screenbuddy.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Send
import androidx.compose.material3.*
import androidx.compose.runtime.*
import kotlinx.coroutines.delay
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.ScreenBuddyApp
import com.screenbuddy.android.data.model.ChatMessage
import com.screenbuddy.android.viewmodel.ChatViewModel

@Composable
private fun rememberChatViewModel(): ChatViewModel {
    val context = LocalContext.current
    val app = context.applicationContext as ScreenBuddyApp
    return remember {
        ChatViewModel(
            aiService = app.aiService,
            apiKeyDao = app.database.apiKeyDao(),
            modelDao = app.database.modelDao(),
            rag = app.ragPipeline,
            agentRuntime = app.agentRuntime
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(
    enabledModels: List<com.screenbuddy.android.data.model.AiModel> = emptyList(),
    selectedModelId: String = "",
    onModelSelected: (String) -> Unit = {},
    ttsEnabled: Boolean = false
) {
    val viewModel = rememberChatViewModel()
    val uiState by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    val listState = rememberLazyListState()

    val app = remember { context.applicationContext as ScreenBuddyApp }
    val tts = remember { app.ttsEngine }
    val engine = remember { com.screenbuddy.android.data.control.ControlBus.creatures }
    val settings by app.settingsRepository.settings.collectAsState(initial = null)

    // Seed one creature so there is something to see, and step the simulation so
    // it moves. Both read the settings the audit found had no consumer.
    // A monotonically increasing frame counter, so the overlay re-reads the
    // engine every frame. Deriving the tick from engine values instead meant it
    // never changed, and an externally set animation never appeared.
    var frame by remember { mutableLongStateOf(0L) }
    LaunchedEffect(Unit) {
        if (engine.count() == 0) {
            engine.add("companion-bird-01", 120f, 260f)
        }
        val targetFps = 30
        while (true) {
            val speed = settings?.animationSpeed ?: 1f
            val enabled = settings?.animationsEnabled ?: true
            engine.step(if (enabled) 1f / targetFps * speed.coerceIn(0.25f, 4f) else 0f)
            frame++
            delay(1000L / targetFps)
        }
    }

    // Restore the persisted model selection once models are known.
    LaunchedEffect(enabledModels, selectedModelId) {
        if (enabledModels.isEmpty()) return@LaunchedEffect
        val target = enabledModels.firstOrNull { it.id == selectedModelId }
            ?: enabledModels.first()
        if (uiState.selectedModel?.id != target.id) {
            viewModel.setModel(target)
        }
    }

    LaunchedEffect(uiState.messages.size) {
        if (uiState.messages.isNotEmpty()) {
            listState.animateScrollToItem(uiState.messages.size - 1)
        }
    }

    // Speak each new assistant reply when TTS is on.
    var lastSpokenId by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(uiState.messages.lastOrNull()?.id) {
        val last = uiState.messages.lastOrNull() ?: return@LaunchedEffect
        if (ttsEnabled && last.role == "assistant" && last.id != lastSpokenId) {
            lastSpokenId = last.id
            tts.speak(last.content)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("ScreenBuddy")
                        val model = uiState.selectedModel
                        if (model != null) {
                            Text(
                                model.displayName,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    }
                },
                actions = {
                    if (enabledModels.size > 1) {
                        var open by remember { mutableStateOf(false) }
                        IconButton(onClick = { open = true }) {
                            Icon(Icons.Filled.ExpandMore, contentDescription = "Change model")
                        }
                        if (open) {
                            ModelChooser(
                                models = enabledModels,
                                selectedId = uiState.selectedModel?.id,
                                onSelect = {
                                    viewModel.setModel(it)
                                    onModelSelected(it.id)
                                    open = false
                                },
                                onDismiss = { open = false }
                            )
                        }
                    }
                    IconButton(onClick = viewModel::clearMessages, enabled = uiState.messages.isNotEmpty()) {
                        Icon(Icons.Filled.Close, contentDescription = "Clear conversation")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp)
        ) {
            if (uiState.error != null) {
                ErrorCard(uiState.error!!, viewModel::dismissError)
            }

            // The creature lives above the conversation: this is what
            // move_creature, set_animation and set_creature_visible now change.
            com.screenbuddy.android.ui.overlay.CreatureOverlay(
                engine = engine,
                tick = frame,
                showCreatures = settings?.showCreatures ?: true,
                animationsEnabled = settings?.animationsEnabled ?: true,
                animationSpeed = settings?.animationSpeed ?: 1f
            )

            LazyColumn(
                state = listState,
                modifier = Modifier.weight(1f).fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                contentPadding = PaddingValues(vertical = 8.dp)
            ) {
                if (uiState.messages.isEmpty()) {
                    item { EmptyState(uiState.selectedModel == null) }
                }
                items(uiState.messages, key = { it.id }) { msg ->
                    MessageBubble(msg)
                }
                if (uiState.isLoading) {
                    item {
                        Row(
                            Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.Center
                        ) {
                            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                        }
                    }
                }
            }

            if (uiState.selectedModel == null) {
                Text(
                    "Enable a model in the Providers tab to start chatting.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
            }

            Row(
                Modifier.fillMaxWidth().padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                OutlinedTextField(
                    value = uiState.inputText,
                    onValueChange = viewModel::updateInput,
                    modifier = Modifier.weight(1f),
                    placeholder = { Text("Type a message...") },
                    maxLines = 4,
                    enabled = !uiState.isLoading
                )
                Spacer(Modifier.width(8.dp))
                IconButton(
                    onClick = viewModel::sendMessage,
                    enabled = uiState.inputText.isNotBlank() &&
                        uiState.selectedModel != null &&
                        !uiState.isLoading
                ) {
                    Icon(Icons.Filled.Send, contentDescription = "Send")
                }
            }
        }
    }
}

@Composable
private fun EmptyState(noModel: Boolean) {
    Column(
        Modifier.fillMaxWidth().padding(vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            if (noModel) "No model selected" else "Ask ScreenBuddy anything",
            style = MaterialTheme.typography.titleMedium
        )
        Spacer(Modifier.height(8.dp))
        Text(
            if (noModel) {
                "Pick a provider and enable one of its models to begin."
            } else {
                "Chat runs against whichever provider you configured."
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}

@Composable
private fun ErrorCard(message: String, onDismiss: () -> Unit) {
    Card(
        Modifier.fillMaxWidth().padding(vertical = 8.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)
    ) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                message,
                Modifier.weight(1f),
                color = MaterialTheme.colorScheme.onErrorContainer,
                style = MaterialTheme.typography.bodySmall
            )
            IconButton(onClick = onDismiss) {
                Icon(Icons.Filled.Close, contentDescription = "Dismiss")
            }
        }
    }
}

@Composable
private fun MessageBubble(message: ChatMessage) {
    val isUser = message.role == "user"
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start
    ) {
        Column(
            Modifier
                .widthIn(max = 300.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(
                    if (isUser) MaterialTheme.colorScheme.primaryContainer
                    else MaterialTheme.colorScheme.surfaceVariant
                )
                .padding(12.dp)
        ) {
            Text(
                message.content,
                style = MaterialTheme.typography.bodyMedium,
                color = if (isUser) MaterialTheme.colorScheme.onPrimaryContainer
                else MaterialTheme.colorScheme.onSurfaceVariant
            )
            if (!message.providerUsed.isNullOrBlank()) {
                Text(
                    message.providerUsed,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModelChooser(
    models: List<com.screenbuddy.android.data.model.AiModel>,
    selectedId: String?,
    onSelect: (com.screenbuddy.android.data.model.AiModel) -> Unit,
    onDismiss: () -> Unit
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Change model") },
        text = {
            LazyColumn {
                items(models, key = { it.id }) { model ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(model) }
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
                            Text("✓", fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } }
    )
}