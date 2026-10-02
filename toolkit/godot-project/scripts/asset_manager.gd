extends Node
## Asset Manager - Handles importing, processing, and exporting assets
##
## Manages textures, sounds, and fonts for creatures. Provides utilities
## for converting Godot resources to ScreenBuddy-compatible formats.

signal asset_imported(path: String, type: String)
signal export_started(creature_id: String)
signal export_completed(creature_id: String, path: String)
signal export_failed(creature_id: String, reason: String)

enum AssetType {
	TEXTURE,
	SOUND,
	FONT,
	SKELETON,
	ANIMATION
}

var _supported_image_extensions: PackedStringArray = [
	"png", "jpg", "jpeg", "webp", "bmp", "tga"
]

var _supported_audio_extensions: PackedStringArray = [
	"wav", "ogg", "mp3"
]

## -------------------------------------------------------------------
## Public API
## -------------------------------------------------------------------

func import_texture(source_path: String, destination_path: String = "") -> String:
	if not _supported_image_extensions.has(source_path.get_extension().to_lower()):
		push_error("Unsupported image format: " + source_path.get_extension())
		return ""
	
	if destination_path.is_empty():
		var file_name: String = source_path.get_file().get_basename() + ".png"
		destination_path = "res://assets/textures/" + file_name
	
	var dir := DirAccess.open("res://")
	var dest_dir: String = destination_path.get_base_dir()
	var path_parts: PackedStringArray = dest_dir.split("/")
	var current_path: String = path_parts[0]
	for i in range(1, path_parts.size()):
		current_path = current_path + "/" + path_parts[i]
		if not DirAccess.dir_exists_absolute(ProjectSettings.globalize_path(current_path)):
			dir.make_dir_recursive(current_path)
	
	var result: Error = DirAccess.copy_absolute(
		ProjectSettings.globalize_path(source_path),
		ProjectSettings.globalize_path(destination_path)
	)
	
	if result != OK:
		push_error("Failed to import texture: " + source_path)
		return ""
	
	emit_signal("asset_imported", destination_path, "texture")
	return destination_path

func import_sound(source_path: String, destination_path: String = "") -> String:
	if not _supported_audio_extensions.has(source_path.get_extension().to_lower()):
		push_error("Unsupported audio format: " + source_path.get_extension())
		return ""
	
	if destination_path.is_empty():
		var file_name: String = source_path.get_file().get_basename() + ".ogg"
		destination_path = "res://assets/sounds/" + file_name
	
	# For now, just copy the file (in future, could transcode to OGG)
	var result: Error = DirAccess.copy_absolute(
		ProjectSettings.globalize_path(source_path),
		ProjectSettings.globalize_path(destination_path)
	)
	
	if result != OK:
		push_error("Failed to import sound: " + source_path)
		return ""
	
	emit_signal("asset_imported", destination_path, "sound")
	return destination_path

func get_texture_size(path: String) -> Vector2i:
	var image := Image.load_from_file(ProjectSettings.globalize_path(path))
	if image == null:
		return Vector2i.ZERO
	return Vector2i(image.get_width(), image.get_height())

func create_placeholder_texture(width: int, height: int, color: Color) -> String:
	var image := Image.create(width, height, false, Image.FORMAT_RGBA8)
	image.fill(color)
	
	var path: String = "res://assets/textures/_placeholder.png"
	image.save_png(ProjectSettings.globalize_path(path))
	return path

func export_creature_assets(creature_id: String, export_dir: String) -> Error:
	emit_signal("export_started", creature_id)
	
	var creature: Dictionary = CreatureDB.get_creature(creature_id)
	if creature.is_empty():
		emit_signal("export_failed", creature_id, "Creature not found")
		return ERR_DOES_NOT_EXIST
	
	# Create export directory
	var dir := DirAccess.open("res://")
	var abs_export_dir: String = ProjectSettings.globalize_path(export_dir)
	dir.make_dir_recursive(abs_export_dir)
	
	# Export creature definition
	var definition_path: String = export_dir + "/" + creature_id + ".definition.json"
	var def_json: String = JSON.stringify(creature, "\t")
	var def_file := FileAccess.open(ProjectSettings.globalize_path(definition_path), FileAccess.WRITE)
	if def_file:
		def_file.store_string(def_json)
		def_file.close()
	
	# Export visual assets
	if creature.has("visual"):
		_export_visual_assets(creature.visual, export_dir, creature_id)
	
	# Export audio assets
	if creature.has("audio"):
		_export_audio_assets(creature.audio, export_dir, creature_id)
	
	emit_signal("export_completed", creature_id, export_dir)
	return OK

## -------------------------------------------------------------------
## Private helpers
## -------------------------------------------------------------------

func _export_visual_assets(visual: Dictionary, export_dir: String, creature_id: String):
	var abs_export_dir: String = ProjectSettings.globalize_path(export_dir)
	var dir := DirAccess.open("res://")
	dir.make_dir_recursive(abs_export_dir + "/textures")
	dir.make_dir_recursive(abs_export_dir + "/animations")
	
	if visual.has("texture"):
		var src_tex: String = visual.texture
		var src_abs: String = ProjectSettings.globalize_path(src_tex)
		if FileAccess.file_exists(src_abs):
			var dest_tex: String = export_dir + "/textures/" + creature_id + ".png"
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest_tex))
	
	if visual.has("atlas"):
		var src_atlas: String = visual.atlas
		var src_abs: String = ProjectSettings.globalize_path(src_atlas)
		if FileAccess.file_exists(src_abs):
			var dest_atlas: String = export_dir + "/textures/" + creature_id + "_atlas.json"
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest_atlas))

func _export_audio_assets(audio: Dictionary, export_dir: String, creature_id: String):
	var abs_export_dir: String = ProjectSettings.globalize_path(export_dir)
	var dir := DirAccess.open("res://")
	dir.make_dir_recursive(abs_export_dir + "/sounds")
	
	for sound_key in audio:
		var src_sound: String = audio[sound_key]
		var src_abs: String = ProjectSettings.globalize_path(src_sound)
		if FileAccess.file_exists(src_abs):
			var ext: String = src_sound.get_extension()
			var dest_sound: String = export_dir + "/sounds/" + creature_id + "_" + sound_key + "." + ext
			DirAccess.copy_absolute(src_abs, ProjectSettings.globalize_path(dest_sound))
