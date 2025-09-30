use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Player {
    pub id: i64,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
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
    }

    #[test]
    fn test_player_with_special_characters() {
        let player = Player {
            id: 54321,
            last_name: "Kärppä".to_string(),
            first_name: "Äkäslompolo".to_string(),
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
        };

        let cloned = player.clone();
        assert_eq!(player.id, cloned.id);
        assert_eq!(player.last_name, cloned.last_name);
        assert_eq!(player.first_name, cloned.first_name);
    }
}
