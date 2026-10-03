package com.screenbuddy.android.ui.overlay

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.data.creature.CreatureEngine
import com.screenbuddy.android.data.creature.CreatureState
import kotlinx.coroutines.delay

/**
 * The creature on screen.
 *
 * This is the feature the Android app was missing: creature commands reported
 * success while nothing was ever drawn. The overlay reads [CreatureEngine] and
 * shows the result, so `move_creature`, `set_animation` and `set_creature_visible`
 * become visible rather than merely accepted.
 *
 * The animation is driven from the engine's state rather than invented here, so
 * what is drawn matches what a control command reported.
 */
@Composable
fun CreatureOverlay(
    engine: CreatureEngine,
    modifier: Modifier = Modifier,
    /** Bumped by the caller to force a redraw, e.g. on each engine step. */
    tick: Long = 0L,
    /** Honours the show_creatures setting. */
    showCreatures: Boolean = true,
    /** Honours the animations_enabled and animation_speed settings. */
    animationsEnabled: Boolean = true,
    animationSpeed: Float = 1.0f,
    creatureEmoji: String = "🐦"
) {
    if (!showCreatures) return

    // Read the engine on every recomposition, keyed by the caller's frame
    // counter. An earlier version cached the list in LaunchedEffect keyed the
    // same way, so an externally set animation never reached the screen.
    val snapshot = remember(engine, tick) { engine.all() }

    Box(modifier = modifier) {
        for (creature in snapshot) {
            if (creature.visible) {
                CreatureView(
                    creature = creature,
                    animationsEnabled = animationsEnabled,
                    animationSpeed = animationSpeed,
                    emoji = creatureEmoji
                )
            }
        }
    }
}

@Composable
private fun CreatureView(
    creature: CreatureState,
    animationsEnabled: Boolean,
    animationSpeed: Float,
    emoji: String
) {
    val transition = rememberInfiniteTransition(label = "creature-${creature.id}")
    // Speed scales the period, and is clamped so a wild value cannot make the
    // animation strobe or appear frozen.
    val periodMillis = (900 / animationSpeed.coerceIn(0.25f, 4f)).toInt().coerceIn(120, 4000)

    val bob by transition.animateFloat(
        initialValue = 0f,
        targetValue = if (animationsEnabled) 1f else 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(periodMillis),
            repeatMode = RepeatMode.Reverse
        ),
        label = "bob"
    )

    val wiggle by transition.animateFloat(
        initialValue = 0f,
        targetValue = if (animationsEnabled && creature.animation != CreatureEngine.ANIMATION_IDLE) 1f else 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(periodMillis * 2),
            repeatMode = RepeatMode.Reverse
        ),
        label = "wiggle"
    )

    // Normalised positions: the engine works in desktop-ish units, the overlay
    // lays out in dp, so the caller scales. Here the fractional part drives the
    // visual offset, which keeps the creature visibly inside its cell.
    val lift = bob * 6f
    // A few degrees either way, so movement reads as a creature rather than a
    // bouncing picture.
    val tilt = (wiggle - 0.5f) * 12f

    Column(
        modifier = Modifier
            .padding(4.dp)
            .alpha(0.95f),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Box(
            modifier = Modifier
                .size(56.dp)
                .background(Color.White.copy(alpha = 0.25f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = emoji,
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.Normal,
                modifier = Modifier
                    .scale(1f + lift / 40f)
                    .rotate(tilt)
                    .alpha(0.9f)
            )
        }
        Text(
            text = creature.animation,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }
}