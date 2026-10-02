extends SceneTree
## Godot CLI Harness for ScreenBuddy
##
## This script provides headless (no GUI) access to creature management
## functions. It's used by the godot-harness.sh wrapper script for
## CI/CD pipelines and automated workflows.
##
## Usage (via wrapper):
##   ./godot-harness.sh --export-creature <id> --output <dir>
##   ./godot-harness.sh --export-all --output <dir>
##   ./godot-harness.sh --list-creatures
##   ./godot-harness.sh --validate <id>

func _init():
	_parse_arguments()

func _parse_arguments():
	var args: PackedStringArray = OS.get_cmdline_args()
	var command: String = ""
	var creature_id: String = ""
	var output_dir: String = ""
	
	var i: int = 0
	while i < args.size():
		var arg: String = args[i]
		
		if arg == "export" and i + 1 < args.size():
			i += 1
			creature_id = args[i]
			command = "export"
		elif arg == "export-all":
			command = "export_all"
		elif arg == "list":
			command = "list"
		elif arg == "validate" and i + 1 < args.size():
			i += 1
			creature_id = args[i]
			command = "validate"
		elif arg == "output" and i + 1 < args.size():
			i += 1
			output_dir = args[i]
		
		i += 1
	
	# Execute command
	match command:
		"export":
			_cmd_export(creature_id, output_dir)
		"export_all":
			_cmd_export_all(output_dir)
		"list":
			_cmd_list()
		"validate":
			_cmd_validate(creature_id)
		_:
			print("[harness] Unknown command: " + command)
			print("[harness] Available: export, export-all, list, validate")
	
	quit()

func _cmd_export(creature_id: String, output_dir: String):
	if creature_id.is_empty():
		print("[harness] ERROR: No creature ID specified")
		return
	
	if output_dir.is_empty():
		output_dir = "res://exports/" + creature_id
	
	print("[harness] Exporting creature: " + creature_id)
	
	# Load creature from DB
	var creature: Dictionary = CreatureDB.get_creature(creature_id)
	if creature.is_empty():
		print("[harness] ERROR: Creature not found: " + creature_id)
		return
	
	# Validate first
	var errors: Array[String] = ExportPlugin.validate_creature(creature)
	if not errors.is_empty():
		print("[harness] VALIDATION FAILED:")
		for error in errors:
			print("  - " + error)
		return
	
	# Export
	var err: Error = ExportPlugin.export_creature(creature_id, output_dir)
	if err == OK:
		print("[harness] SUCCESS: Exported to " + output_dir)
	else:
		print("[harness] ERROR: Export failed with code " + str(err))

func _cmd_export_all(output_dir: String):
	if output_dir.is_empty():
		output_dir = "res://exports"
	
	print("[harness] Exporting all creatures...")
	var results: Dictionary = ExportPlugin.export_all_creatures(output_dir)
	print("[harness] Success: " + str(results.success.size()))
	print("[harness] Failed: " + str(results.failed.size()))
	
	if not results.failed.is_empty():
		print("[harness] Failed creatures:")
		for id in results.failed:
			print("  - " + id)

func _cmd_list():
	print("[harness] Available creatures:")
	var creatures: Dictionary = CreatureDB.get_all_creatures()
	for creature_id in creatures:
		var creature: Dictionary = creatures[creature_id]
		var name: String = creature.get("name", creature_id)
		var category: String = creature.get("category", "unknown")
		print("  - " + creature_id + " (" + name + ", " + category + ")")

func _cmd_validate(creature_id: String):
	if creature_id.is_empty():
		print("[harness] ERROR: No creature ID specified")
		return
	
	print("[harness] Validating: " + creature_id)
	
	var creature: Dictionary = CreatureDB.get_creature(creature_id)
	if creature.is_empty():
		print("[harness] ERROR: Creature not found: " + creature_id)
		return
	
	var errors: Array[String] = ExportPlugin.validate_creature(creature)
	if errors.is_empty():
		print("[harness] VALIDATION PASSED")
	else:
		print("[harness] VALIDATION FAILED:")
		for error in errors:
			print("  - " + error)
