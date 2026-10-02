package com.screenbuddy.android

import android.app.Application
import com.screenbuddy.android.data.local.AppDatabase

class ScreenBuddyApp : Application() {
    val database: AppDatabase by lazy { AppDatabase.getDatabase(this) }
}
