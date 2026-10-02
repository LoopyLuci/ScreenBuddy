package com.screenbuddy.android.data.repository

import com.screenbuddy.android.data.model.ProviderData
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Regression tests for the stubbed repository: `setApiKey`/`removeApiKey` used
 * to be empty function bodies, so keys entered in the UI were silently discarded
 * while chat read from storage and failed.
 */
class ProviderRepositoryTest {

    private lateinit var apiKeyDao: FakeApiKeyDao
    private lateinit var modelDao: FakeModelDao
    private lateinit var repo: ProviderRepository

    @Before
    fun setUp() {
        apiKeyDao = FakeApiKeyDao()
        modelDao = FakeModelDao()
        repo = ProviderRepository(apiKeyDao, modelDao)
    }

    @Test
    fun `api key is retrievable after being set`() = runTest {
        repo.setApiKey("openai", "sk-test-123")
        assertEquals("sk-test-123", repo.getApiKey("openai"))
    }

    @Test
    fun `api key is trimmed`() = runTest {
        repo.setApiKey("anthropic", "  sk-ant-xyz \n")
        assertEquals("sk-ant-xyz", repo.getApiKey("anthropic"))
    }

    @Test
    fun `removing a key clears it`() = runTest {
        repo.setApiKey("openai", "sk-test")
        repo.removeApiKey("openai")
        assertNull(repo.getApiKey("openai"))
        assertFalse(repo.hasApiKey("openai"))
    }

    @Test
    fun `blank key is treated as absent`() = runTest {
        repo.setApiKey("openai", "   ")
        assertNull(repo.getApiKey("openai"))
        assertFalse(repo.hasApiKey("openai"))
    }

    @Test
    fun `missing key returns null`() = runTest {
        assertNull(repo.getApiKey("never-set"))
        assertFalse(repo.hasApiKey("never-set"))
    }

    @Test
    fun `hasApiKey is true once a key is stored`() = runTest {
        repo.setApiKey("mistral", "sk-m")
        assertTrue(repo.hasApiKey("mistral"))
    }

    @Test
    fun `keys for different providers do not collide`() = runTest {
        repo.setApiKey("openai", "sk-openai")
        repo.setApiKey("mistral", "sk-mistral")
        assertEquals("sk-openai", repo.getApiKey("openai"))
        assertEquals("sk-mistral", repo.getApiKey("mistral"))
    }

    @Test
    fun `setting a key twice replaces it`() = runTest {
        repo.setApiKey("openai", "first")
        repo.setApiKey("openai", "second")
        assertEquals("second", repo.getApiKey("openai"))
    }

    @Test
    fun `clearAllApiKeys wipes every key`() = runTest {
        repo.setApiKey("openai", "a")
        repo.setApiKey("anthropic", "b")
        repo.clearAllApiKeys()
        assertFalse(repo.hasApiKey("openai"))
        assertFalse(repo.hasApiKey("anthropic"))
    }

    @Test
    fun `providersWithKeys lists only configured providers`() = runTest {
        repo.setApiKey("openai", "sk-1")
        repo.setApiKey("mistral", "sk-2")
        repo.setApiKey("empty-one", "  ")
        val ids = repo.providersWithKeys().first()
        assertEquals(setOf("openai", "mistral"), ids.toSet())
    }

    @Test
    fun `seedIfEmpty populates the catalogue`() = runTest {
        repo.seedIfEmpty()
        assertEquals(expectedSeedSize(), modelDao.count())
    }

    @Test
    fun `seedIfEmpty is idempotent`() = runTest {
        repo.seedIfEmpty()
        val afterFirst = modelDao.count()
        repo.seedIfEmpty()
        assertEquals(afterFirst, modelDao.count())
    }

    @Test
    fun `seeding records the owning provider id`() = runTest {
        repo.seedIfEmpty()
        // "cohere-command-r" must belong to cohere, not some other provider.
        val cohere = modelDao.snapshot().first { it.id == "cohere-command-r" }
        assertEquals("cohere", cohere.providerId)
    }

    @Test
    fun `toggleModel persists the change`() = runTest {
        repo.seedIfEmpty()
        repo.toggleModel("openai-gpt-4o", true)
        val gpt4o = repo.providers.first().flatMap { it.models }.first { it.id == "openai-gpt-4o" }
        assertTrue("model should be enabled", gpt4o.isEnabled)

        repo.toggleModel("openai-gpt-4o", false)
        val after = repo.providers.first().flatMap { it.models }.first { it.id == "openai-gpt-4o" }
        assertFalse("model should be disabled", after.isEnabled)
    }

    @Test
    fun `toggling an unknown model is a no-op`() = runTest {
        repo.seedIfEmpty()
        val before = modelDao.count()
        repo.toggleModel("does-not-exist", true)
        assertEquals(before, modelDao.count())
    }

    @Test
    fun `enableAllModels affects only the named provider`() = runTest {
        repo.seedIfEmpty()
        repo.enableAllModels("mistral")
        val providers = repo.providers.first()
        assertTrue(
            "all mistral models enabled",
            providers.first { it.id == "mistral" }.models.all { it.isEnabled }
        )
        // Other providers keep their catalogue defaults (OpenAI ships some off).
        assertTrue(
            "other providers unchanged",
            providers.first { it.id == "openai" }.models.any { !it.isEnabled }
        )
    }

    @Test
    fun `disableAllModels affects only the named provider`() = runTest {
        repo.seedIfEmpty()
        repo.enableAllModels("mistral")
        repo.disableAllModels("mistral")
        assertTrue(
            "all mistral models disabled",
            repo.providers.first().first { it.id == "mistral" }.models.none { it.isEnabled }
        )
    }

    @Test
    fun `enableAllFreeModels only touches free models`() = runTest {
        repo.seedIfEmpty()
        repo.enableAllFreeModels("openai")
        val models = repo.providers.first().first { it.id == "openai" }.models
        assertTrue(models.filter { it.isFree }.all { it.isEnabled })
    }

    @Test
    fun `providers flow preserves catalogue metadata`() = runTest {
        repo.seedIfEmpty()
        repo.toggleModel("openai-gpt-4o", true)
        val openAi = repo.providers.first().first { it.id == "openai" }
        // Static fields must survive the storage round-trip.
        assertEquals("OpenAI", openAi.name)
        assertEquals("https://openai.com", openAi.website)
        assertNotNull(openAi.models.first().description)
    }

    @Test
    fun `providers flow returns the full catalogue once seeded`() = runTest {
        repo.seedIfEmpty()
        assertEquals(ProviderData.providers.size, repo.providers.first().size)
    }

    @Test
    fun `getProvider finds by id`() {
        assertEquals("Anthropic", repo.getProvider("anthropic")?.name)
        assertNull(repo.getProvider("nope"))
    }

    @Test
    fun `getEnabledModels returns catalogue defaults`() {
        val enabled = repo.getEnabledModels()
        assertTrue(enabled.isNotEmpty())
        assertTrue(enabled.all { it.isEnabled })
    }

    @Test
    fun `getFreeModels returns only free models`() {
        val free = repo.getFreeModels()
        assertTrue(free.isNotEmpty())
        assertTrue(free.all { it.isFree })
    }

    @Test
    fun `catalogue model ids are unique`() {
        // Seeding uses REPLACE on the primary key, so a duplicate id would
        // silently collapse and drop a model.
        val ids = ProviderData.providers.flatMap { it.models }.map { it.id }
        val duplicates = ids.groupingBy { it }.eachCount().filterValues { it > 1 }.keys
        assertTrue("duplicate model ids: $duplicates", duplicates.isEmpty())
    }

    @Test
    fun `catalogue provider ids are unique`() {
        val ids = ProviderData.providers.map { it.id }
        val duplicates = ids.groupingBy { it }.eachCount().filterValues { it > 1 }.keys
        assertTrue("duplicate provider ids: $duplicates", duplicates.isEmpty())
    }

    @Test
    fun `every catalogue model has a non-empty display name and description`() {
        ProviderData.providers.forEach { provider ->
            provider.models.forEach { model ->
                assertTrue("${model.id} has no displayName", model.displayName.isNotBlank())
                assertTrue("${model.id} has no description", model.description.isNotBlank())
            }
        }
    }
}