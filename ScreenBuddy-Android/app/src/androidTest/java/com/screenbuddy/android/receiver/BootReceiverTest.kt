package com.screenbuddy.android.receiver

import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.screenbuddy.android.data.control.ControlBus
import com.screenbuddy.android.data.repository.SettingsRepository
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The boot receiver, invoked directly.
 *
 * A simulated `am broadcast` cannot stand in for a real boot: on Android 11 the
 * protected action is not delivered to a backgrounded app, and timing makes the
 * result unreliable. Calling the receiver directly tests the decision it makes,
 * which is the part that can be wrong.
 */
@RunWith(AndroidJUnit4::class)
class BootReceiverTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ControlBus.creatures.reset()
        runBlocking { SettingsRepository(context).setAutoStart(false) }
    }

    /** Let the receiver's worker thread finish before asserting. */
    private fun settle() {
        // The receiver does its work on a thread and calls goAsync().finish().
        Thread.sleep(1200)
    }

    @Test
    fun withAutoStartOff_theBootBroadcastIsIgnored() {
        BootReceiver().onReceive(
            context,
            Intent(Intent.ACTION_BOOT_COMPLETED)
        )
        settle()

        assertEquals(
            "the creature should not have been restored",
            0, ControlBus.creatures.count()
        )
    }

    @Test
    fun withAutoStartOn_theCreatureIsRestored() {
        runBlocking { SettingsRepository(context).setAutoStart(true) }
        ControlBus.creatures.reset()

        BootReceiver().onReceive(
            context,
            Intent(Intent.ACTION_BOOT_COMPLETED)
        )
        settle()

        assertTrue(
            "a boot with auto start on should restore a creature",
            ControlBus.creatures.count() > 0
        )
    }

    @Test
    fun withAutoStartOn_anExistingCreatureIsNotDuplicated() {
        runBlocking { SettingsRepository(context).setAutoStart(true) }
        ControlBus.creatures.reset()
        ControlBus.creatures.add("companion-bird-01", 10f, 20f)

        BootReceiver().onReceive(
            context,
            Intent(Intent.ACTION_BOOT_COMPLETED)
        )
        settle()

        assertEquals(
            "the boot must not add a second copy",
            1, ControlBus.creatures.count()
        )
    }

    @Test
    fun anUnrelatedBroadcastIsIgnored() {
        runBlocking { SettingsRepository(context).setAutoStart(true) }
        ControlBus.creatures.reset()

        BootReceiver().onReceive(context, Intent("com.example.SOMETHING_ELSE"))
        settle()

        assertEquals(0, ControlBus.creatures.count())
    }

    @Test
    fun aPackageReplaceAlsoRestores() {
        runBlocking { SettingsRepository(context).setAutoStart(true) }
        ControlBus.creatures.reset()

        BootReceiver().onReceive(
            context,
            Intent(Intent.ACTION_MY_PACKAGE_REPLACED)
        )
        settle()

        assertTrue(
            "an update should restore the creature too",
            ControlBus.creatures.count() > 0
        )
    }
}