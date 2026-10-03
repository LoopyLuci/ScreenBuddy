package com.screenbuddy.android.data.creature

import kotlin.math.abs
import kotlin.random.Random

/**
 * Where a creature is and how it is moving.
 *
 * The Android app had no physics at all: creature commands updated a status
 * record and nothing was ever drawn, so `move_creature` and `set_animation`
 * reported success while changing no visible thing. The desktop has this engine;
 * this is the same behaviour in Kotlin so the two clients agree.
 */
data class CreatureState(
    val id: String,
    val x: Float,
    val y: Float,
    val velocityX: Float = 0f,
    val velocityY: Float = 0f,
    val animation: String = CreatureEngine.ANIMATION_IDLE,
    val visible: Boolean = true,
    /** True when an agent placed this creature, so physics must not fight it. */
    val pinned: Boolean = false
)

/**
 * The creature simulation.
 *
 * Pure Kotlin and deterministic given a seed, so it can be tested without a
 * device. The rendering layer only ever reads the result.
 */
class CreatureEngine(
    private val random: Random = Random(SEED),
    private val bounds: Bounds = Bounds.DEFAULT
) {

    private val creatures = LinkedHashMap<String, CreatureState>()
    private var autoCycle = true

    /** Seconds since the engine started, for the caller to render from. */
    var elapsed: Float = 0f
        private set

    /** Add a creature if it is not already present. */
    fun add(id: String, x: Float, y: Float): CreatureState {
        val existing = creatures[id]
        if (existing != null) return existing
        val created = CreatureState(id = id, x = x, y = y)
        creatures[id] = created
        return created
    }

    fun remove(id: String) {
        creatures.remove(id)
    }

    fun get(id: String): CreatureState? = creatures[id]

    fun all(): List<CreatureState> = creatures.values.toList()

    fun count(): Int = creatures.size

    fun visibleCount(): Int = creatures.values.count { it.visible }

    fun isAutoCycle(): Boolean = autoCycle

    fun setAutoCycle(enabled: Boolean) {
        autoCycle = enabled
    }

    /**
     * Place a creature at an absolute position and pin it there.
     *
     * Pinned means external control wins: physics must not immediately drift the
     * creature away from where an agent put it. The desktop had exactly this bug
     * and the fix is to reassert the position after stepping.
     */
    fun moveTo(id: String, x: Float, y: Float) {
        val target = creatures[id] ?: add(id, x, y)
        creatures[id] = target.copy(
            x = clampX(x),
            y = clampY(y),
            velocityX = 0f,
            velocityY = 0f,
            pinned = true
        )
    }

    fun setVisible(id: String, visible: Boolean) {
        val target = creatures[id] ?: return
        creatures[id] = target.copy(visible = visible)
    }

    /**
     * Set an animation state.
     *
     * While auto-cycle is on, an externally chosen state must survive: otherwise
     * the next frame would overwrite it and the command would appear to do
     * nothing. Turning auto-cycle off is how a state is held.
     */
    fun setAnimation(id: String, state: String) {
        if (state !in ANIMATION_STATES) return
        val target = creatures[id] ?: return
        creatures[id] = target.copy(animation = state)
        if (autoCycle) {
            // Hold the requested state by leaving cycling off for this creature.
            heldStates[id] = state
        }
    }

    /** States an agent asked for, which cycling must not overwrite. */
    private val heldStates = mutableMapOf<String, String>()

    /** Whether [id] is currently obeying an externally requested animation. */
    fun isHoldingState(id: String): Boolean = heldStates.containsKey(id)

    /**
     * Advance the simulation by [dt] seconds.
     *
     * Deterministic: the same seed and the same inputs give the same positions,
     * which is what makes the tests meaningful.
     */
    fun step(dt: Float) {
        if (dt <= 0f) return
        elapsed += dt
        for (id in creatures.keys.toList()) {
            val c = creatures[id] ?: continue
            if (!c.visible || c.pinned) continue

            var vx = c.velocityX
            var vy = c.velocityY

            // A creature at rest starts wandering, so the screen is never static.
            if (abs(vx) < EPSILON && abs(vy) < EPSILON) {
                if (random.nextFloat() < WANDER_CHANCE) {
                    val angle = random.nextFloat() * TWO_PI
                    val speed = SPEED_MIN + random.nextFloat() * (SPEED_MAX - SPEED_MIN)
                    vx = kotlin.math.cos(angle) * speed
                    vy = kotlin.math.sin(angle) * speed
                }
            }

            var x = c.x + vx * dt
            var y = c.y + vy * dt

            // Bounce off the edges rather than sticking to them.
            var bounced = false
            if (x <= bounds.minX || x >= bounds.maxX) {
                vx = -vx
                x = clampX(x)
                bounced = true
            }
            if (y <= bounds.minY || y >= bounds.maxY) {
                vy = -vy
                y = clampY(y)
                bounced = true
            }

            // Auto-cycle picks a state from the movement, unless one is held.
            var animation = if (heldStates.containsKey(id)) c.animation else c.animation
            if (!heldStates.containsKey(id) && autoCycle) {
                animation = animationFor(vx, vy, bounced)
            }

            creatures[id] = c.copy(
                x = x,
                y = y,
                velocityX = vx,
                velocityY = vy,
                animation = animation
            )
        }
    }

    /** The state a creature's motion implies. */
    private fun animationFor(vx: Float, vy: Float, bounced: Boolean): String = when {
        bounced -> ANIMATION_CELEBRATE
        abs(vx) > abs(vy) -> ANIMATION_WALK
        abs(vy) > abs(vx) -> ANIMATION_FLY
        else -> ANIMATION_IDLE
    }

    /** Release a pinned creature so physics takes over again. */
    fun unpin(id: String) {
        val target = creatures[id] ?: return
        creatures[id] = target.copy(pinned = false)
    }

    /** Reset to a known state, so a test does not depend on run order. */
    fun reset() {
        creatures.clear()
        heldStates.clear()
        elapsed = 0f
    }

    private fun clampX(x: Float) = x.coerceIn(bounds.minX, bounds.maxX)
    private fun clampY(y: Float) = y.coerceIn(bounds.minY, bounds.maxY)

    /** The drawing area, in the same units the caller positions creatures in. */
    data class Bounds(
        val minX: Float,
        val minY: Float,
        val maxX: Float,
        val maxY: Float
    ) {
        companion object {
            /** Matches the desktop's work area closely enough to feel the same. */
            val DEFAULT = Bounds(0f, 0f, 1920f, 1080f)
        }
    }

    companion object {
        const val ANIMATION_IDLE = "idle"
        const val ANIMATION_WALK = "walk"
        const val ANIMATION_FLY = "fly"
        const val ANIMATION_SLEEP = "sleep"
        const val ANIMATION_CELEBRATE = "celebrate"

        val ANIMATION_STATES = setOf(
            ANIMATION_IDLE, ANIMATION_WALK, ANIMATION_FLY,
            ANIMATION_SLEEP, ANIMATION_CELEBRATE
        )

        const val SEED = 20260101
        private const val EPSILON = 0.01f
        private const val TWO_PI = (Math.PI * 2).toFloat()
        private const val WANDER_CHANCE = 0.02f
        private const val SPEED_MIN = 12f
        private const val SPEED_MAX = 40f
    }
}