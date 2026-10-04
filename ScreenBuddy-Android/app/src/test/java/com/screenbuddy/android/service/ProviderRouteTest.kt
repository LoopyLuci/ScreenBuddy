package com.screenbuddy.android.service

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Guards the routing table. The bug these tests exist for: an unknown provider
 * used to fall through to an `else` branch that POSTed the user's key to
 * api.openai.com, so a Mistral or DeepSeek credential was sent to OpenAI.
 */
class ProviderRouteTest {

    @Test
    fun `resolves every provider in the catalogue`() {
        val catalogueProviderIds = listOf(
            "ollama", "openai", "anthropic", "openrouter", "together", "deepseek",
            "mistral", "cohere", "perplexity", "replicate", "huggingface", "google",
            "grok", "unsloth", "llmstudio", "vllm", "llamacpp", "lmstudio",
            "textgen", "koboldcpp", "opencode-go", "opencode-zen"
        )
        val unresolved = catalogueProviderIds.filter { ProviderRoute.resolve(it) == null }
        assertTrue("unrouted providers: $unresolved", unresolved.isEmpty())
    }

    @Test
    fun `no provider other than openai routes to the OpenAI host`() {
        val openAiHosts = setOf("https://api.openai.com/v1")
        catalogueProviders().forEach { (providerId, route) ->
            if (providerId != "openai") {
                assertFalse(
                    "$providerId must not use the OpenAI host",
                    route.baseUrl in openAiHosts
                )
            }
        }
    }

    @Test
    fun `each provider has a distinct host`() {
        // Catches copy-paste mistakes where two vendors share a base URL.
        val hosted = catalogueProviders().filter { it.second.baseUrl.isNotEmpty() }
        val byHost = hosted.groupBy({ it.second.baseUrl }, { it.first })
        val collisions = byHost.filterValues { it.size > 1 }
        assertTrue("providers sharing a host: $collisions", collisions.isEmpty())
    }

    @Test
    fun `anthropic uses its own api style`() {
        val route = ProviderRoute.resolve("anthropic")
        assertNotNull(route)
        assertEquals(ApiStyle.ANTHROPIC, route!!.apiStyle)
        assertTrue(route.requiresKey)
    }

    @Test
    fun `ollama needs no key`() {
        val route = ProviderRoute.resolve("ollama")
        assertNotNull(route)
        assertEquals(ApiStyle.OLLAMA, route!!.apiStyle)
        assertFalse(route.requiresKey)
    }

    @Test
    fun `cloud providers require a key`() {
        listOf("openai", "anthropic", "deepseek", "mistral", "grok").forEach { id ->
            assertTrue("$id should require a key", ProviderRoute.resolve(id)!!.requiresKey)
        }
    }

    @Test
    fun `self-hosted providers need no key and no hardcoded host`() {
        // These must be user-configured; guessing a host would be wrong.
        listOf("llamacpp", "lmstudio", "vllm", "koboldcpp", "textgen").forEach { id ->
            val route = ProviderRoute.resolve(id)!!
            assertFalse("$id should not require a key", route.requiresKey)
            assertTrue("$id should not hardcode a host", route.baseUrl.isEmpty())
        }
    }

    @Test
    fun `longest provider id wins for compound ids`() {
        // "opencode-zen" must not resolve to "opencode-go"/"opencode".
        assertEquals("opencode-zen", ProviderRoute.resolve("opencode-zen")!!.providerId)
    }

    @Test
    fun `unknown provider returns null rather than a default route`() {
        // The critical guard: never fall back to a real vendor's endpoint.
        assertNull(ProviderRoute.resolve("totally-made-up-provider"))
        assertNull(ProviderRoute.forProvider("totally-made-up-provider"))
    }

    @Test
    fun `mistral and deepseek route to their own vendors`() {
        assertEquals("https://api.mistral.ai/v1", ProviderRoute.resolve("mistral")!!.baseUrl)
        assertEquals("https://api.deepseek.com/v1", ProviderRoute.resolve("deepseek")!!.baseUrl)
    }

    @Test
    fun `service reports the endpoint it will call`() {
        val service = AiService()
        assertEquals("https://api.mistral.ai/v1", service.endpointFor("mistral"))
        assertNull(service.endpointFor("nope"))
    }

    @Test
    fun `the configured local endpoint beats the built-in host`() {
        // This ordering was the bug: a local provider resolved its built-in
        // loopback first, so the endpoint setting could never take effect.
        val service = AiService(initialBaseUrl = "http://192.168.1.9:11434")
        assertEquals("http://192.168.1.9:11434", service.endpointFor("ollama"))
    }

    @Test
    fun `base url override wins over the built-in host`() {
        val service = AiService(initialBaseUrlOverrides = mapOf("llamacpp" to "http://192.168.1.5:8080"))
        assertEquals("http://192.168.1.5:8080", service.endpointFor("llamacpp"))
    }

    @Test
    fun `a model the provider serves is usable even if the catalogue lacks it`() {
        // send_chat resolved names against the built-in catalogue only, so a
        // model pulled from Ollama - "qwen2.5:0.5b" - was rejected as unknown
        // and the app could never talk to a real provider. The catalogue is a
        // convenience list, not the set of callable models.
        val resolved = com.screenbuddy.android.data.control.ControlCommands
            .resolveModelForTest("qwen2.5:0.5b")
        assertEquals("qwen2.5:0.5b", resolved.name)
        assertEquals("ollama", resolved.providerId)
        assertTrue("a local model is free", resolved.isFree)
    }

    @Test
    fun `a catalogue model keeps its own provider`() {
        val resolved = com.screenbuddy.android.data.control.ControlCommands
            .resolveModelForTest("openai-gpt-4o")
        assertEquals("openai", resolved.providerId)
    }

    private fun catalogueProviders(): List<Pair<String, ProviderRoute>> = ProviderRoute.entries
        .map { it.providerId to it }
}