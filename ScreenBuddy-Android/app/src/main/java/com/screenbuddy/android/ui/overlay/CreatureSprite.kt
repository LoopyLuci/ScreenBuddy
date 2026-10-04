package com.screenbuddy.android.ui.overlay

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.screenbuddy.android.data.creature.CreatureEngine
import kotlin.math.abs
import kotlin.math.sin

/**
 * A creature drawn from primitives.
 *
 * The overlay previously showed a single hardcoded emoji for every creature, so
 * the species, the colour and the animation state in the data model had no
 * visible effect. For a desktop companion the creature is the product, so it is
 * drawn here instead: shapes that respond to the state the engine reports.
 *
 * Deliberately vector rather than sprites. There are no image assets to keep in
 * step with the code, it scales to any density, and a state change is a number
 * changing rather than a frame lookup that can be missing.
 */
@Composable
fun CreatureSprite(
    animation: String,
    /** Base colour from the creature's own data, e.g. #F5A623. */
    color: Color,
    /** Species, which decides the silhouette. */
    species: String,
    animationsEnabled: Boolean,
    animationSpeed: Float,
    size: Dp = 72.dp,
    modifier: Modifier = Modifier
) {
    val speed = animationSpeed.coerceIn(0.25f, 4f)

    val transition = rememberInfiniteTransition(label = "sprite-$animation")
    // One shared phase drives everything, so parts move together rather than
    // drifting apart independently.
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = if (animationsEnabled) 1f else 0f,
        animationSpec = infiniteRepeatable(
            animation = tween((900 / speed).toInt().coerceIn(120, 4000), easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "phase"
    )
    // Slower and independent, for secondary motion such as blinking.
    val slow by transition.animateFloat(
        initialValue = 0f,
        targetValue = if (animationsEnabled) 1f else 0f,
        animationSpec = infiniteRepeatable(
            animation = tween((2600 / speed).toInt().coerceIn(200, 12000), easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "slow"
    )

    // Blink for a fraction of each cycle, so the eye is mostly open.
    val blinking = slow < 0.08f

    Canvas(modifier.size(size)) {
        val w = this.size.width
        val h = this.size.height
        val cx = w / 2f
        val cy = h / 2f

        // Colours derived from the creature's own, so a species change is visible.
        val body = color
        val shade = lerp(color, Color.Black, 0.28f)
        val light = lerp(color, Color.White, 0.35f)

        when (animation) {
            CreatureEngine.ANIMATION_SLEEP -> drawSleeping(cx, cy, w, h, body, shade, light)
            CreatureEngine.ANIMATION_CELEBRATE ->
                drawCelebrating(cx, cy, w, h, body, shade, light, phase)
            CreatureEngine.ANIMATION_FLY ->
                drawFlying(cx, cy, w, h, body, shade, light, phase)
            CreatureEngine.ANIMATION_WALK ->
                drawWalking(cx, cy, w, h, body, shade, light, phase, species)
            else -> drawIdle(cx, cy, w, h, body, shade, light, phase, blinking, species)
        }
    }
}

/** Shared: the eyes, so every state has them. */
private fun DrawScope.drawEyes(
    cx: Float,
    cy: Float,
    spread: Float,
    radius: Float,
    blinking: Boolean,
    pupilShift: Float = 0f
) {
    val lidColour = Color.White
    for (side in intArrayOf(-1, 1)) {
        val ex = cx + side * spread
        if (blinking) {
            drawLine(
                color = Color(0xFF2B2B2B),
                start = Offset(ex - radius, cy),
                end = Offset(ex + radius, cy),
                strokeWidth = radius * 0.5f
            )
        } else {
            drawCircle(lidColour, radius, Offset(ex, cy))
            drawCircle(Color(0xFF2B2B2B), radius * 0.55f, Offset(ex + pupilShift, cy + radius * 0.1f))
        }
    }
}

/** Ears or a beak, decided by species: this is what makes a cat look like a cat. */
private fun DrawScope.drawSpeciesMark(
    cx: Float,
    cy: Float,
    w: Float,
    /** Canvas height, passed in because this function has no `h` parameter. */
    ch: Float,
    species: String,
    dark: Color
) {
    when (species.lowercase()) {
        "cat" -> {
            val ear = Path().apply {
                moveTo(cx - w * 0.26f, cy - ch * 0.26f)
                lineTo(cx - w * 0.16f, cy - ch * 0.44f)
                lineTo(cx - w * 0.04f, cy - ch * 0.24f)
                close()
            }
            drawPath(ear, dark)
            val ear2 = Path().apply {
                moveTo(cx + w * 0.26f, cy - ch * 0.26f)
                lineTo(cx + w * 0.16f, cy - ch * 0.44f)
                lineTo(cx + w * 0.04f, cy - ch * 0.24f)
                close()
            }
            drawPath(ear2, dark)
        }
        "dog" -> {
            // Floppy ears hanging beside the head.
            drawOval(
                color = dark,
                topLeft = Offset(cx - w * 0.34f, cy - ch * 0.22f),
                size = Size(w * 0.16f, ch * 0.34f)
            )
            drawOval(
                color = dark,
                topLeft = Offset(cx + w * 0.18f, cy - ch * 0.22f),
                size = Size(w * 0.16f, ch * 0.34f)
            )
        }
        "owl" -> {
            // A pair of discs, which is most of what makes an owl read as one.
            drawCircle(dark, w * 0.15f, Offset(cx - w * 0.15f, cy - ch * 0.12f))
            drawCircle(dark, w * 0.15f, Offset(cx + w * 0.15f, cy - ch * 0.12f))
        }
    }
}

/** Canvas height; named distinctly because draw calls take an `h` parameter. */
private fun DrawScope.canvasH(): Float = size.height

private fun DrawScope.drawIdle(
    cx: Float, cy: Float, w: Float, h: Float,
    body: Color, shade: Color, light: Color,
    phase: Float, blinking: Boolean, species: String
) {
    // A gentle breath rather than a static blob.
    val breathe = 1f + 0.03f * sin(phase * 2f * Math.PI.toFloat())
    drawCircle(shade, w * 0.40f * breathe, Offset(cx, cy + h * 0.03f))
    drawCircle(body, w * 0.38f * breathe, Offset(cx, cy))
    drawCircle(light, w * 0.22f * breathe, Offset(cx - w * 0.08f, cy - h * 0.08f))
    drawSpeciesMark(cx, cy, w, h, species, shade)
    drawEyes(cx, cy - h * 0.02f, w * 0.15f, w * 0.062f, blinking)
}

private fun DrawScope.drawWalking(
    cx: Float, cy: Float, w: Float, h: Float,
    body: Color, shade: Color, light: Color, phase: Float, species: String
) {
    val bob = sin(phase * 2f * Math.PI.toFloat()) * h * 0.02f
    translate(0f, bob) {
        drawCircle(shade, w * 0.38f, Offset(cx, cy + h * 0.02f))
        drawCircle(body, w * 0.36f, Offset(cx, cy))
        drawCircle(light, w * 0.2f, Offset(cx - w * 0.07f, cy - h * 0.08f))
        drawSpeciesMark(cx, cy, w, h, species, shade)
        drawEyes(cx, cy - h * 0.02f, w * 0.14f, w * 0.058f, false)
        // Two feet, alternating.
        val stride = sin(phase * 2f * Math.PI.toFloat()) * w * 0.07f
        drawCircle(shade, w * 0.055f, Offset(cx - w * 0.14f + stride, cy + h * 0.32f))
        drawCircle(shade, w * 0.055f, Offset(cx + w * 0.14f - stride, cy + h * 0.32f))
    }
}

private fun DrawScope.drawFlying(
    cx: Float, cy: Float, w: Float, h: Float,
    body: Color, shade: Color, light: Color, phase: Float
) {
    val flap = sin(phase * 2f * Math.PI.toFloat())
    val drift = sin(phase * Math.PI.toFloat()) * h * 0.05f
    translate(0f, drift) {
        drawCircle(shade, w * 0.30f, Offset(cx, cy))
        drawCircle(body, w * 0.28f, Offset(cx, cy))
        // Wings sweep through a full beat.
        for (side in intArrayOf(-1, 1)) {
            rotate(degrees = side * (18f + flap * 26f), pivot = Offset(cx + side * w * 0.18f, cy)) {
                drawOval(
                    color = light,
                    topLeft = Offset(cx + side * w * 0.16f - w * 0.16f, cy - h * 0.1f),
                    size = Size(w * 0.32f, h * 0.13f)
                )
            }
        }
        drawEyes(cx, cy, w * 0.11f, w * 0.05f, false)
    }
}

private fun DrawScope.drawCelebrating(
    cx: Float, cy: Float, w: Float, h: Float,
    body: Color, shade: Color, light: Color, phase: Float
) {
    val hop = -abs(sin(phase * 2f * Math.PI.toFloat())) * h * 0.16f
    translate(0f, hop) {
        drawCircle(shade, w * 0.36f, Offset(cx, cy + h * 0.04f))
        drawCircle(body, w * 0.34f, Offset(cx, cy))
        drawCircle(light, w * 0.19f, Offset(cx - w * 0.08f, cy - h * 0.09f))
        // Happy closed eyes, as arcs.
        for (side in intArrayOf(-1, 1)) {
            val ex = cx + side * w * 0.14f
            drawArc(
                color = Color(0xFF2B2B2B),
                startAngle = 200f,
                sweepAngle = 140f,
                useCenter = false,
                topLeft = Offset(ex - w * 0.07f, cy - h * 0.09f),
                size = Size(w * 0.14f, h * 0.11f),
                style = Stroke(width = w * 0.022f)
            )
        }
    }
    // Radiating marks, so a celebration reads at a glance.
    val rays = 8
    for (i in 0 until rays) {
        val angle = (i / rays.toFloat()) * 2f * Math.PI.toFloat() + phase * 2f
        val inner = w * 0.44f
        val outer = w * 0.50f
        drawLine(
            color = light,
            start = Offset(cx + cos(angle) * inner, cy + sin(angle) * inner),
            end = Offset(cx + cos(angle) * outer, cy + sin(angle) * outer),
            strokeWidth = w * 0.018f
        )
    }
}

private fun DrawScope.drawSleeping(
    cx: Float, cy: Float, w: Float, h: Float,
    body: Color, shade: Color, light: Color
) {
    drawCircle(shade, w * 0.38f, Offset(cx, cy + h * 0.05f))
    drawCircle(body, w * 0.36f, Offset(cx, cy))
    drawCircle(light, w * 0.2f, Offset(cx - w * 0.07f, cy - h * 0.07f))
    // Closed eyes, and a slow breath, so sleep looks like sleep.
    for (side in intArrayOf(-1, 1)) {
        val ex = cx + side * w * 0.14f
        drawLine(
            color = Color(0xFF2B2B2B),
            start = Offset(ex - w * 0.06f, cy),
            end = Offset(ex + w * 0.06f, cy),
            strokeWidth = w * 0.02f
        )
    }
    // A drifting z, which is the conventional signal.
    val z = Path().apply {
        moveTo(cx + w * 0.24f, cy - h * 0.24f)
        lineTo(cx + w * 0.34f, cy - h * 0.24f)
        lineTo(cx + w * 0.24f, cy - h * 0.32f)
        lineTo(cx + w * 0.34f, cy - h * 0.32f)
    }
    drawPath(z, light, style = Stroke(width = w * 0.022f))
}

private fun cos(a: Float): Float = kotlin.math.cos(a)
private fun sin(a: Float): Float = kotlin.math.sin(a)