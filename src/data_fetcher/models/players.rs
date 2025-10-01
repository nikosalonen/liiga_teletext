use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Player {
    pub id: i64,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
    /// Line number the player is on (null if not in lineup)
    #[serde(default)]
    pub line: Option<i32>,
    /// Whether the player is injured
    #[serde(default)]
    pub injured: bool,
    /// Whether the player is suspended
    #[serde(default)]
    pub suspended: bool,
    /// Whether the player has been removed from the roster
    #[serde(default)]
    pub removed: bool,
}

impl Player {
    /// Determines if a player is active and should be included in disambiguation.
    /// A player is considered active if they:
    /// - Have a line assignment (not null)
    /// - Are not injured
    /// - Are not suspended
    /// - Have not been removed from the roster
    pub fn is_active(&self) -> bool {
        self.line.is_some() && !self.injured && !self.suspended && !self.removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_player_serialization() {
        let player = Player {
            id: 12345,
            last_name: "Koivu".to_string(),
            first_name: "Mikko".to_string(),
            line: Some(1),
            injured: false,
            suspended: false,
            removed: false,
        };

        // Test serialization
        let json = serde_json::to_string(&player).unwrap();
        assert!(json.contains("\"id\":12345"));
        assert!(json.contains("\"lastName\":\"Koivu\""));
        assert!(json.contains("\"firstName\":\"Mikko\""));

        // Test deserialization
        let deserialized: Player = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 12345);
        assert_eq!(deserialized.last_name, "Koivu");
        assert_eq!(deserialized.first_name, "Mikko");
        assert_eq!(deserialized.line, Some(1));
        assert!(!deserialized.injured);
    }

    #[test]
    fn test_player_with_special_characters() {
        let player = Player {
            id: 54321,
            last_name: "Kärppä".to_string(),
            first_name: "Äkäslompolo".to_string(),
            line: Some(2),
            injured: false,
            suspended: false,
            removed: false,
        };

        let json = serde_json::to_string(&player).unwrap();
        let deserialized: Player = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.last_name, "Kärppä");
        assert_eq!(deserialized.first_name, "Äkäslompolo");
    }

    #[test]
    fn test_player_clone() {
        let player = Player {
            id: 99999,
            last_name: "Test".to_string(),
            first_name: "Player".to_string(),
            line: Some(3),
            injured: false,
            suspended: false,
            removed: false,
        };

        let cloned = player.clone();
        assert_eq!(player.id, cloned.id);
        assert_eq!(player.last_name, cloned.last_name);
        assert_eq!(player.first_name, cloned.first_name);
    }

    #[test]
    fn test_is_active() {
        // Active player
        let active = Player {
            id: 1,
            first_name: "Active".to_string(),
            last_name: "Player".to_string(),
            line: Some(1),
            injured: false,
            suspended: false,
            removed: false,
        };
        assert!(active.is_active());

        // Injured player
        let injured = Player {
            id: 2,
            first_name: "Injured".to_string(),
            last_name: "Player".to_string(),
            line: Some(1),
            injured: true,
            suspended: false,
            removed: false,
        };
        assert!(!injured.is_active());

        // No line assignment
        let no_line = Player {
            id: 3,
            first_name: "Benched".to_string(),
            last_name: "Player".to_string(),
            line: None,
            injured: false,
            suspended: false,
            removed: false,
        };
        assert!(!no_line.is_active());

        // Suspended player
        let suspended = Player {
            id: 4,
            first_name: "Suspended".to_string(),
            last_name: "Player".to_string(),
            line: Some(1),
            injured: false,
            suspended: true,
            removed: false,
        };
        assert!(!suspended.is_active());

        // Removed player
        let removed = Player {
            id: 5,
            first_name: "Removed".to_string(),
            last_name: "Player".to_string(),
            line: Some(1),
            injured: false,
            suspended: false,
            removed: true,
        };
        assert!(!removed.is_active());
    }
}
