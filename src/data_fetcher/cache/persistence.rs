use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::paths::get_cache_dir_path;

/// Cached player name data for a single game.
#[derive(Serialize, Deserialize, Clone)]
pub struct CachedGamePlayers {
    pub goal_count: u32,
    pub players: HashMap<i64, String>,
}

/// Persistent store for disambiguated player names, backed by a JSON file per season.
///
/// Unlike `TtlCache`, this store has no LRU eviction or TTL — entries persist
/// for the entire season. The `dirty` flag tracks whether new data has been
/// added since the last save, enabling debounced writes.
pub struct PlayerNameStore {
    data: RwLock<HashMap<i32, CachedGamePlayers>>,
    dirty: AtomicBool,
    loaded_season: RwLock<Option<i32>>,
}

impl Default for PlayerNameStore {
    fn default() -> Self {
        Self::new()
    }
}

pub static PLAYER_NAME_STORE: LazyLock<PlayerNameStore> = LazyLock::new(PlayerNameStore::new);

impl PlayerNameStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            dirty: AtomicBool::new(false),
            loaded_season: RwLock::new(None),
        }
    }

    /// Returns cached player names for a game if the goal count matches.
    ///
    /// Returns `None` if the game is not cached or if the cached goal count
    /// differs from `expected_goal_count` (indicating stale data).
    pub async fn get(
        &self,
        game_id: i32,
        expected_goal_count: u32,
    ) -> Option<HashMap<i64, String>> {
        let data = self.data.read().await;
        match data.get(&game_id) {
            Some(cached) if cached.goal_count == expected_goal_count => {
                debug!(
                    "Player name store hit for game_id={game_id} (goal_count={expected_goal_count})"
                );
                Some(cached.players.clone())
            }
            Some(cached) => {
                debug!(
                    "Player name store stale for game_id={game_id}: cached goal_count={}, expected={expected_goal_count}",
                    cached.goal_count
                );
                None
            }
            None => None,
        }
    }

    /// Inserts disambiguated player names for a completed game.
    pub async fn insert(&self, game_id: i32, goal_count: u32, players: HashMap<i64, String>) {
        let player_count = players.len();
        let mut data = self.data.write().await;
        data.insert(
            game_id,
            CachedGamePlayers {
                goal_count,
                players,
            },
        );
        self.dirty.store(true, Ordering::Release);
        debug!("Player name store: cached {player_count} players for game_id={game_id}");
    }

    /// Loads cached player names from disk for the given season.
    ///
    /// Only loads once per season — subsequent calls for the same season are no-ops.
    /// If the file is missing or corrupted, starts with an empty store.
    pub async fn load_from_disk(&self, season: i32) {
        {
            let loaded = self.loaded_season.read().await;
            if *loaded == Some(season) {
                return;
            }
        }

        let path = cache_file_path(season);
        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => {
                match serde_json::from_str::<HashMap<i32, CachedGamePlayers>>(&contents) {
                    Ok(cached_data) => {
                        let count = cached_data.len();
                        let mut data = self.data.write().await;
                        *data = cached_data;
                        info!(
                            "Loaded {count} cached player entries from {}",
                            path.display()
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Corrupted player cache at {}, starting fresh: {e}",
                            path.display()
                        );
                        let mut data = self.data.write().await;
                        data.clear();
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("No player cache file at {}, starting fresh", path.display());
            }
            Err(e) => {
                warn!("Failed to read player cache at {}: {e}", path.display());
            }
        }

        let mut loaded = self.loaded_season.write().await;
        *loaded = Some(season);
        self.dirty.store(false, Ordering::Release);
    }

    /// Writes cached player names to disk if new data has been added since the last save.
    pub async fn save_to_disk(&self, season: i32) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }

        let path = cache_file_path(season);

        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            warn!("Failed to create cache directory {}: {e}", parent.display());
            return;
        }

        let data = self.data.read().await;
        match serde_json::to_string_pretty(&*data) {
            Ok(json) => match tokio::fs::write(&path, json).await {
                Ok(()) => {
                    self.dirty.store(false, Ordering::Release);
                    info!(
                        "Saved {} player cache entries to {}",
                        data.len(),
                        path.display()
                    );
                }
                Err(e) => {
                    warn!("Failed to write player cache to {}: {e}", path.display());
                }
            },
            Err(e) => {
                warn!("Failed to serialize player cache: {e}");
            }
        }
    }

    /// Returns the number of cached game entries.
    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }

    /// Returns whether the store has been modified since the last save.
    #[cfg(test)]
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    /// Clears all entries and resets state.
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
        self.dirty.store(false, Ordering::Release);
        let mut loaded = self.loaded_season.write().await;
        *loaded = None;
    }
}

fn cache_file_path(season: i32) -> PathBuf {
    get_cache_dir_path().join(format!("players_{season}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = PlayerNameStore::new();
        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());
        players.insert(200, "Selänne".to_string());

        store.insert(1001, 5, players.clone()).await;

        let result = store.get(1001, 5).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_get_returns_none_for_missing_game() {
        let store = PlayerNameStore::new();
        assert!(store.get(9999, 3).await.is_none());
    }

    #[tokio::test]
    async fn test_staleness_detection() {
        let store = PlayerNameStore::new();
        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());

        store.insert(1001, 5, players).await;

        // Same goal count → hit
        assert!(store.get(1001, 5).await.is_some());

        // Different goal count → stale, returns None
        assert!(store.get(1001, 6).await.is_none());
    }

    #[tokio::test]
    async fn test_dirty_flag() {
        let store = PlayerNameStore::new();
        assert!(!store.is_dirty());

        store.insert(1001, 3, HashMap::new()).await;
        assert!(store.is_dirty());
    }

    #[tokio::test]
    async fn test_save_and_load_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let season = 2026;

        // Create store with data
        let store = PlayerNameStore::new();
        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());
        players.insert(200, "Selänne M.".to_string());
        store.insert(1001, 5, players).await;
        store
            .insert(1002, 3, HashMap::from([(300, "Barkov".to_string())]))
            .await;

        // Save to temp dir (we test using the file directly since cache_file_path uses config dir)
        let path = temp_dir.path().join(format!("players_{season}.json"));
        let data = store.data.read().await;
        let json = serde_json::to_string_pretty(&*data).unwrap();
        drop(data);
        tokio::fs::write(&path, &json).await.unwrap();

        // Load into a new store from the file
        let store2 = PlayerNameStore::new();
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let loaded: HashMap<i32, CachedGamePlayers> = serde_json::from_str(&contents).unwrap();
        {
            let mut data = store2.data.write().await;
            *data = loaded;
        }

        assert_eq!(store2.len().await, 2);
        let result = store2.get(1001, 5).await;
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&200), Some(&"Selänne M.".to_string()));
    }

    #[tokio::test]
    async fn test_corrupted_file_handled_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("players_2026.json");
        tokio::fs::write(&path, "not valid json{{{").await.unwrap();

        // Directly test deserialization failure
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let result: Result<HashMap<i32, CachedGamePlayers>, _> = serde_json::from_str(&contents);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_file_starts_empty() {
        let store = PlayerNameStore::new();
        // Without loading, store is empty
        assert_eq!(store.len().await, 0);
    }

    #[tokio::test]
    async fn test_load_idempotent_for_same_season() {
        let store = PlayerNameStore::new();

        // Simulate loading season (will be a no-op since file doesn't exist, but sets loaded_season)
        {
            let mut loaded = store.loaded_season.write().await;
            *loaded = Some(2026);
        }

        // Insert data after "loading"
        store
            .insert(1001, 3, HashMap::from([(100, "Test".to_string())]))
            .await;

        // Second load for same season should be a no-op (data preserved)
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn test_serialization_format() {
        let mut data = HashMap::new();
        data.insert(
            1001,
            CachedGamePlayers {
                goal_count: 5,
                players: HashMap::from([(100, "Koivu".to_string()), (200, "Selänne".to_string())]),
            },
        );

        let json = serde_json::to_string_pretty(&data).unwrap();
        let deserialized: HashMap<i32, CachedGamePlayers> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        let entry = deserialized.get(&1001).unwrap();
        assert_eq!(entry.goal_count, 5);
        assert_eq!(entry.players.len(), 2);
    }
}
