package com.screenbuddy.android.ui.screens

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithText
import com.screenbuddy.android.data.model.AiModel
import com.screenbuddy.android.data.model.Provider
import com.screenbuddy.android.viewmodel.ProvidersUiState
import org.junit.Rule
import org.junit.Test

/**
 * Instrumented regression tests for the Providers screen.
 *
 * These must run on a device or emulator (`./gradlew connectedDebugAndroidTest`).
 * The defect they guard against is a Compose slot-table corruption that only
 * appears during real composition -- the 77 JVM unit tests all passed while the
 * Providers tab crashed on hardware, so no JVM test could have caught it.
 */
class ProvidersScreenTest {

    @get:Rule
    val composeRule = createComposeRule()

    private fun provider(id: String, name: String) = Provider(
        id = id,
        name = name,
        description = "$name description",
        apiKeyName = "${name.uppercase()}_API_KEY",
        website = "https://example.invalid/$id",
        models = listOf(
            AiModel(
                id = "$id-model",
                providerId = id,
                name = "$name Model",
                displayName = "$name Model",
                description = "A model",
                isEnabled = true
            )
        )
    )

    private fun content(state: androidx.compose.runtime.MutableState<ProvidersUiState>) {
        composeRule.setContent {
            ProvidersContent(
                uiState = state.value,
                onToggleModel = { _, _ -> },
                onSetApiKey = { _, _ -> },
                onRemoveApiKey = { _ -> },
                onEnableAll = { _ -> },
                onDisableAll = { _ -> },
                onErrorShown = {}
            )
        }
    }

    /**
     * Drives the real ProvidersContent through the loading -> loaded transition.
     *
     * The original code used `return@Column` while loading, which swapped the
     * entire child subtree as isLoading flipped to false and threw
     * ArrayIndexOutOfBoundsException in SlotTableKt.key. Reintroducing that early
     * return makes this test crash rather than merely fail.
     */
    @Test
    fun loadingToLoadedTransitionRendersProviders() {
        val state = mutableStateOf(
            ProvidersUiState(isLoading = true, providers = emptyList())
        )
        content(state)
        composeRule.waitForIdle()

        state.value = ProvidersUiState(
            isLoading = false,
            providers = listOf(provider("openai", "OpenAI"), provider("anthropic", "Anthropic"))
        )
        composeRule.waitForIdle()

        composeRule.onNodeWithText("2 providers available").assertIsDisplayed()
        composeRule.onNodeWithText("OpenAI").assertIsDisplayed()
        composeRule.onNodeWithText("Anthropic").assertIsDisplayed()
    }

    @Test
    fun everyProviderRendersWithItsModelCount() {
        val state = mutableStateOf(
            ProvidersUiState(
                isLoading = false,
                providers = listOf(provider("openai", "OpenAI"), provider("ollama", "Ollama"))
            )
        )
        content(state)
        composeRule.waitForIdle()

        composeRule.onNodeWithText("2 providers available").assertIsDisplayed()
        composeRule.onNodeWithText("OpenAI").assertIsDisplayed()
        composeRule.onNodeWithText("Ollama").assertIsDisplayed()
        // "1/1 models enabled" appears once per provider, so assert on the count
        // rather than expecting a single matching node.
        composeRule.onAllNodes(
            hasText("1/1 models enabled")
        ).assertCountEquals(2)
    }

    @Test
    fun emptyProviderListIsHandled() {
        val state = mutableStateOf(
            ProvidersUiState(isLoading = false, providers = emptyList())
        )
        content(state)
        composeRule.waitForIdle()
        composeRule.onNodeWithText("0 providers available").assertIsDisplayed()
    }

    @Test
    fun errorBannerIsDisplayed() {
        val state = mutableStateOf(
            ProvidersUiState(isLoading = false, error = "Something went wrong")
        )
        content(state)
        composeRule.waitForIdle()
        composeRule.onNodeWithText("Something went wrong").assertIsDisplayed()
    }
}
