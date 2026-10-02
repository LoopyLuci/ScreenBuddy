package com.screenbuddy.android.data.local

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Encrypts API keys at rest with an AES-256-GCM key held in the Android Keystore.
 *
 * The key never leaves the keystore, so the database only ever holds ciphertext;
 * a copied `app.db` file yields nothing usable. No third-party dependency is
 * needed. Values are stored as `v1:<base64(iv|ciphertext)>` so the format can
 * change later without breaking existing rows.
 *
 * If the keystore is unavailable (some rooted/older devices, or a JVM unit-test
 * context) [isAvailable] is false and callers should fall back to plaintext
 * rather than crash.
 */
class KeyCipher {

    private val keyStore: KeyStore? = runCatching {
        KeyStore.getInstance(KEYSTORE).apply { load(null) }
    }.getOrNull()

    /** True when hardware-backed or software Keystore encryption is usable. */
    val isAvailable: Boolean
        get() = runCatching { secretKey() != null }.getOrDefault(false)

    private fun secretKey(): SecretKey? {
        val store = keyStore ?: return null
        val existing = store.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry
        if (existing != null) return existing.secretKey

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }

    /** Encrypt [plaintext], returning a storable string. Null on failure. */
    fun encrypt(plaintext: String): String? = runCatching {
        val key = secretKey() ?: return null
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key)
        val iv = cipher.iv
        val encrypted = cipher.doFinal(plaintext.toByteArray(Charsets.UTF_8))
        val combined = iv + encrypted
        PREFIX + Base64.encodeToString(combined, Base64.NO_WRAP)
    }.getOrNull()

    /**
     * Decrypt a stored value. Returns the input unchanged when it is not
     * ciphertext, so rows written before encryption was enabled still work.
     */
    fun decrypt(stored: String): String = runCatching {
        if (!stored.startsWith(PREFIX)) return stored
        val payload = Base64.decode(stored.removePrefix(PREFIX), Base64.NO_WRAP)
        val iv = payload.copyOfRange(0, IV_LENGTH)
        val body = payload.copyOfRange(IV_LENGTH, payload.size)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(TAG_LENGTH_BITS, iv))
        String(cipher.doFinal(body), Charsets.UTF_8)
    }.getOrDefault(stored)

    /** True when [stored] looks like an encrypted value from this cipher. */
    fun isEncrypted(stored: String): Boolean = stored.startsWith(PREFIX)

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "screenbuddy_apikeys"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val PREFIX = "v1:"
        const val IV_LENGTH = 12 // GCM standard nonce length
        const val TAG_LENGTH_BITS = 128
    }
}