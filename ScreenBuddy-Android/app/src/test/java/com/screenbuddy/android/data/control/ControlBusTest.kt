package com.screenbuddy.android.data.control

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Control surface tests.
 *
 * Command names and response shapes must match the desktop IPC surface, so
 * these assert the vocabulary and the failure modes an agent depends on.
 */
class ControlBusTest {

    @Before
    fun setUp() {
        ControlBus.reset()
    }

    @After
    fun tearDown() {
        ControlBus.reset()
    }

    private fun install() {
        // The production installer needs a Context; the handler logic under test
        // does not, so the same commands are registered against the bare bus.
        ControlBus.register("ping") { ControlResponse.ok(mapOf("status" to "pong")) }
        ControlBus.register("get_status") {
            val s = ControlBus.status.value
            ControlResponse.ok(
                mapOf(
                    "visible" to s.visible,
                    "animation_state" to s.animationState,
                    "auto_cycle" to s.autoCycle,
                )
            )
        }
        ControlBus.register("set_animation") { req ->
            val state = req.state ?: return@register ControlResponse.error("missing state")
            if (state !in setOf("idle", "walk", "fly", "sleep", "celebrate")) {
                return@register ControlResponse.error("bad state")
            }
            ControlBus.updateStatus { it.copy(animationState = state, autoCycle = false) }
            ControlResponse.queued("animation")
        }
        ControlBus.register("set_creature_visible") { req ->
            val visible = req.visible ?: return@register ControlResponse.error("missing visible")
            ControlBus.updateStatus { it.copy(visible = visible) }
            ControlResponse.queued("visibility")
        }
        ControlBus.register("set_setting") { req ->
            val key = req.key ?: return@register ControlResponse.error("missing key")
            val value = req.value ?: return@register ControlResponse.error("missing value")
            ControlBus.updateStatus { it.copy(settings = it.settings + (key to value)) }
            ControlResponse.queued("setting")
        }
        ControlBus.register("get_setting") { req ->
            val settings = ControlBus.status.value.settings
            val key = req.key
            if (key.isNullOrBlank()) {
                ControlResponse.ok(mapOf("settings" to settings))
            } else {
                val v = settings[key] ?: return@register ControlResponse.error("unknown", 404)
                ControlResponse.ok(mapOf("key" to key, "value" to v))
            }
        }
    }

    @Test
    fun pingSucceeds() {
        install()
        val r = ControlBus.submit(ControlRequest(cmd = "ping"))
        assertTrue(r.ok)
        assertEquals("pong", r.data?.get("status"))
    }

    @Test
    fun unknownCommandIsRejectedNotCrashed() {
        install()
        val r = ControlBus.submit(ControlRequest(cmd = "definitely_not_a_command"))
        assertFalse(r.ok)
        assertEquals(404, r.data?.get("code"))
    }

    @Test
    fun handlerThrowingIsReportedAsAnError() {
        ControlBus.register("boom") { throw IllegalStateException("kaboom") }
        val r = ControlBus.submit(ControlRequest(cmd = "boom"))
        assertFalse(r.ok)
        assertEquals(500, r.data?.get("code"))
        assertTrue(r.message?.contains("kaboom") == true)
    }

    @Test
    fun setAnimationChangesStateAndPausesAutoCycle() {
        install()
        ControlBus.submit(ControlRequest(cmd = "set_animation", state = "celebrate"))
        val s = ControlBus.status.value
        assertEquals("celebrate", s.animationState)
        // Holding a state must survive the rotation timer, so auto-cycle pauses.
        assertFalse("auto-cycle should pause when a state is pinned", s.autoCycle)
    }

    @Test
    fun invalidAnimationStateIsRejected() {
        install()
        val r = ControlBus.submit(ControlRequest(cmd = "set_animation", state = "bogus"))
        assertFalse(r.ok)
        assertEquals("idle", ControlBus.status.value.animationState)
    }

    @Test
    fun missingArgumentIsReportedClearly() {
        install()
        val r = ControlBus.submit(ControlRequest(cmd = "set_animation"))
        assertFalse(r.ok)
        assertTrue(r.message!!.contains("state"))
    }

    @Test
    fun visibilityRoundTrips() {
        install()
        ControlBus.submit(ControlRequest(cmd = "set_creature_visible", visible = false))
        assertFalse(ControlBus.status.value.visible)
        val status = ControlBus.submit(ControlRequest(cmd = "get_status"))
        assertEquals(false, status.data?.get("visible"))
    }

    @Test
    fun settingsRoundTrip() {
        install()
        ControlBus.submit(ControlRequest(cmd = "set_setting", key = "volume", value = "0.5"))
        val one = ControlBus.submit(ControlRequest(cmd = "get_setting", key = "volume"))
        assertEquals("0.5", one.data?.get("value"))

        val all = ControlBus.submit(ControlRequest(cmd = "get_setting"))
        assertTrue((all.data?.get("settings") as Map<*, *>).containsKey("volume"))
    }

    @Test
    fun unknownSettingIsNotFound() {
        install()
        val r = ControlBus.submit(ControlRequest(cmd = "get_setting", key = "nope"))
        assertFalse(r.ok)
        assertEquals(404, r.data?.get("code"))
    }

    @Test
    fun commandsOverlapTheDesktopVocabulary() {
        // If the desktop adds or renames a command, this list should follow it.
        val desktopCommands = setOf(
            "ping", "get_status", "list_creature_state", "set_animation",
            "move_creature", "set_creature_visible", "set_auto_cycle",
            "send_chat", "get_chat_history", "get_agent_info", "play_sound",
            "speak", "get_setting", "set_setting", "memory_ingest", "memory_search",
        )
        val android = ControlCommands.SUPPORTED.toSet()
        val missing = desktopCommands - android
        assertTrue("Android is missing desktop commands: $missing", missing.isEmpty())
    }

    @Test
    fun responseEnvelopeMatchesTheDesktopShape() {
        val ok = ControlResponse.ok(mapOf("queued" to true))
        assertEquals("success", ok.status)
        val err = ControlResponse.error("bad", 404)
        assertEquals("error", err.status)
        assertEquals(404, err.data?.get("code"))
    }
}