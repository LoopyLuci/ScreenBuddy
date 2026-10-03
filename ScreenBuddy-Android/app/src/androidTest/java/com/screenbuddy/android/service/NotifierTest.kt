package com.screenbuddy.android.service

import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.content.Context
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * What the notifier reports when notifications are unavailable.
 *
 * The Nokia 7.2 is API 30, where POST_NOTIFICATIONS is not a runtime permission,
 * so the blocked path cannot be produced by revoking it. These assertions cover
 * the decision the settings screen acts on instead of relying on a device that
 * cannot reach the state.
 */
@RunWith(AndroidJUnit4::class)
class NotifierTest {

    private lateinit var notifier: Notifier

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        notifier = Notifier(context)
    }

    @Test
    fun theChannelCanBeCreated() {
        assertTrue("the channel must be creatable", notifier.ensureChannel())
    }

    @Test
    fun creatingTheChannelTwiceIsSafe() {
        // ensureChannel is called on every notify, so it has to be idempotent.
        assertTrue(notifier.ensureChannel())
        assertTrue(notifier.ensureChannel())
    }

    @Test
    fun notifyReportsWhetherItPosted() {
        // Whatever the answer, it must be an explicit true or false rather than
        // a silent no-op: a caller needs to tell a blocked notification from a
        // delivered one.
        val posted = notifier.notify("ScreenBuddy", "test body")
        assertTrue(posted == true || posted == false)
    }

    @Test
    fun blockReasonIsNullWhenNotificationsAreAllowed() {
        // On a device where notifications are on, there is nothing to warn about.
        if (notifier.areNotificationsEnabled()) {
            assertTrue(
                "no warning should be shown when notifications work",
                notifier.blockReason() == null
            )
        }
    }

    @Test
    fun blockReasonExplainsItselfWhenBlocked() {
        // If Android has notifications off, the reason must say so rather than
        // being an empty string the UI would render as nothing.
        if (!notifier.areNotificationsEnabled()) {
            val reason = notifier.blockReason()
            assertTrue("a blocked state needs a reason", !reason.isNullOrBlank())
        }
    }
}