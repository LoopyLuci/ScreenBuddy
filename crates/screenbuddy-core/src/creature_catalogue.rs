//! The creature catalogue.
//!
//! Previously this was a tuple array hardcoded in the desktop binary's `main`,
//! which meant the GUI had no way to know what creatures exist, what they are
//! called, or which one a saved agent referred to. It lives here now so the
//! editor's picker, the IPC layer and the running app share one list.

use crate::screen_physics::CreaturePersonality;
use serde::{Deserialize, Serialize};

/// One selectable creature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureInfo {
    /// Stable id, matching the sprite asset name.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Default movement behaviour.
    pub default_personality: CreaturePersonality,
    /// One-line description, shown in the picker so the choice is informed.
    pub description: String,
}

impl CreatureInfo {
    fn new(
        id: &str,
        name: &str,
        default_personality: CreaturePersonality,
        description: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            default_personality,
            description: description.to_string(),
        }
    }
}

/// Every creature that can be selected.
pub fn creature_catalogue() -> Vec<CreatureInfo> {
    vec![
        CreatureInfo::new(
            "companion-bird-01",
            "Companion Bird",
            CreaturePersonality::Curious,
            "Follows your cursor. Small and quick.",
        ),
        CreatureInfo::new(
            "robo-cat-01",
            "Robo-Cat",
            CreaturePersonality::Energetic,
            "Fast and playful. Never sits still.",
        ),
        CreatureInfo::new(
            "slime-king-01",
            "Slime King",
            CreaturePersonality::Lazy,
            "Slow and deliberate. Rarely hurries.",
        ),
        CreatureInfo::new(
            "pixel-wizard-01",
            "Pixel Wizard",
            CreaturePersonality::Neutral,
            "Wanders calmly. The sensible choice.",
        ),
        CreatureInfo::new(
            "cosmic-jellyfish-01",
            "Cosmic Jellyfish",
            CreaturePersonality::Shy,
            "Drifts away from the cursor.",
        ),
        CreatureInfo::new(
            "dragon-01",
            "Dragon",
            CreaturePersonality::Neutral,
            "Wanders calmly, at scale.",
        ),
        CreatureInfo::new(
            "ghost-01",
            "Ghost",
            CreaturePersonality::Shy,
            "Fades out of the way of your pointer.",
        ),
    ]
}

/// Look up one creature by id.
pub fn creature_info(id: &str) -> Option<CreatureInfo> {
    creature_catalogue().into_iter().find(|c| c.id == id)
}

/// The movement behaviour a creature uses by default.
pub fn creature_default_personality(id: &str) -> Option<CreaturePersonality> {
    creature_info(id).map(|c| c.default_personality)
}

/// The first creature, used when an agent names one that does not exist.
///
/// Resolution must be total: a profile saved against a creature that was later
/// removed should still show something rather than a blank screen.
pub fn fallback_creature() -> CreatureInfo {
    creature_catalogue()
        .into_iter()
        .next()
        .expect("the catalogue is never empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_is_not_empty() {
        assert!(!creature_catalogue().is_empty());
    }

    #[test]
    fn creature_ids_are_unique() {
        let catalogue = creature_catalogue();
        let ids: Vec<&str> = catalogue.iter().map(|c| c.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            ids.len(),
            sorted.len(),
            "duplicate creature id in the catalogue"
        );
    }

    #[test]
    fn every_creature_has_a_name_and_description() {
        // A picker entry with a blank description gives the user nothing to go on.
        for creature in creature_catalogue() {
            assert!(
                !creature.name.trim().is_empty(),
                "{} has no name",
                creature.id
            );
            assert!(
                !creature.description.trim().is_empty(),
                "{} has no description",
                creature.id
            );
        }
    }

    #[test]
    fn lookup_finds_known_creatures() {
        let found = creature_info("robo-cat-01").expect("known creature");
        assert_eq!(found.name, "Robo-Cat");
    }

    #[test]
    fn lookup_returns_none_for_unknown_ids() {
        assert!(creature_info("not-a-creature").is_none());
    }

    #[test]
    fn the_fallback_is_always_available() {
        // A profile referencing a removed creature must still resolve to
        // something drawable.
        let fallback = fallback_creature();
        assert!(!fallback.id.is_empty());
    }

    #[test]
    fn default_personalities_come_from_the_catalogue() {
        let expected = creature_info("robo-cat-01")
            .expect("creature")
            .default_personality;
        assert_eq!(creature_default_personality("robo-cat-01"), Some(expected));
        assert_eq!(creature_default_personality("nope"), None);
    }

    #[test]
    fn the_catalogue_covers_a_range_of_personalities() {
        // If every creature behaved identically, choosing one would be pointless.
        let distinct: std::collections::BTreeSet<_> = creature_catalogue()
            .iter()
            .map(|c| format!("{:?}", c.default_personality))
            .collect();
        assert!(
            distinct.len() >= 3,
            "expected varied personalities, got {distinct:?}"
        );
    }
}
