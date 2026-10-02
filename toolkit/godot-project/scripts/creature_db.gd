extends Node
## Creature Database - Autoload singleton for managing creature definitions
##
## This autoload provides centralized access to all creature data,
## including definitions, animations, behaviors, and metadata.

signal creature_added(creature_id: String)
signal creature_removed(creature_id: String)
signal creature_updated(creature_id: String)

var _creatures: Dictionary = {}  # id -> CreatureData
var _categories: Dictionary = {}  # category -> Array[String]

func _init():
	_load_builtin_creatures()

## -------------------------------------------------------------------
## Public API
## -------------------------------------------------------------------

func get_creature(creature_id: String) -> Dictionary:
	return _creatures.get(creature_id, {})

func get_all_creatures() -> Dictionary:
	return _creatures.duplicate()

func get_creatures_by_category(category: String) -> Array:
	var result: Array = []
	for creature_id in _creatures:
		if _creatures[creature_id].get("category", "") == category:
			result.append(_creatures[creature_id])
	return result

func get_all_categories() -> PackedStringArray:
	return PackedStringArray(_categories.keys())

func register_creature(creature_data: Dictionary) -> Error:
	var id: String = creature_data.get("id", "")
	if id.is_empty():
		push_error("Creature must have an id")
		return ERR_INVALID_DATA
	
	if _creatures.has(id):
		push_warning("Overwriting existing creature: " + id)
	
	_creatures[id] = creature_data
	
	var category: String = creature_data.get("category", "uncategorized")
	if not _categories.has(category):
		_categories[category] = []
	if not _categories[category].has(id):
		_categories[category].append(id)
	
	emit_signal("creature_added", id)
	return OK

func unregister_creature(creature_id: String) -> Error:
	if not _creatures.has(creature_id):
		return ERR_DOES_NOT_EXIST
	
	var category: String = _creatures[creature_id].get("category", "uncategorized")
	_creatures.erase(creature_id)
	
	if _categories.has(category):
		_categories[category].erase(creature_id)
		if _categories[category].is_empty():
			_categories.erase(category)
	
	emit_signal("creature_removed", creature_id)
	return OK

func save_creature_to_file(creature_id: String, path: String) -> Error:
	var data: Dictionary = get_creature(creature_id)
	if data.is_empty():
		return ERR_DOES_NOT_EXIST
	
	var json: String = JSON.stringify(data, "\t")
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return FileAccess.get_open_error()
	
	file.store_string(json)
	file.close()
	return OK

func load_creature_from_file(path: String) -> Error:
	if not FileAccess.file_exists(path):
		return ERR_FILE_NOT_FOUND
	
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		return FileAccess.get_open_error()
	
	var json: String = file.get_as_text()
	file.close()
	
	var result := JSON.parse_string(json)
	if result == null:
		return ERR_INVALID_DATA
	
	return register_creature(result as Dictionary)

## -------------------------------------------------------------------
## Built-in creatures
## -------------------------------------------------------------------

func _load_builtin_creatures():
	# These are placeholder definitions; actual creatures are loaded
	# from the creatures/ directory at runtime
	var creature_paths: Array = []
	var dir := DirAccess.open("res://creatures/")
	if dir == null:
		return
	
	dir.list_dir_begin()
	var file_name: String = dir.get_next()
	while file_name != "":
		if not dir.current_is_dir() and file_name.ends_with(".json"):
			creature_paths.append("res://creatures/" + file_name)
		file_name = dir.get_next()
	dir.list_dir_end()
	
	for path in creature_paths:
		load_creature_from_file(path)
