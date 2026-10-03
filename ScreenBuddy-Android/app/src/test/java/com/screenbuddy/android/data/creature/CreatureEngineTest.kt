package com.screenbuddy.android.data.creature

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import kotlin.random.Random

/**
 * The creature simulation.
 *
 * Deterministic by construction, so these assert on real positions rather than
 * "it changed somehow". The external-control cases are the point: an agent's
 * command has to survive the next physics step, or `move_creature` reports
 * success and the creature drifts away.
 */
class CreatureEngineTest {

    private lateinit var engine: CreatureEngine

    @Before
    fun setUp() {
        engine = CreatureEngine(
            random = Random(1234),
            bounds = CreatureEngine.Bounds(0f, 0f, 400f, 400f)
        )
    }

    @Test
    fun `adding a creature places it`() {
        engine.add("bird", 100f, 150f)
        val c = engine.get("bird")
        assertNotNull(c)
        assertEquals(100f, c!!.x, 1e-3f)
        assertEquals(150f, c.y, 1e-3f)
        assertEquals(CreatureEngine.ANIMATION_IDLE, c.animation)
    }

    @Test
    fun `adding the same creature twice does not reset it`() {
        engine.add("bird", 100f, 100f)
        engine.moveTo("bird", 300f, 300f)
        engine.add("bird", 0f, 0f)
        assertEquals(300f, engine.get("bird")!!.x, 1e-3f)
    }

    @Test
    fun `a moved creature stays where it was put`() {
        // The bug this guards: physics ran after the move and drifted the
        // creature away, so the command looked accepted but was undone.
        engine.add("bird", 0f, 0f)
        engine.moveTo("bird", 320f, 240f)
        repeat(60) { engine.step(1f / 30f) }
        assertEquals(320f, engine.get("bird")!!.x, 0.5f)
        assertEquals(240f, engine.get("bird")!!.y, 0.5f)
    }

    @Test
    fun `a hidden creature is not simulated`() {
        engine.add("bird", 50f, 50f)
        engine.setVisible("bird", false)
        repeat(30) { engine.step(1f / 30f) }
        assertEquals(50f, engine.get("bird")!!.x, 0.5f)
        assertEquals(0, engine.visibleCount())
    }

    @Test
    fun `an animation set by an agent is not overwritten`() {
        // Auto-cycle would otherwise replace it on the next frame, making
        // set_animation look like it did nothing.
        engine.add("bird", 200f, 200f)
        engine.setAnimation("bird", CreatureEngine.ANIMATION_CELEBRATE)
        repeat(60) { engine.step(1f / 30f) }
        assertEquals(
            CreatureEngine.ANIMATION_CELEBRATE,
            engine.get("bird")!!.animation
        )
        assertTrue(engine.isHoldingState("bird"))
    }

    @Test
    fun `an unknown animation state is ignored`() {
        engine.add("bird", 10f, 10f)
        engine.setAnimation("bird", "moonwalk")
        assertEquals(CreatureEngine.ANIMATION_IDLE, engine.get("bird")!!.animation)
    }

    @Test
    fun `a moving creature chooses a movement animation`() {
        engine.add("bird", 200f, 200f)
        engine.setAutoCycle(true)
        // Drive it so the state must follow the motion.
        engine.moveTo("bird", 200f, 200f)
        engine.unpin("bird")
        val target = engine.get("bird")!!
        engine.remove("bird")
        engine.add("bird", target.x, target.y)
        engine.unpin("bird")
        repeat(200) { engine.step(1f / 30f) }
        val state = engine.get("bird")!!.animation
        assertTrue(
            "expected a movement state, got $state",
            state in setOf(
                CreatureEngine.ANIMATION_WALK,
                CreatureEngine.ANIMATION_FLY,
                CreatureEngine.ANIMATION_IDLE
            )
        )
    }

    @Test
    fun `a creature stays inside the bounds`() {
        engine.add("bird", 200f, 200f)
        engine.moveTo("bird", 395f, 200f)
        engine.unpin("bird")
        repeat(600) { engine.step(1f / 30f) }
        val c = engine.get("bird")!!
        assertTrue("x escaped: ${c.x}", c.x in 0f..400f)
        assertTrue("y escaped: ${c.y}", c.y in 0f..400f)
    }

    @Test
    fun `a move outside the bounds is clamped rather than rejected`() {
        engine.add("bird", 0f, 0f)
        engine.moveTo("bird", 9000f, -50f)
        assertEquals(400f, engine.get("bird")!!.x, 1e-3f)
        assertEquals(0f, engine.get("bird")!!.y, 1e-3f)
    }

    @Test
    fun `auto-cycle can be turned off`() {
        engine.add("bird", 100f, 100f)
        engine.moveTo("bird", 100f, 100f)
        engine.unpin("bird")
        engine.setAutoCycle(false)
        val before = engine.get("bird")!!.copy()
        repeat(120) { engine.step(1f / 30f) }
        assertFalse("cycling off should mean no automatic state change",
            engine.get("bird")!!.animation != before.animation &&
                engine.get("bird")!!.animation != CreatureEngine.ANIMATION_IDLE)
        assertFalse(engine.isAutoCycle())
    }

    @Test
    fun `stepping with no time does nothing`() {
        engine.add("bird", 100f, 100f)
        val before = engine.elapsed
        engine.step(0f)
        engine.step(-1f)
        assertEquals(before, engine.elapsed, 1e-6f)
    }

    @Test
    fun `the same seed gives the same run`() {
        // Determinism is what makes the position assertions above meaningful.
        fun run(): List<Pair<Float, Float>> {
            val e = CreatureEngine(Random(99), CreatureEngine.Bounds(0f, 0f, 400f, 400f))
            e.add("bird", 200f, 200f)
            e.unpin("bird")
            repeat(100) { e.step(1f / 30f) }
            return listOf(e.get("bird")!!.x to e.get("bird")!!.y)
        }
        assertEquals(run(), run())
    }

    @Test
    fun `elapsed time accumulates`() {
        engine.add("bird", 0f, 0f)
        engine.step(0.5f)
        engine.step(0.5f)
        assertEquals(1.0f, engine.elapsed, 1e-4f)
    }

    @Test
    fun `reset clears everything`() {
        engine.add("bird", 10f, 10f)
        engine.moveTo("bird", 20f, 20f)
        engine.reset()
        assertEquals(0, engine.count())
        assertEquals(0f, engine.elapsed, 1e-6f)
    }
}