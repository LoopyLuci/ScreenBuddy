package com.screenbuddy.android.data.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The creature catalogue.
 *
 * CreatureData used to generate a random UUID for every entry, while the rest of
 * the app referenced ids like "companion-bird-01". Nothing matched, so every
 * species and colour lookup fell back to a default and every creature looked
 * identical. These pin the ids down so that cannot regress silently.
 */
class CreatureCatalogueTest {

    @Test
    fun `ids are stable and human readable`() {
        val ids = CreatureDefaults.creatures.map { it.id }
        assertTrue("the catalogue must not be empty", ids.isNotEmpty())
        ids.forEach { id ->
            assertTrue(
                "'$id' should be readable, not a generated UUID",
                id.matches(Regex("[a-z0-9]+(-[a-z0-9]+)+"))
            )
        }
    }

    @Test
    fun `ids are unique`() {
        val ids = CreatureDefaults.creatures.map { it.id }
        assertEquals(
            "duplicate ids would make a lookup ambiguous",
            ids.size, ids.toSet().size
        )
    }

    @Test
    fun `the ids the app refers to exist in the catalogue`() {
        // These are the ids the engine, the boot receiver, the control surface
        // and the agent presets all use. Any one missing means a silent fallback.
        val known = CreatureDefaults.creatures.map { it.id }.toSet()
        for (id in listOf(
            "companion-bird-01",
            "robo-cat-01",
            "pixel-wizard-01",
            "cosmic-jellyfish-01",
            "slime-king-01",
            "companion-dog-01",
            "ghost-01"
        )) {
            assertTrue(
                "'$id' is referenced by the app but is not in the catalogue",
                id in known
            )
        }
    }

    @Test
    fun `every creature has a species and a parseable colour`() {
        for (creature in CreatureDefaults.creatures) {
            assertTrue("${creature.name} has no species", creature.species.isNotBlank())
            assertTrue(
                "${creature.name} has no colour",
                creature.colorHex.matches(Regex("#[0-9A-Fa-f]{6}"))
            )
        }
    }

    @Test
    fun `species are distinct enough to draw differently`() {
        // A catalogue of identical species would make the drawing code
        // unreachable, which is the defect this test exists to prevent.
        val species = CreatureDefaults.creatures.map { it.species.lowercase() }.toSet()
        assertTrue(
            "expected several species, found ${species.size}",
            species.size >= 3
        )
    }

    @Test
    fun `a creature can be looked up by id`() {
        val bird = CreatureDefaults.creatures.firstOrNull { it.id == "companion-bird-01" }
        assertNotNull("the bird should be findable by id", bird)
        // Species is what drives the drawing; the name is a display label.
        assertEquals("Owl", bird!!.species)
        assertTrue("a species is needed for the drawing to differ", bird.species.isNotBlank())
    }
}