package com.screenbuddy.android.data.model

import java.util.UUID

data class CreatureData(
    /**
     * Stable identifier, used by agent profiles and by the control surface.
     *
     * This was a random UUID, which meant the ids the rest of the app uses -
     * "companion-bird-01" and friends - matched no catalogue entry, and every
     * lookup fell back silently. A defaulting parameter would repeat that; ids
     * are required so a catalogue entry cannot be anonymous.
     */
    val id: String,
    val name: String,
    val species: String,
    val personality: String,
    val colorHex: String,
    val animationStates: List<AnimationState> = emptyList(),
    val isUnlocked: Boolean = true,
    val modelPreference: String? = null
)

data class AnimationState(
    val name: String,
    val frames: List<String> = emptyList(),
    val duration: Float = 0.1f,
    val loop: Boolean = true
)

object CreatureDefaults {
    val creatures = listOf(
        CreatureData(
            id = "companion-dog-01",
            name = "Buddy",
            species = "Dog",
            personality = "Loyal, cheerful, always ready to help",
            colorHex = "#F5A623",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "robo-cat-01",
            name = "Whiskers",
            species = "Cat",
            personality = "Curious, independent, loves to explore",
            colorHex = "#7B68EE",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "companion-bird-01",
            name = "Hoot",
            species = "Owl",
            personality = "Wise, thoughtful, answers questions",
            colorHex = "#8B4513",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "pixel-wizard-01",
            name = "Spike",
            species = "Dinosaur",
            personality = "Energetic, playful, loves adventure",
            colorHex = "#2ECC71",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "cosmic-jellyfish-01",
            name = "Bubbles",
            species = "Fish",
            personality = "Calm, soothing, peaceful presence",
            colorHex = "#3498DB",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "slime-king-01",
            name = "Flutter",
            species = "Butterfly",
            personality = "Colorful, graceful, brings joy",
            colorHex = "#E74C3C",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        ),
        CreatureData(
            id = "ghost-01",
            name = "Shadow",
            species = "Raven",
            personality = "Mysterious, clever, deep thinker",
            colorHex = "#2C3E50",
            animationStates = listOf(
                AnimationState(name = "idle"),
                AnimationState(name = "happy"),
                AnimationState(name = "thinking"),
                AnimationState(name = "alert"),
                AnimationState(name = "sleeping")
            )
        )
    )
}