package com.screenbuddy.android.data.agent

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.Rule
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * Agent profiles and the runtime that must consume them.
 *
 * The point of these tests is the same defect the desktop audit found: a profile
 * can be saved correctly and still change nothing. So the runtime assertions
 * check that activating a profile actually alters what a request would use.
 */
class AgentProfileTest {

    @get:Rule
    val folder = TemporaryFolder()

    private lateinit var store: AgentProfileStore
    private lateinit var dir: File

    @Before
    fun setUp() {
        dir = folder.newFolder("agents")
        store = AgentProfileStore(dir)
    }

    @Test
    fun `a fresh install offers the built-in agents`() {
        val profiles = store.load()
        assertTrue(profiles.isNotEmpty())
        assertTrue(
            "the presets should be present",
            profiles.any { it.id == "assistant" }
        )
        assertTrue(
            "every preset is marked built-in",
            profiles.filter { it.builtin }.isNotEmpty()
        )
    }

    @Test
    fun `a saved profile survives a reload`() {
        val saved = store.save(
            AgentProfile(id = "", name = "Night Owl", persona = Persona.Wry)
        )
        assertEquals("night-owl", saved.id)
        val reloaded = store.get("night-owl")
        assertNotNull("the profile should have been written", reloaded)
        assertEquals(Persona.Wry, reloaded!!.persona)
        assertEquals("Night Owl", reloaded.name)
    }

    @Test
    fun `persona guidance reaches the system prompt`() {
        // This is the whole point: a persona that is stored but never used looks
        // identical to one that works.
        val profile = AgentProfile(id = "x", name = "Wry One", persona = Persona.Wry)
        assertTrue(profile.systemPrompt().contains(Persona.Wry.guidance))
    }

    @Test
    fun `free-text instructions follow the persona`() {
        val profile = AgentProfile(
            id = "x",
            name = "Helper",
            persona = Persona.Terse,
            systemPrompt = "Always answer in French."
        )
        val prompt = profile.systemPrompt()
        assertTrue(prompt.indexOf(Persona.Terse.guidance) < prompt.indexOf("Always answer in French."))
    }

    @Test
    fun `activating a profile changes what the runtime reports`() {
        val runtime = AgentRuntime(store)
        val target = store.save(AgentProfile(id = "", name = "Budgeter", maxIterations = 3))
        assertTrue(runtime.activate(target.id))
        assertEquals(target.id, runtime.active().id)
        assertEquals(3, runtime.maxIterations())
    }

    @Test
    fun `the active profile survives a new runtime`() {
        val target = store.save(AgentProfile(id = "", name = "Persisted"))
        AgentRuntime(store).activate(target.id)
        // A new runtime models a restart: the choice must not be lost.
        val afterRestart = AgentRuntime(AgentProfileStore(dir))
        assertEquals(target.id, afterRestart.active().id)
    }

    @Test
    fun `an unknown agent is rejected rather than activated`() {
        val runtime = AgentRuntime(store)
        val before = runtime.active().id
        assertFalse(runtime.activate("no-such-agent"))
        assertEquals(before, runtime.active().id)
    }

    @Test
    fun `tools off means no tools advertised`() {
        val target = store.save(AgentProfile(id = "", name = "Plain", toolsEnabled = false))
        val runtime = AgentRuntime(store)
        runtime.activate(target.id)
        assertFalse(runtime.toolsEnabled())
    }

    @Test
    fun `a profile that did not choose a temperature does not override the app`() {
        // 0.7 means "unset": overriding it would silently defeat the user's own
        // slider, which is the bug this guards.
        val runtime = AgentRuntime(store)
        assertNull(runtime.temperatureOverride())

        val target = store.save(AgentProfile(id = "", name = "Cold", temperature = 0.1f))
        runtime.activate(target.id)
        assertEquals(0.1f, runtime.temperatureOverride()!!, 1e-6f)
    }

    @Test
    fun `an empty model means automatic rather than an unusable pin`() {
        val runtime = AgentRuntime(store)
        assertNull(runtime.pinnedModel())
        val target = store.save(AgentProfile(id = "", name = "Pinned", model = "gpt-4o"))
        runtime.activate(target.id)
        assertEquals("gpt-4o", runtime.pinnedModel())
    }

    @Test
    fun `built-in agents cannot be deleted`() {
        assertFalse(store.delete("assistant"))
        assertNotNull("the preset must still be there", store.get("assistant"))
    }

    @Test
    fun `duplicating leaves the original intact`() {
        val copy = store.duplicate("assistant")
        assertNotNull(copy)
        assertFalse("the copy is not built in", copy!!.builtin)
        assertEquals("Assistant copy", copy.name)
        assertNotNull("the original must survive", store.get("assistant"))
        assertEquals(2, store.load().count { it.id.startsWith("assistant") })
    }

    @Test
    fun `editing a built-in keeps it built in`() {
        val edited = store.save(store.get("assistant")!!.copy(persona = Persona.Terse))
        assertTrue("editing a preset must not turn it into a normal agent", edited.builtin)
        assertEquals(Persona.Terse, store.get("assistant")!!.persona)
    }

    @Test
    fun `a corrupt file yields the presets rather than no agents`() {
        File(dir, AgentProfileStore.FILE_NAME).writeText("{not json")
        val recovered = AgentProfileStore(dir).load()
        assertTrue("a bad file must not leave the user with nothing", recovered.isNotEmpty())
    }

    @Test
    fun `out-of-range values are repaired rather than stored`() {
        val saved = store.save(
            AgentProfile(
                id = "", name = "Wild",
                maxIterations = 9999, timeoutSeconds = 1, temperature = 9f
            )
        )
        assertTrue(saved.maxIterations <= 50)
        assertTrue(saved.timeoutSeconds >= 5)
        assertTrue(saved.temperature <= 2f)
    }

    @Test
    fun `a blank name still yields a usable profile`() {
        val saved = store.save(AgentProfile(id = "", name = "   "))
        assertTrue("the store must assign a usable id", saved.id.isNotBlank())
        assertTrue(saved.name.isNotBlank())
    }

    @Test
    fun `describing the active agent reports what it is configured with`() {
        val target = store.save(
            AgentProfile(id = "", name = "Reported", persona = Persona.Explainer)
        )
        val runtime = AgentRuntime(store)
        runtime.activate(target.id)
        val described = runtime.describe()
        assertEquals("Reported", described["name"])
        assertEquals("explainer", described["persona"])
    }
}