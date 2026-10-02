extends Control
## Main Creature Editor UI
##
## Provides the interface for creating, editing, and exporting creatures.
## Includes a preview panel, property editor, and export controls.

@onready var creature_list: ItemList = $MainLayout/CreatureList
@onready var preview_viewport: SubViewport = $MainLayout/PreviewPanel/SubViewportContainer/PreviewViewport
@onready var property_panel: Control = $MainLayout/PropertyPanel
@onready var status_bar: Label = $StatusBar
@onready var animation_player: AnimationPlayer = $AnimationPlayer

var _current_creature_id: String = ""
var _preview_node: Node2D = null

func _ready():
	_refresh_creature_list()
	_connect_signals()
	_update_status("Ready")

func _connect_signals():
	ExportPlugin.validation_completed.connect(_on_validation_completed)
	ExportPlugin.export_progress.connect(_on_export_progress)
	AssetManager.export_started.connect(_on_export_started)
	AssetManager.export_completed.connect(_on_export_completed)
	AssetManager.export_failed.connect(_on_export_failed)

func _refresh_creature_list():
	creature_list.clear()
	var creatures: Dictionary = CreatureDB.get_all_creatures()
	for creature_id in creatures:
		var creature: Dictionary = creatures[creature_id]
		var name: String = creature.get("name", creature_id)
		var category: String = creature.get("category", "unknown")
		creature_list.add_item(name + " (" + category + ")")
		creature_list.set_item_metadata(creature_list.get_item_count() - 1, creature_id)

func _on_creature_selected(index: int):
	var creature_id: String = creature_list.get_item_metadata(index)
	_load_creature(creature_id)

func _load_creature(creature_id: String):
	_current_creature_id = creature_id
	var creature: Dictionary = CreatureDB.get_creature(creature_id)
	if creature.is_empty():
		return
	
	_clear_preview()
	_create_preview(creature)
	_populate_properties(creature)
	_update_status("Loaded: " + creature.get("name", creature_id))

func _clear_preview():
	if _preview_node:
		_preview_node.queue_free()
		_preview_node = null
	
	for child in preview_viewport.get_children():
		child.queue_free()

func _create_preview(creature: Dictionary):
	# Create a basic preview based on creature type
	var visual: Dictionary = creature.get("visual", {})
	var format: String = visual.get("animation_format", "frame")
	
	match format:
		"sprite_sheet", "frame":
			_preview_node = _create_sprite_preview(visual)
		"spine":
			_preview_node = _create_spine_preview(visual)
		_:
			_preview_node = _create_placeholder_preview(creature)
	
	if _preview_node:
		preview_viewport.add_child(_preview_node)

func _create_sprite_preview(visual: Dictionary) -> Node2D:
	var sprite := Sprite2D.new()
	
	var texture_path: String = visual.get("texture", "")
	if not texture_path.is_empty():
		var texture := load(texture_path)
		if texture:
			sprite.texture = texture
	
	var scale: float = visual.get("default_scale", 1.0)
	sprite.scale = Vector2(scale, scale)
	
	return sprite

func _create_spine_preview(visual: Dictionary) -> Node2D:
	# Placeholder for spine skeleton preview
	var container := Node2D.new()
	var label := Label.new()
	label.text = "Spine Skeleton Preview\n(Requires Spine runtime)"
	label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	label.position = Vector2(-100, -20)
	container.add_child(label)
	return container

func _create_placeholder_preview(creature: Dictionary) -> Node2D:
	var container := Node2D.new()
	
	var rect := ColorRect.new()
	rect.color = Color(0.3, 0.3, 0.3, 1.0)
	rect.size = Vector2(128, 128)
	rect.position = Vector2(-64, -64)
	container.add_child(rect)
	
	var label := Label.new()
	label.text = creature.get("name", "Creature")
	label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	label.position = Vector2(-50, -10)
	container.add_child(label)
	
	return container

func _populate_properties(creature: Dictionary):
	# Clear existing properties
	for child in property_panel.get_children():
		if child is Control and child != property_panel:
			child.queue_free()
	
	# Build property editor dynamically
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	property_panel.add_child(scroll)
	
	var container := VBoxContainer.new()
	container.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(container)
	
	# Basic info section
	_add_section_header(container, "Basic Information")
	_add_property_field(container, "ID", creature.get("id", ""), false)
	_add_property_field(container, "Name", creature.get("name", ""), true)
	_add_property_field(container, "Category", creature.get("category", ""), true)
	_add_property_field(container, "Description", creature.get("description", ""), true)
	
	# Visual section
	if creature.has("visual"):
		_add_section_header(container, "Visual")
		var visual: Dictionary = creature.visual
		_add_property_field(container, "Format", visual.get("animation_format", ""), false)
		_add_property_field(container, "Scale", str(visual.get("default_scale", 1.0)), true)
	
	# Behavior section
	if creature.has("behavior"):
		_add_section_header(container, "Behavior")
		var behavior: Dictionary = creature.behavior
		_add_property_field(container, "Default State", behavior.get("default_state", ""), false)
		
		if behavior.has("personality"):
			_add_section_header(container, "Personality")
			var personality: Dictionary = behavior.personality
			for trait in personality:
				_add_property_field(container, trait.capitalize(), str(personality[trait]), true)

func _add_section_header(container: VBoxContainer, text: String):
	var header := Label.new()
	header.text = text
	header.add_theme_font_size_override("font_size", 16)
	header.add_theme_color_override("font_color", Color(0.8, 0.8, 1.0))
	container.add_child(header)
	container.add_child(HSeparator.new())

func _add_property_field(container: BoxContainer, label: String, value: String, editable: bool):
	var row := HBoxContainer.new()
	row.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	
	var label_node := Label.new()
	label_node.text = label + ":"
	label_node.custom_minimum_size = Vector2(120, 0)
	row.add_child(label_node)
	
	var line_edit := LineEdit.new()
	line_edit.text = value
	line_edit.editable = editable
	line_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(line_edit)
	
	container.add_child(row)

func _on_export_button_pressed():
	if _current_creature_id.is_empty():
		_update_status("No creature selected")
		return
	
	var output_dir: String = "res://exports/" + _current_creature_id
	var err: Error = ExportPlugin.export_creature(_current_creature_id, output_dir)
	
	if err != OK:
		_update_status("Export failed: " + error_string(err))

func _on_export_all_button_pressed():
	var results: Dictionary = ExportPlugin.export_all_creatures("res://exports")
	_update_status("Export complete: " + str(results.success.size()) + " succeeded, " + str(results.failed.size()) + " failed")

func _on_validation_completed(creature_id: String, errors: Array[String]):
	if errors.is_empty():
		_update_status("Validation passed for: " + creature_id)
	else:
		_update_status("Validation failed: " + str(errors.size()) + " errors")

func _on_export_progress(creature_id: String, step: String, progress: float):
	_update_status("Exporting " + creature_id + ": " + step + " (" + str(int(progress * 100)) + "%)")

func _on_export_started(creature_id: String):
	_update_status("Export started: " + creature_id)

func _on_export_completed(creature_id: String, path: String):
	_update_status("Export complete: " + creature_id + " -> " + path)

func _on_export_failed(creature_id: String, reason: String):
	_update_status("Export failed: " + creature_id + " - " + reason)

func _update_status(text: String):
	status_bar.text = text
	print("[ScreenBuddy Editor] " + text)
