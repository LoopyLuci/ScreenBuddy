extends Node
## Export Plugin - Handles exporting creatures from Godot to ScreenBuddy format
##
## Provides the main export pipeline: validating creature data,
## processing animations, and writing the final package.

signal validation_completed(creature_id: String, errors: Array[String])
signal export_progress(creature_id: String, step: String, progress: float)

const CREATURE_SCHEMA_VERSION: String = "1.0.0"

## -------------------------------------------------------------------
## Validation
## -------------------------------------------------------------------

func validate_creature(creature: Dictionary) -> Array[String]:
	var errors: Array[String] = []
	
	# Required fields
	if not creature.has("id"):
		errors.append("Missing required field: id")
	elif not creature.id is String or creature.id.is_empty():
		errors.append("Field 'id' must be a non-empty string")
	
	if not creature.has("name"):
		errors.append("Missing required field: name")
	
	if not creature.has("category"):
		errors.append("Missing required field: category")
	elif not creature.category is String:
		errors.append("Field 'category' must be a string")
	
	if not creature.has("visual"):
		errors.append("Missing required field: visual")
	else:
		errors.append_array(_validate_visual(creature.visual))
	
	if not creature.has("behavior"):
		errors.append("Missing required field: behavior")
	else:
		errors.append_array(_validate_behavior(creature.behavior))
	
	# Optional field validation
	if creature.has("ai"):
		errors.append_array(_validate_ai(creature.ai))
	
	if creature.has("permissions"):
		errors.append_array(_validate_permissions(creature.permissions))
	
	return errors

func _validate_visual(visual: Dictionary) -> Array[String]:
	var errors: Array[String] = []
	
	if not visual.has("animation_format"):
		errors.append("visual: Missing required field: animation_format")
	elif not ["sprite_sheet", "spine", "live2d", "frame"].has(visual.animation_format):
		errors.append("visual: Invalid animation_format. Must be: sprite_sheet, spine, live2d, frame")
	
	if not visual.has("texture"):
		errors.append("visual: Missing required field: texture")
	
	if not visual.has("default_scale"):
		errors.append("visual: Missing required field: default_scale")
	elif not visual.default_scale is float:
		errors.append("visual: default_scale must be a number")
	
	if not visual.has("idle_animation"):
		errors.append("visual: Missing required field: idle_animation")
	
	return errors

func _validate_behavior(behavior: Dictionary) -> Array[String]:
	var errors: Array[String] = []
	
	if not behavior.has("default_state"):
		errors.append("behavior: Missing required field: default_state")
	
	if behavior.has("personality"):
		var personality: Dictionary = behavior.personality
		var valid_traits: PackedStringArray = [
			"curiosity", "sass", "patience", "energy", "playfulness",
			"intelligence", "bravery", "kindness", "mischief"
		]
		for trait in personality:
			if not valid_traits.has(trait):
				errors.append("behavior: Unknown personality trait: " + trait)
			elif not personality[trait] is float:
				errors.append("behavior: Personality trait '" + trait + "' must be a number (0.0-1.0)")
			elif personality[trait] < 0.0 or personality[trait] > 1.0:
				errors.append("behavior: Personality trait '" + trait + "' must be between 0.0 and 1.0")
	
	return errors

func _validate_ai(ai: Dictionary) -> Array[String]:
	var errors: Array[String] = []
	
	if ai.has("system_prompt") and not ai.system_prompt is String:
		errors.append("ai: system_prompt must be a string")
	
	if ai.has("specializations"):
		if not ai.specializations is Array:
			errors.append("ai: specializations must be an array")
	
	return errors

func _validate_permissions(permissions: Dictionary) -> Array[String]:
	var errors: Array[String] = []
	
	var valid_keys: PackedStringArray = [
		"can_read_screen", "can_read_clipboard", "can_access_files",
		"can_use_tools", "can_send_notifications", "can_access_microphone",
		"can_access_camera", "can_modify_files"
	]
	
	for key in permissions:
		if not valid_keys.has(key):
			errors.append("permissions: Unknown key: " + key)
		elif not permissions[key] is bool:
			errors.append("permissions: '" + key + "' must be a boolean")
	
	return errors

## -------------------------------------------------------------------
## Export
## -------------------------------------------------------------------

func export_creature(creature_id: String, output_dir: String, options: Dictionary = {}) -> Error:
	emit_signal("export_progress", creature_id, "Starting export", 0.0)
	
	# Get creature data
	var creature: Dictionary = CreatureDB.get_creature(creature_id)
	if creature.is_empty():
		push_error("Creature not found: " + creature_id)
		return ERR_DOES_NOT_EXIST
	
	# Validate
	emit_signal("export_progress", creature_id, "Validating creature data", 0.1)
	var errors: Array[String] = validate_creature(creature)
	if not errors.is_empty():
		emit_signal("validation_completed", creature_id, errors)
		for error in errors:
			push_error("Validation error: " + error)
		return ERR_INVALID_DATA
	
	emit_signal("validation_completed", creature_id, [])
	
	# Create output directory
	emit_signal("export_progress", creature_id, "Creating output directory", 0.2)
	var dir := DirAccess.open("res://")
	var abs_output: String = ProjectSettings.globalize_path(output_dir)
	dir.make_dir_recursive(abs_output)
	
	# Process and export
	emit_signal("export_progress", creature_id, "Processing visual assets", 0.3)
	var processed_creature: Dictionary = _process_creature(creature)
	
	emit_signal("export_progress", creature_id, "Exporting textures", 0.5)
	_export_textures(processed_creature, output_dir)
	
	emit_signal("export_progress", creature_id, "Exporting animations", 0.7)
	_export_animations(processed_creature, output_dir)
	
	emit_signal("export_progress", creature_id, "Writing definition file", 0.9)
	_write_definition_file(processed_creature, output_dir)
	
	emit_signal("export_progress", creature_id, "Export complete", 1.0)
	return OK

func _process_creature(creature: Dictionary) -> Dictionary:
	# Deep copy and add metadata
	var processed: Dictionary = creature.duplicate(true)
	processed["_screenbuddy"] = {
		"schema_version": CREATURE_SCHEMA_VERSION,
		"exported_at": Time.get_datetime_string_from_system(),
		"godot_version": Engine.get_version_info().hash
	}
	return processed

func _export_textures(creature: Dictionary, output_dir: String):
	if not creature.has("visual"):
		return
	
	var visual: Dictionary = creature.visual
	var dir := DirAccess.open("res://")
	dir.make_dir_recursive(ProjectSettings.globalize_path(output_dir) + "/textures")
	
	# Export main texture
	if visual.has("texture"):
		var src: String = visual.texture
		var src_abs: String = ProjectSettings.globalize_path(src)
		if FileAccess.file_exists(src_abs):
			var dest: String = output_dir + "/textures/" + creature.id + ".png"
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest))
	
	# Export atlas if present
	if visual.has("atlas"):
		var src: String = visual.atlas
		var src_abs: String = ProjectSettings.globalize_path(src)
		if FileAccess.file_exists(src_abs):
			var dest: String = output_dir + "/textures/" + creature.id + "_atlas.json"
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest))

func _export_animations(creature: Dictionary, output_dir: String):
	if not creature.has("visual"):
		return
	
	var visual: Dictionary = creature.visual
	var dir := DirAccess.open("res://")
	dir.make_dir_recursive(ProjectSettings.globalize_path(output_dir) + "/animations")
	
	# Animation data is stored in the definition file
	# For spine/live2d, export the skeleton data
	if visual.has("skeleton"):
		var src: String = visual.skeleton
		var src_abs: String = ProjectSettings.globalize_path(src)
		if FileAccess.file_exists(src_abs):
			var dest: String = output_dir + "/animations/" + creature.id + "_skeleton.json"
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest))

func _write_definition_file(creature: Dictionary, output_dir: String):
	var path: String = output_dir + "/" + creature.id + ".json"
	var json: String = JSON.stringify(creature, "\t")
	var file := FileAccess.open(ProjectSettings.globalize_path(path), FileAccess.WRITE)
	if file:
		file.store_string(json)
		file.close()

## -------------------------------------------------------------------
## Batch Export
## -------------------------------------------------------------------

func export_all_creatures(output_dir: String) -> Dictionary:
	var results: Dictionary = {
		"success": [],
		"failed": []
	}
	
	var creatures: Dictionary = CreatureDB.get_all_creatures()
	for creature_id in creatures:
		var err: Error = export_creature(creature_id, output_dir + "/" + creature_id)
		if err == OK:
			results.success.append(creature_id)
		else:
			results.failed.append(creature_id)
	
	return results
