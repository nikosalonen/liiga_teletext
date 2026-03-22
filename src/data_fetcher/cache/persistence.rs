use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::paths::get_cache_dir_path;

/// Deletes all player cache files from the given directory.
/// Returns the count of deleted files.
pub async fn clear_all_cache_files_in(cache_dir: &std::path::Path) -> usize {
    let mut deleted = 0;
    let mut entries = match tokio::fs::read_dir(cache_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            debug!("Cache directory not accessible: {e}");
            return 0;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("players_") && name_str.ends_with(".json") {
            if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                warn!("Failed to delete {}: {e}", entry.path().display());
            } else {
                deleted += 1;
            }
        }
    }
    deleted
}

/// Deletes all persistent player cache files from the default cache directory.
/// Returns the count of deleted files.
pub async fn clear_all_cache_files() -> usize {
    let cache_dir = get_cache_dir_path();
    let count = clear_all_cache_files_in(&cache_dir).await;
    if count > 0 {
        info!(
            "Deleted {count} player cache file(s) from {}",
            cache_dir.display()
        );
    } else {
        info!("No player cache files found in {}", cache_dir.display());
    }
    count
}

/// Persistent store for disambiguated player names, keyed by team.
///
/// Stores a flat `team_id → (player_id → display_name)` map per season,
/// backed by a JSON file. Each player is stored exactly once under their
/// team, eliminating the per-game duplication of the previous design.
pub struct PlayerNameStore {
    data: RwLock<HashMap<String, HashMap<i64, String>>>,
    /// Mutation sequence counter. Zero means clean; each `insert_team` increments it.
    /// `save_to_disk` uses compare-exchange so concurrent inserts are not lost.
    dirty_seq: AtomicU64,
    loaded_season: RwLock<Option<i32>>,
    base_path: PathBuf,
}

pub(crate) static PLAYER_NAME_STORE: LazyLock<PlayerNameStore> =
    LazyLock::new(PlayerNameStore::default);

impl Default for PlayerNameStore {
    fn default() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            dirty_seq: AtomicU64::new(0),
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
            dirty_seq: AtomicU64::new(0),
            loaded_season: RwLock::new(None),
            base_path,
        }
    }

    fn cache_file_path(&self, season: i32) -> PathBuf {
        self.base_path.join(format!("players_{season}.json"))
    }

    /// Returns merged player names for both teams if both rosters are cached.
    ///
    /// Returns `None` if either team ID is missing or either team's roster
    /// has not been cached yet, signalling that an API fetch is needed.
    pub async fn get_players(
        &self,
        home_team_id: Option<&str>,
        away_team_id: Option<&str>,
    ) -> Option<HashMap<i64, String>> {
        let (home_id, away_id) = match (home_team_id, away_team_id) {
            (Some(h), Some(a)) => (h, a),
            _ => return None,
        };

        let data = self.data.read().await;
        let home = data.get(home_id)?;
        let away = data.get(away_id)?;

        let mut merged = home.clone();
        merged.extend(away.iter().map(|(k, v)| (*k, v.clone())));
        debug!(
            "Player name store hit for {home_id} vs {away_id} ({} players)",
            merged.len()
        );
        Some(merged)
    }

    /// Inserts a team's disambiguated roster into the store.
    ///
    /// Merges with any existing entries for the team, so new players
    /// from later games are accumulated.
    pub async fn insert_team(&self, team_id: &str, players: HashMap<i64, String>) {
        let player_count = players.len();
        let mut data = self.data.write().await;
        let entry = data.entry(team_id.to_string()).or_default();
        entry.extend(players);
        self.dirty_seq.fetch_add(1, Ordering::AcqRel);
        debug!("Player name store: cached {player_count} players for team {team_id}");
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

        // Save any pending data from the previous season before switching
        if self.dirty_seq.load(Ordering::Acquire) != 0 {
            info!("Season changed, saving pending data before loading season {season}");
            self.save_to_disk().await;
        }

        let path = self.cache_file_path(season);
        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => {
                match serde_json::from_str::<HashMap<String, HashMap<i64, String>>>(&contents) {
                    Ok(cached_data) => {
                        let team_count = cached_data.len();
                        let player_count: usize =
                            cached_data.values().map(|roster| roster.len()).sum();
                        let mut data = self.data.write().await;
                        *data = cached_data;
                        info!(
                            "Loaded {team_count} team rosters ({player_count} players) from {}",
                            path.display()
                        );
                    }
                    Err(e) => {
                        error!(
                            "Corrupted player cache at {}, removing and starting fresh: {e}",
                            path.display()
                        );
                        if let Err(remove_err) = tokio::fs::remove_file(&path).await {
                            error!(
                                "Failed to remove corrupted cache file {}: {remove_err}",
                                path.display()
                            );
                        }
                        let mut data = self.data.write().await;
                        data.clear();
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("No player cache file at {}, starting fresh", path.display());
                let mut data = self.data.write().await;
                data.clear();
            }
            Err(e) => {
                error!(
                    "Failed to read player cache at {}: {e} — clearing stale data, will retry on next fetch cycle",
                    path.display()
                );
                let mut data = self.data.write().await;
                data.clear();
            }
        }

        let mut loaded = self.loaded_season.write().await;
        *loaded = Some(season);
        self.dirty_seq.store(0, Ordering::Release);
    }

    /// Writes cached player names to disk if new data has been added since the last save.
    ///
    /// Derives the season from the previously loaded season. No-op if nothing was loaded
    /// or if no new data has been added since the last save.
    pub async fn save_to_disk(&self) {
        let seq = self.dirty_seq.load(Ordering::Acquire);
        if seq == 0 {
            return;
        }

        let season = {
            let loaded = self.loaded_season.read().await;
            match *loaded {
                Some(s) => s,
                None => {
                    warn!(
                        "Cannot save player cache: season unknown (load_from_disk was never called)"
                    );
                    return;
                }
            }
        };

        let path = self.cache_file_path(season);

        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            error!("Failed to create cache directory {}: {e}", parent.display());
            return;
        }

        let (json, team_count) = {
            let data = self.data.read().await;
            let count = data.len();
            match serde_json::to_string_pretty(&*data) {
                Ok(json) => (json, count),
                Err(e) => {
                    error!("Failed to serialize player cache: {e}");
                    return;
                }
            }
        }; // lock dropped before file I/O

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
        // Only clear if no concurrent inserts occurred since we snapshotted
        let _ = self
            .dirty_seq
            .compare_exchange(seq, 0, Ordering::AcqRel, Ordering::Acquire);
        info!("Saved {team_count} team rosters to {}", path.display());
    }

    /// Returns the number of cached team entries.
    #[cfg(test)]
    #[allow(clippy::len_without_is_empty)]
    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }

    /// Returns whether the store has been modified since the last save.
    #[cfg(test)]
    pub fn is_dirty(&self) -> bool {
        self.dirty_seq.load(Ordering::Acquire) != 0
    }

    /// Clears all entries and resets state.
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
        self.dirty_seq.store(0, Ordering::Release);
        let mut loaded = self.loaded_season.write().await;
        *loaded = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_insert_and_get_players() {
        let store = PlayerNameStore::default();
        store
            .insert_team(
                "TPS",
                HashMap::from([(100, "Koivu".to_string()), (200, "Selänne".to_string())]),
            )
            .await;
        store
            .insert_team("HIFK", HashMap::from([(300, "Barkov".to_string())]))
            .await;

        let result = store.get_players(Some("TPS"), Some("HIFK")).await;
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names.len(), 3);
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&300), Some(&"Barkov".to_string()));
    }

    #[tokio::test]
    async fn test_get_returns_none_for_missing_team() {
        let store = PlayerNameStore::default();
        store
            .insert_team("TPS", HashMap::from([(100, "Koivu".to_string())]))
            .await;

        // One team cached, other not → None
        assert!(store.get_players(Some("TPS"), Some("HIFK")).await.is_none());

        // Missing team IDs → None
        assert!(store.get_players(None, Some("TPS")).await.is_none());
        assert!(store.get_players(Some("TPS"), None).await.is_none());
    }

    #[tokio::test]
    async fn test_insert_merges_with_existing() {
        let store = PlayerNameStore::default();
        store
            .insert_team("TPS", HashMap::from([(100, "Koivu".to_string())]))
            .await;
        store
            .insert_team("TPS", HashMap::from([(200, "Selänne".to_string())]))
            .await;

        // Both players should be present under TPS
        store
            .insert_team("HIFK", HashMap::from([(300, "Barkov".to_string())]))
            .await;
        let result = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&200), Some(&"Selänne".to_string()));
    }

    #[tokio::test]
    async fn test_dirty_flag_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        assert!(!store.is_dirty());

        store.load_from_disk(2026).await;
        assert!(!store.is_dirty());

        store
            .insert_team("TPS", HashMap::from([(100, "Koivu".to_string())]))
            .await;
        assert!(store.is_dirty());

        store.save_to_disk().await;
        assert!(!store.is_dirty());
    }

    #[tokio::test]
    async fn test_save_and_load_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let season = 2026;

        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(season).await;

        store
            .insert_team(
                "TPS",
                HashMap::from([(100, "Koivu".to_string()), (200, "Selänne M.".to_string())]),
            )
            .await;
        store
            .insert_team("HIFK", HashMap::from([(300, "Barkov".to_string())]))
            .await;

        store.save_to_disk().await;

        let path = temp_dir.path().join(format!("players_{season}.json"));
        assert!(path.exists());

        let store2 = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store2.load_from_disk(season).await;

        assert_eq!(store2.len().await, 2);
        let result = store2.get_players(Some("TPS"), Some("HIFK")).await;
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&200), Some(&"Selänne M.".to_string()));
        assert_eq!(names.get(&300), Some(&"Barkov".to_string()));
    }

    #[tokio::test]
    async fn test_corrupted_file_handled_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("players_2026.json");
        tokio::fs::write(&path, "not valid json{{{").await.unwrap();

        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 0);

        store
            .insert_team("TPS", HashMap::from([(100, "Test".to_string())]))
            .await;
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn test_missing_file_starts_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 0);

        store
            .insert_team("TPS", HashMap::from([(100, "Test".to_string())]))
            .await;
        assert!(store.get_players(Some("TPS"), Some("TPS")).await.is_some());
    }

    #[tokio::test]
    async fn test_load_idempotent_for_same_season() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        store.load_from_disk(2026).await;

        store
            .insert_team("TPS", HashMap::from([(100, "Test".to_string())]))
            .await;

        // Second load for same season should be a no-op
        store.load_from_disk(2026).await;
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn test_save_noop_when_not_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(2026).await;

        store.save_to_disk().await;
        let path = temp_dir.path().join("players_2026.json");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_season_switch_saves_pending_data() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        // Load season 2025 and insert data
        store.load_from_disk(2025).await;
        store
            .insert_team("TPS", HashMap::from([(100, "Koivu".to_string())]))
            .await;
        assert!(store.is_dirty());

        // Switch to season 2026 — should auto-save 2025 data first
        store.load_from_disk(2026).await;

        // Season 2025 file should exist on disk
        let path_2025 = temp_dir.path().join("players_2025.json");
        assert!(path_2025.exists());

        // Verify the saved data is correct
        let store2 = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store2.load_from_disk(2025).await;
        let result = store2.get_players(Some("TPS"), Some("TPS")).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().get(&100), Some(&"Koivu".to_string()));

        // Current store should now be on season 2026 with empty data
        assert_eq!(store.len().await, 0);
        assert!(!store.is_dirty());
    }

    #[tokio::test]
    async fn test_corrupted_file_removed_on_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("players_2026.json");
        tokio::fs::write(&path, "not valid json{{{").await.unwrap();
        assert!(path.exists());

        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(2026).await;

        // Corrupted file should be removed
        assert!(!path.exists());

        // Store should work normally after recovery
        store
            .insert_team("TPS", HashMap::from([(100, "Test".to_string())]))
            .await;
        store.save_to_disk().await;
        assert!(path.exists());

        // Verify saved data is valid
        let store2 = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store2.load_from_disk(2026).await;
        assert_eq!(store2.len().await, 1);
    }

    #[tokio::test]
    async fn test_serialization_format() {
        let mut data: HashMap<String, HashMap<i64, String>> = HashMap::new();
        data.insert(
            "TPS".to_string(),
            HashMap::from([(100, "Koivu".to_string()), (200, "Selänne".to_string())]),
        );

        let json = serde_json::to_string_pretty(&data).unwrap();
        let deserialized: HashMap<String, HashMap<i64, String>> =
            serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        let roster = deserialized.get("TPS").unwrap();
        assert_eq!(roster.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_all_cache_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();

        // Create fake cache files
        tokio::fs::write(cache_dir.join("players_2025.json"), "{}")
            .await
            .unwrap();
        tokio::fs::write(cache_dir.join("players_2024.json"), "{}")
            .await
            .unwrap();
        tokio::fs::write(cache_dir.join("other_file.txt"), "keep")
            .await
            .unwrap();

        let count = clear_all_cache_files_in(&cache_dir).await;
        assert_eq!(count, 2);

        // Verify player files deleted
        assert!(!cache_dir.join("players_2025.json").exists());
        assert!(!cache_dir.join("players_2024.json").exists());
        // Verify other files untouched
        assert!(cache_dir.join("other_file.txt").exists());
    }

    #[tokio::test]
    async fn test_clear_all_cache_files_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let count = clear_all_cache_files_in(temp_dir.path()).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_clear_all_cache_files_missing_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nonexistent_subdir");
        let count = clear_all_cache_files_in(&path).await;
        assert_eq!(count, 0);
    }
}
