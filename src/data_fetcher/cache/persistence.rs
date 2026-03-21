use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

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
    base_path: PathBuf,
}

pub static PLAYER_NAME_STORE: LazyLock<PlayerNameStore> = LazyLock::new(PlayerNameStore::default);

impl Default for PlayerNameStore {
    fn default() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            dirty: AtomicBool::new(false),
            loaded_season: RwLock::new(None),
            base_path: get_cache_dir_path(),
        }
    }
}

impl PlayerNameStore {
    #[cfg(test)]
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            dirty: AtomicBool::new(false),
            loaded_season: RwLock::new(None),
            base_path,
        }
    }

    fn cache_file_path(&self, season: i32) -> PathBuf {
        self.base_path.join(format!("players_{season}.json"))
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

        let path = self.cache_file_path(season);
        let should_mark_loaded = match tokio::fs::read_to_string(&path).await {
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
                        error!(
                            "Corrupted player cache at {}, starting fresh: {e}",
                            path.display()
                        );
                        let mut data = self.data.write().await;
                        data.clear();
                    }
                }
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("No player cache file at {}, starting fresh", path.display());
                true
            }
            Err(e) => {
                error!(
                    "Failed to read player cache at {}: {e} — will retry on next fetch cycle",
                    path.display()
                );
                false
            }
        };

        if should_mark_loaded {
            let mut loaded = self.loaded_season.write().await;
            *loaded = Some(season);
            self.dirty.store(false, Ordering::Release);
        }
    }

    /// Writes cached player names to disk if new data has been added since the last save.
    ///
    /// Derives the season from the previously loaded season. No-op if nothing was loaded
    /// or if no new data has been added since the last save.
    pub async fn save_to_disk(&self) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }

        let season = {
            let loaded = self.loaded_season.read().await;
            match *loaded {
                Some(s) => s,
                None => return,
            }
        };

        let path = self.cache_file_path(season);

        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            error!("Failed to create cache directory {}: {e}", parent.display());
            return;
        }

        let data = self.data.read().await;
        match serde_json::to_string_pretty(&*data) {
            Ok(json) => {
                let tmp_path = path.with_extension("json.tmp");
                if let Err(e) = tokio::fs::write(&tmp_path, &json).await {
                    error!(
                        "Failed to write player cache to {}: {e}",
                        tmp_path.display()
                    );
                    return;
                }
                if let Err(e) = tokio::fs::rename(&tmp_path, &path).await {
                    error!(
                        "Failed to rename player cache {} -> {}: {e}",
                        tmp_path.display(),
                        path.display()
                    );
                    return;
                }
                self.dirty.store(false, Ordering::Release);
                info!(
                    "Saved {} player cache entries to {}",
                    data.len(),
                    path.display()
                );
            }
            Err(e) => {
                error!("Failed to serialize player cache: {e}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = PlayerNameStore::default();
        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());
        players.insert(200, "Selänne".to_string());

        store.insert(1001, 5, players).await;

        let result = store.get(1001, 5).await;
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&200), Some(&"Selänne".to_string()));
    }

    #[tokio::test]
    async fn test_get_returns_none_for_missing_game() {
        let store = PlayerNameStore::default();
        assert!(store.get(9999, 3).await.is_none());
    }

    #[tokio::test]
    async fn test_staleness_detection() {
        let store = PlayerNameStore::default();
        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());

        store.insert(1001, 5, players).await;

        // Same goal count → hit
        assert!(store.get(1001, 5).await.is_some());

        // Different goal count → stale, returns None
        assert!(store.get(1001, 6).await.is_none());
    }

    #[tokio::test]
    async fn test_dirty_flag_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        assert!(!store.is_dirty());

        // Load sets up the season (empty file = fresh start)
        store.load_from_disk(2026).await;
        assert!(!store.is_dirty());

        // Insert sets dirty
        store.insert(1001, 3, HashMap::new()).await;
        assert!(store.is_dirty());

        // Save clears dirty
        store.save_to_disk().await;
        assert!(!store.is_dirty());
    }

    #[tokio::test]
    async fn test_save_and_load_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let season = 2026;

        // Create store pointing at temp dir, load season, insert data
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(season).await;

        let mut players = HashMap::new();
        players.insert(100, "Koivu".to_string());
        players.insert(200, "Selänne M.".to_string());
        store.insert(1001, 5, players).await;
        store
            .insert(1002, 3, HashMap::from([(300, "Barkov".to_string())]))
            .await;

        // Save using the real method
        store.save_to_disk().await;

        // Verify file was created
        let path = temp_dir.path().join(format!("players_{season}.json"));
        assert!(path.exists());

        // Load into a fresh store using the real method
        let store2 = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store2.load_from_disk(season).await;

        assert_eq!(store2.len().await, 2);
        let result = store2.get(1001, 5).await;
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&200), Some(&"Selänne M.".to_string()));

        let result2 = store2.get(1002, 3).await;
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().get(&300), Some(&"Barkov".to_string()));
    }

    #[tokio::test]
    async fn test_corrupted_file_handled_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("players_2026.json");
        tokio::fs::write(&path, "not valid json{{{").await.unwrap();

        // Load from corrupted file — should recover gracefully with empty store
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 0);

        // Store should still be usable after corruption recovery
        store
            .insert(1001, 3, HashMap::from([(100, "Test".to_string())]))
            .await;
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn test_missing_file_starts_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        // Load from nonexistent file — should start empty and be operational
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 0);

        // Store should be usable
        store
            .insert(1001, 3, HashMap::from([(100, "Test".to_string())]))
            .await;
        assert!(store.get(1001, 3).await.is_some());
    }

    #[tokio::test]
    async fn test_load_idempotent_for_same_season() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        // First load
        store.load_from_disk(2026).await;

        // Insert data after loading
        store
            .insert(1001, 3, HashMap::from([(100, "Test".to_string())]))
            .await;

        // Second load for same season should be a no-op (data preserved)
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn test_save_noop_when_not_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(2026).await;

        // Save without inserting — should be a no-op (no file created)
        store.save_to_disk().await;
        let path = temp_dir.path().join("players_2026.json");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_insert_overwrites_existing_entry() {
        let store = PlayerNameStore::default();
        store
            .insert(1001, 5, HashMap::from([(100, "Koivu".to_string())]))
            .await;
        store
            .insert(
                1001,
                6,
                HashMap::from([(100, "Koivu".to_string()), (200, "Selänne".to_string())]),
            )
            .await;

        // Old entry (goal_count=5) should be gone
        assert!(store.get(1001, 5).await.is_none());

        // New entry (goal_count=6) should be present
        let result = store.get(1001, 6).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
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
