package com.screenbuddy.android.data.repository

import com.screenbuddy.android.data.local.KeyCipher
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Key storage must round-trip correctly whether or not the Android Keystore is
 * usable. On the JVM there is no Keystore, so these cover the plaintext fallback
 * and prove a missing Keystore degrades rather than losing keys.
 */
class ProviderRepositoryKeyStorageTest {

    private lateinit var apiKeyDao: FakeApiKeyDao
    private lateinit var modelDao: FakeModelDao

    @Before
    fun setUp() {
        apiKeyDao = FakeApiKeyDao()
        modelDao = FakeModelDao()
    }

    @Test
    fun `key round-trips when no keystore is available`() = runTest {
        val repo = ProviderRepository(apiKeyDao, modelDao)
        assertFalse("JVM has no keystore", KeyCipher().isAvailable)

        repo.setApiKey("openai", "sk-plain")
        assertEquals("sk-plain", repo.getApiKey("openai"))
    }

    @Test
    fun `stored value is readable back through a new repository`() = runTest {
        ProviderRepository(apiKeyDao, modelDao).setApiKey("openai", "sk-value")
        // A fresh instance proves the value came from storage.
        val reopened = ProviderRepository(apiKeyDao, modelDao)
        assertEquals("sk-value", reopened.getApiKey("openai"))
    }

    @Test
    fun `hasStoredKey mirrors getApiKey`() = runTest {
        val repo = ProviderRepository(apiKeyDao, modelDao)
        assertFalse(repo.hasStoredKey("openai"))
        repo.setApiKey("openai", "sk-1")
        assertTrue(repo.hasStoredKey("openai"))
        repo.removeApiKey("openai")
        assertFalse(repo.hasStoredKey("openai"))
    }

    @Test
    fun `blank key is stored but reads back as absent`() = runTest {
        val repo = ProviderRepository(apiKeyDao, modelDao)
        repo.setApiKey("openai", "   ")
        assertNull(repo.getApiKey("openai"))
        assertFalse(repo.hasStoredKey("openai"))
    }

    @Test
    fun `removing one key leaves others intact`() = runTest {
        val repo = ProviderRepository(apiKeyDao, modelDao)
        repo.setApiKey("openai", "a")
        repo.setApiKey("mistral", "b")
        repo.removeApiKey("openai")
        assertNull(repo.getApiKey("openai"))
        assertEquals("b", repo.getApiKey("mistral"))
    }

    @Test
    fun `values with newlines and spaces survive the round trip`() = runTest {
        val repo = ProviderRepository(apiKeyDao, modelDao)
        repo.setApiKey("anthropic", " sk-ant-xyz \n")
        assertEquals("sk-ant-xyz", repo.getApiKey("anthropic"))
    }
}