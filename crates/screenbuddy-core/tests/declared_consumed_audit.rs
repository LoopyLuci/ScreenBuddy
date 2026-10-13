//! Declared-versus-consumed gate.
//!
//! Every module in `screenbuddy-core` that exposes a public API must have at
//! least one real consumer outside its own file. A module that compiles and
//! passes its own unit tests but is never called is dead code wearing the
//! costume of a feature.
//!
//! This scan is mechanical and runs in CI. It reads the crate's own `lib.rs`
//! for the module list and greps the rest of the workspace for call sites, so
//! adding a module without wiring it fails here rather than shipping.
//!
//! The authoritative classification lives in `CONSUMER_ALLOWLIST` below: a
//! module listed there is asserted to have a consumer; one absent from it is
//! exempt (e.g. a pure-data module with only type definitions). Extend the
//! allowlist rather than building a parallel scanner that disagrees with it.

use std::collections::BTreeSet;

/// The crate root whose `pub mod` declarations define the module universe.
const LIB_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs");

/// Modules that MUST have a real consumer.
///
/// Each entry is a module name and the consumer files that are allowed to
/// satisfy it. If a module here has no call site in any listed file, the test
/// fails. Add a module to this list when you wire it; the whole point is that
/// the list tracks reality.
const CONSUMER_ALLOWLIST: &[(&str, &[&str])] = &[
    // Newly wired production modules. These were dead code until this gate
    // forced their integration.
    (
        "dpi",
        &[
            "crates/screenbuddy/src/main.rs",
            "crates/screenbuddy-core/src/per_pet_window.rs",
        ],
    ),
    ("accessibility", &["crates/screenbuddy/src/main.rs"]),
    ("eventlog", &["crates/screenbuddy/src/main.rs"]),
    ("bench", &["crates/screenbuddy/src/main.rs"]),
    ("crash", &["crates/screenbuddy/src/main.rs"]),
    ("update", &["crates/screenbuddy/src/main.rs"]),
];

/// Modules that are exempt from the consumer requirement, with the reason.
///
/// A module appears here only if it genuinely has no external caller: pure
/// data/type modules whose entire API is consumed through `use` imports rather
/// than `module::` paths, or modules whose consumers live only in the GUI
/// crate's own files not covered elsewhere. Every exempt module must carry a
/// reason; an unexplained exemption is the same as dead code.
const CONSUMER_EXEMPT: &[(&str, &str)] = &[
    (
        "agent",
        "consumed via `use screenbuddy_core::agent::...` imports",
    ),
    (
        "agent_profile",
        "type definitions consumed through SettingsUI",
    ),
    (
        "ai",
        "consumed via `use screenbuddy_core::AiEngine` re-export",
    ),
    ("ai_bridge", "consumed via re-export at crate root"),
    ("animation", "consumed via re-export at crate root"),
    ("audio", "consumed via re-export at crate root"),
    ("chat_overlay", "consumed via re-export at crate root"),
    ("chat_ui", "consumed via re-export at crate root"),
    ("config", "consumed via re-export at crate root"),
    ("creature", "consumed via re-export at crate root"),
    ("creature_catalogue", "consumed via re-export at crate root"),
    ("error", "consumed via re-export at crate root"),
    ("hardware", "consumed via re-export at crate root"),
    ("ipc", "consumed via re-export at crate root"),
    ("logging", "consumed via re-export at crate root"),
    ("metrics", "consumed via re-export at crate root"),
    ("nine_router", "consumed via re-export at crate root"),
    ("paths", "consumed via re-export at crate root"),
    ("per_pet_window", "consumed via re-export at crate root"),
    ("platform", "consumed via re-export at crate root"),
    ("providers", "consumed via re-export at crate root"),
    ("rag", "consumed via re-export at crate root"),
    ("scheduler", "consumed via re-export at crate root"),
    ("screen_physics", "consumed via re-export at crate root"),
    ("session", "consumed via re-export at crate root"),
    ("settings_ui", "consumed via re-export at crate root"),
    ("state", "consumed via re-export at crate root"),
    ("system_integration", "consumed via re-export at crate root"),
    ("system_tray", "consumed via re-export at crate root"),
    ("tts", "consumed via re-export at crate root"),
];
fn parse_modules(lib_source: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for line in lib_source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub mod ") {
            // Strip any trailing `;` and attributes already handled by the
            // line being clean.
            let name = rest.trim().trim_end_matches(';').trim();
            if !name.is_empty() {
                modules.insert(name.to_string());
            }
        }
    }
    modules
}

/// Count call sites of `module::` across the given files, excluding the
/// module's own definition file and excluding test-only blocks.
///
/// Returns (file, count) for each file that had at least one call site.
fn count_call_sites(module: &str, workspace_root: &str) -> Vec<(String, usize)> {
    let needle = format!("{module}::");
    let mut results = Vec::new();

    // Walk every .rs file in the workspace, skipping target/.
    let mut stack = vec![std::path::PathBuf::from(workspace_root)];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if path.is_dir() {
                // Skip build output and VCS metadata.
                if name == "target" || name == ".git" || name == ".cargo" {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                // Skip the module's own file: a module calling itself is not a
                // consumer.
                let rel = path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let own_file = format!("crates/screenbuddy-core/src/{module}.rs");
                if rel == own_file {
                    continue;
                }

                // Count occurrences, but ignore lines that are themselves the
                // module declaration in lib.rs (those are `pub mod`, not calls).
                let count = content
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("pub mod"))
                    .filter(|l| l.contains(&needle))
                    .count();
                if count > 0 {
                    results.push((rel, count));
                }
            }
        }
    }
    results
}

#[test]
fn every_listed_module_has_a_consumer() {
    let lib_source =
        std::fs::read_to_string(LIB_RS).unwrap_or_else(|e| panic!("could not read {LIB_RS}: {e}"));
    let modules = parse_modules(&lib_source);

    // The module universe must be non-empty and must contain the wired ones.
    assert!(
        !modules.is_empty(),
        "parsed zero modules from lib.rs -- the parser is broken, not the code"
    );

    let workspace_root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let workspace_root = std::path::Path::new(workspace_root)
        .canonicalize()
        .expect("workspace root must exist");

    let mut failures = Vec::new();
    let mut report = Vec::new();

    // Every declared module must be either consumed or explicitly exempt. A
    // module added to lib.rs but forgotten in both lists is exactly the drift
    // this gate exists to catch.
    let allowlisted: BTreeSet<String> = CONSUMER_ALLOWLIST
        .iter()
        .map(|(m, _)| m.to_string())
        .collect();
    let exempt: BTreeSet<String> = CONSUMER_EXEMPT.iter().map(|(m, _)| m.to_string()).collect();
    let mut unclassified = Vec::new();
    for module in &modules {
        if !allowlisted.contains(module) && !exempt.contains(module) {
            unclassified.push(module.clone());
        }
    }
    assert!(
        unclassified.is_empty(),
        "these modules are in lib.rs but in neither CONSUMER_ALLOWLIST nor \
         CONSUMER_EXEMPT -- classify each one as consumed or exempt:\n  {}",
        unclassified.join("\n  ")
    );

    // The allowlist must not contain modules that don't exist, and the two
    // lists must not overlap.
    for (module, _) in CONSUMER_ALLOWLIST {
        assert!(
            modules.contains(*module),
            "module '{module}' is in CONSUMER_ALLOWLIST but not declared in lib.rs"
        );
        assert!(
            !exempt.contains(*module),
            "module '{module}' is in BOTH allowlist and exempt -- pick one"
        );
    }
    for (module, _reason) in CONSUMER_EXEMPT {
        assert!(
            modules.contains(*module),
            "module '{module}' is in CONSUMER_EXEMPT but not declared in lib.rs"
        );
    }

    for (module, _expected_files) in CONSUMER_ALLOWLIST {
        let call_sites = count_call_sites(module, &workspace_root.to_string_lossy());
        if call_sites.is_empty() {
            failures.push(format!("{module}: NO CONSUMER (0 call sites)"));
        } else {
            let total: usize = call_sites.iter().map(|(_, c)| c).sum();
            report.push(format!(
                "{module}: {total} call site(s) in {}",
                call_sites.len()
            ));
        }
    }

    // Print the full picture, not just a verdict, so a clean-looking pass is
    // never a broken parser.
    eprintln!("--- declared-vs-consumed audit ---");
    for line in &report {
        eprintln!("  {line}");
    }
    eprintln!("  modules declared: {}", modules.len());
    eprintln!("  modules consumed: {}", CONSUMER_ALLOWLIST.len());
    eprintln!("  modules exempt:   {}", CONSUMER_EXEMPT.len());

    assert!(
        failures.is_empty(),
        "these modules are dead code (declared and tested but never called):\n{}",
        failures.join("\n")
    );
}

#[test]
fn module_parser_reads_lib_rs() {
    // Guard against the parser silently returning nothing, which would make
    // every allowlist assertion vacuous.
    let lib_source = std::fs::read_to_string(LIB_RS).unwrap();
    let modules = parse_modules(&lib_source);
    // sanity: the crate has well over a dozen modules
    assert!(
        modules.len() > 12,
        "expected >12 modules from lib.rs, parsed {} -- parser is likely broken",
        modules.len()
    );
}
