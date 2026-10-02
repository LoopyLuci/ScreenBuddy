package com.screenbuddy.android.data.model

import java.util.UUID

data class CreatureData(
    val id: String = UUID.randomUUID().toString(),
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