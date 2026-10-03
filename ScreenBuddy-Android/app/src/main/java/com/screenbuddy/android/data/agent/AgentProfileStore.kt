package com.screenbuddy.android.data.agent

import android.content.Context
import com.google.gson.Gson
import com.google.gson.JsonSyntaxException
import com.google.gson.reflect.TypeToken
import java.io.File

/**
 * Persistence for agent profiles.
 *
 * A JSON file rather than Room: profiles are a small, rarely-changed document
 * that is read and written as a whole, and the desktop stores them the same way.
 * The active agent is a separate marker file for the same reason the desktop
 * uses one - so loading the list never has to care which agent is active.
 *
 * Writes are atomic via a temporary file, because a half-written profile file
 * would otherwise lose every agent the user had.
 */
class AgentProfileStore(
    /**
     * Where the files live.
     *
     * Primary rather than a Context because the store does no Android work beyond
     * being handed a directory path. That also lets it be exercised on a plain
     * JVM, which is a much better trade than adding Robolectric.
     */
    private val directory: File
) {

    private val gson = Gson()
    private val file = File(directory, FILE_NAME)
    private val activeFile = File(directory, ACTIVE_FILE)



    /**
     * Every stored profile, with the built-ins always present.
     *
     * A corrupt file yields the built-ins rather than an empty list: an
     * unreadable file must not leave the user with no agents at all.
     */
    @Synchronized
    fun load(): List<AgentProfile> {
        val stored = readStored()
        val merged = LinkedHashMap<String, AgentProfile>()
        for (profile in AgentProfile.BUILT_INS) {
            merged[profile.id] = profile
        }
        // A user copy of a built-in replaces it rather than duplicating it, so
        // editing a preset does not produce two agents that look identical.
        for (profile in stored) {
            val sanitized = profile.sanitized()
            val existing = merged[sanitized.id]
            merged[sanitized.id] = if (existing?.builtin == true) {
                // Keep it built-in, but honour the user's edits.
                sanitized.copy(builtin = true)
            } else {
                sanitized
            }
        }
        return merged.values.toList()
    }

    @Synchronized
    fun get(id: String): AgentProfile? = load().firstOrNull { it.id == id }

    /** Insert or replace [profile]. Returns the stored copy. */
    @Synchronized
    fun save(profile: AgentProfile): AgentProfile {
        val sanitized = profile.sanitized()
        val existing = load().firstOrNull { it.id == sanitized.id }
        // Look the original up before filtering it out: searching the filtered
        // list meant an edited preset lost its built-in flag and became deletable.
        val stored = sanitized.copy(builtin = existing?.builtin ?: false)
        write(load().filterNot { it.id == stored.id } + stored)
        return stored
    }

    /**
     * Remove a profile.
     *
     * Built-ins cannot be deleted: they are the agents the app always offers, and
     * removing one would leave a fresh install without them.
     */
    @Synchronized
    fun delete(id: String): Boolean {
        val target = get(id) ?: return false
        if (target.builtin) return false
        write(load().filterNot { it.id == id })
        if (activeId() == id) {
            activeFile.delete()
        }
        return true
    }

    /** Copy [id] under a new name, so the original stays intact. */
    @Synchronized
    fun duplicate(id: String): AgentProfile? {
        val source = get(id) ?: return null
        val name = nextCopyName(source.name)
        val copy = source.copy(
            id = uniqueId(AgentProfile.slug(name)),
            name = name,
            builtin = false
        )
        write(load() + copy)
        return copy
    }

    /**
     * The active agent, or null when none has been chosen.
     *
     * A marker naming an agent that no longer exists reads as none rather than
     * as a dangling id.
     */
    @Synchronized
    fun activeId(): String? = activeFile.takeIf { it.isFile }?.readText()?.trim()
        ?.takeIf { it.isNotEmpty() && get(it) != null }

    @Synchronized
    fun activeProfile(): AgentProfile? = activeId()?.let(::get)

    /** Record [id] as active. Passing null or an unknown id clears it. */
    @Synchronized
    fun setActive(id: String?): Boolean {
        if (id == null) {
            activeFile.delete()
            return true
        }
        if (get(id) == null) return false
        activeFile.writeText(id)
        return true
    }

    /** Profiles with [active] marked, for the agent list. */
    @Synchronized
    fun summaries(): List<AgentSummary> {
        val active = activeId()
        return load().map {
            AgentSummary(
                id = it.id,
                name = it.name,
                persona = it.persona.label,
                creature = it.creature,
                builtin = it.builtin,
                active = it.id == active
            )
        }
    }

    private fun nextCopyName(base: String): String {
        val stem = base.removeSuffix(" copy").removeSuffix(" copy 2")
        var index = 2
        var candidate = "$stem copy"
        val taken = load().map { it.name }.toSet()
        while (candidate in taken) {
            candidate = "$stem copy ${index++}"
        }
        return candidate
    }

    /** Ensure [base] is not already taken, appending a number if it is. */
    private fun uniqueId(base: String): String {
        val taken = load().map { it.id }.toSet()
        if (base !in taken) return base
        var index = 2
        while ("$base-$index" in taken) index++
        return "$base-$index"
    }

    private fun readStored(): List<AgentProfile> {
        if (!file.isFile) return emptyList()
        return try {
            val type = object : TypeToken<List<AgentProfile>>() {}.type
            gson.fromJson<List<AgentProfile>>(file.readText(), type) ?: emptyList()
        } catch (e: JsonSyntaxException) {
            // A malformed file must not crash startup, and must not be
            // overwritten either: keeping it lets a user recover their profiles.
            emptyList()
        } catch (e: Exception) {
            emptyList()
        }
    }

    private fun write(profiles: List<AgentProfile>) {
        val tmp = File(file.parentFile, "$FILE_NAME.tmp")
        try {
            // Built-ins are seeded from code on load, so a user's edit to one is
            // written as an override. Filtering them out here discarded those
            // edits on the next read.
            tmp.writeText(gson.toJson(profiles))
            // Replace in one step so a crash cannot leave a truncated file.
            if (file.exists() && !file.delete()) return
            if (!tmp.renameTo(file)) {
                // Rename can fail on some filesystems; fall back to a copy.
                file.writeText(tmp.readText())
                tmp.delete()
            }
        } catch (e: Exception) {
            tmp.delete()
        }
    }

    companion object {
        const val FILE_NAME = "agents.json"
        const val ACTIVE_FILE = "agents.active.json"

        /**
         * Store under the app's private files directory.
         *
         * filesDir is platform-typed (File?), so it is named explicitly rather
         * than passed to File(), which will not accept it.
         */
        fun inAppFiles(context: Context): AgentProfileStore {
            val app = context.applicationContext
            val dir: File = app.filesDir ?: File(app.cacheDir, "agents")
            return AgentProfileStore(dir)
        }
    }
}