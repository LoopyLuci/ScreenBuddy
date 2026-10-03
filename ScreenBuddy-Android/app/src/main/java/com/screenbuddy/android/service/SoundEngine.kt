package com.screenbuddy.android.service

import android.content.Context
import android.media.AudioAttributes
import android.media.SoundPool
import android.util.Log
import com.screenbuddy.android.data.creature.CreatureEngine

/**
 * Creature and interface sounds.
 *
 * Three settings - sound_effects_enabled, creature_sounds_enabled and
 * effects_volume - had no consumer, because the only audio output was TTS.
 * This gives them one.
 *
 * The two switches are honoured separately: a user can silence the UI without
 * also silencing their creature, which is what the settings screen implies.
 *
 * SoundPool is used rather than MediaPlayer because these are short effects that
 * may overlap; MediaPlayer is for longer playback.
 */
class SoundEngine(context: Context) {

    private val appContext = context.applicationContext
    private val pool: SoundPool? = try {
        SoundPool.Builder()
            .setMaxStreams(4)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_ASSISTANCE_SONIFICATION)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build()
            )
            .build()
    } catch (e: Exception) {
        // No audio hardware, or the platform refused. Sounds are optional.
        Log.w(TAG, "no audio output available: ${e.message}")
        null
    }

    private val soundIds = mutableMapOf<String, Int>()

    /** Whether effects are on at all, from the two switches combined. */
    @Volatile
    var effectsEnabled: Boolean = true

    /** UI sounds specifically, from sound_effects_enabled. */
    @Volatile
    var uiSoundsEnabled: Boolean = true

    /** Creature sounds specifically, from creature_sounds_enabled. */
    @Volatile
    var creatureSoundsEnabled: Boolean = true

    /** 0..1, from effects_volume scaled by master_volume. */
    @Volatile
    var volume: Float = 0.8f

    /**
     * Load a sound from res/raw, or from assets as a fallback.
     *
     * @return true when it can be played.
     */
    fun load(name: String, rawResource: Int): Boolean {
        val target = pool ?: return false
        if (soundIds.containsKey(name)) return true
        return try {
            // Raw resources first; a missing one falls back to assets so a sound
            // can live in either place during development.
            val id = try {
                target.load(appContext, rawResource, 1)
            } catch (e: Exception) {
                appContext.assets.openFd("sounds/$name.ogg").use { afd ->
                    target.load(afd, 1)
                }
            }
            soundIds[name] = id
            true
        } catch (e: Exception) {
            Log.i(TAG, "sound '$name' is not present; skipping")
            false
        }
    }

    /**
     * Play [name] if it is loaded and the relevant switch allows it.
     *
     * Returns whether it was played, so a caller can report honestly rather
     * than assuming a sound played.
     */
    fun play(name: String, rate: Float = 1f): Boolean {
        if (!effectsEnabled) return false
        val id = soundIds[name] ?: return false
        val target = pool ?: return false
        return try {
            target.play(
                id,
                volume.coerceIn(0f, 1f),
                volume.coerceIn(0f, 1f),
                1,
                0,
                rate.coerceIn(0.5f, 2f)
            )
            true
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Play the sound matching a creature's animation state.
     *
     * Movement gets a quieter sound than a celebration, so a wandering creature
     * is not annoying.
     */
    fun playForAnimation(state: String): Boolean {
        if (!creatureSoundsEnabled) return false
        return when (state) {
            CreatureEngine.ANIMATION_CELEBRATE -> play("celebrate")
            CreatureEngine.ANIMATION_WALK -> play("step", rate = 1.4f)
            CreatureEngine.ANIMATION_FLY -> play("flutter")
            CreatureEngine.ANIMATION_SLEEP -> play("snore")
            else -> false
        }
    }

    /** A UI click, honouring the UI switch. */
    fun playUiClick(): Boolean {
        if (!uiSoundsEnabled) return false
        return play("click")
    }

    /** Names currently loaded, for the settings screen and for tests. */
    fun loadedSounds(): Set<String> = soundIds.keys.toSet()

    /** Release the pool. Safe to call more than once. */
    fun release() {
        try {
            soundIds.clear()
            pool?.release()
        } catch (e: Exception) {
            Log.w(TAG, "release failed: ${e.message}")
        }
    }

    companion object {
        private const val TAG = "ScreenBuddySound"
    }
}