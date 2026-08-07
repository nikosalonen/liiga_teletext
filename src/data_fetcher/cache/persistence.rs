use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::paths::get_cache_dir_path;
use crate::data_fetcher::player_names::format_with_disambiguation;

/// On-disk format version. Bump whenever the shape of the cache file changes;
/// files carrying any other version are discarded and rebuilt from the API.
const CACHE_FORMAT_VERSION: u32 = 2;

/// A player's name as the API reports it, stored unformatted.
///
/// Disambiguation is deliberately *not* baked in here: it depends on which
/// other players share the surname, and that set grows as the season goes on.
/// Storing raw names lets [`PlayerNameStore::get_players`] recompute display
/// names against the current roster every time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerName {
    pub first: String,
    pub last: String,
}

impl PlayerName {
    pub fn new(first: impl Into<String>, last: impl Into<String>) -> Self {
        Self {
            first: first.into(),
            last: last.into(),
        }
    }
}

/// Versioned envelope written to disk.
#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    teams: HashMap<String, HashMap<i64, PlayerName>>,
}

/// Result of interpreting the bytes of a cache file.
enum DecodedCache {
    /// A cache file of the current version.
    Current(HashMap<String, HashMap<i64, PlayerName>>),
    /// A readable file from an older format — expected after an upgrade, not an error.
    Outdated,
    /// Not parseable as any known format.
    Corrupted(serde_json::Error),
}

/// Converts a stored roster into the `(id, first, last)` shape the
/// disambiguation functions expect.
fn to_tuples(roster: &HashMap<i64, PlayerName>) -> Vec<(i64, String, String)> {
    roster
        .iter()
        .map(|(id, name)| (*id, name.first.clone(), name.last.clone()))
        .collect()
}

/// Interprets cache-file contents, distinguishing a routine format upgrade from
/// genuine corruption so the two can be logged at appropriate levels.
fn decode_cache(contents: &str) -> DecodedCache {
    match serde_json::from_str::<CacheFile>(contents) {
        Ok(file) if file.version == CACHE_FORMAT_VERSION => DecodedCache::Current(file.teams),
        Ok(_) => DecodedCache::Outdated,
        Err(e) => {
            // Version 1 stored `{team_id: {player_id: "display name"}}` with no envelope.
            if serde_json::from_str::<HashMap<String, HashMap<i64, String>>>(contents).is_ok() {
                DecodedCache::Outdated
            } else {
                DecodedCache::Corrupted(e)
            }
        }
    }
}

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

/// Persistent store for player names, keyed by team.
///
/// Stores a flat `team_id → (player_id → raw name)` map per season, backed by a
/// JSON file. Each player is stored exactly once under their team. Display
/// names are derived on read so that every game — live or finished — resolves
/// scorers against the same team-scoped roster.
pub struct PlayerNameStore {
    data: RwLock<HashMap<String, HashMap<i64, PlayerName>>>,
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

    /// Removes an unusable cache file and resets the in-memory store.
    async fn discard_cache_file(&self, path: &std::path::Path) {
        if let Err(e) = tokio::fs::remove_file(path).await {
            error!("Failed to remove cache file {}: {e}", path.display());
        }
        let mut data = self.data.write().await;
        data.clear();
    }

    /// Returns merged display names for both teams if both rosters are cached.
    ///
    /// Disambiguation is applied per team at read time, so a player's display
    /// name always reflects every teammate currently known to share their
    /// surname — not just the ones present in whichever game first cached them.
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

        // Disambiguate each team separately: players on opposing teams never
        // affect each other's display names.
        let mut merged = format_with_disambiguation(&to_tuples(home));
        merged.extend(format_with_disambiguation(&to_tuples(away)));
        debug!(
            "Player name store hit for {home_id} vs {away_id} ({} players)",
            merged.len()
        );
        Some(merged)
    }

    /// Inserts a team's roster into the store.
    ///
    /// Merges with any existing entries for the team, so new players from later
    /// games are accumulated. The dirty counter only advances when something
    /// actually changed, keeping live-game refreshes from rewriting the file.
    pub async fn insert_team(&self, team_id: &str, players: HashMap<i64, PlayerName>) {
        if players.is_empty() {
            // Never create an empty team entry: `get_players` treats a present
            // key as "roster known", and an empty one would suppress the fetch.
            return;
        }

        let player_count = players.len();
        let mut data = self.data.write().await;
        let entry = data.entry(team_id.to_string()).or_default();

        let mut added = 0;
        for (id, name) in players {
            if entry.get(&id) != Some(&name) {
                entry.insert(id, name);
                added += 1;
            }
        }

        if added > 0 {
            self.dirty_seq.fetch_add(1, Ordering::AcqRel);
            debug!(
                "Player name store: {added} new/changed of {player_count} players for team {team_id}"
            );
        }
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
            Ok(contents) => match decode_cache(&contents) {
                DecodedCache::Current(cached_data) => {
                    let team_count = cached_data.len();
                    let player_count: usize = cached_data.values().map(|roster| roster.len()).sum();
                    let mut data = self.data.write().await;
                    *data = cached_data;
                    info!(
                        "Loaded {team_count} team rosters ({player_count} players) from {}",
                        path.display()
                    );
                }
                DecodedCache::Outdated => {
                    info!(
                        "Player cache at {} predates format v{CACHE_FORMAT_VERSION}, rebuilding from the API",
                        path.display()
                    );
                    self.discard_cache_file(&path).await;
                }
                DecodedCache::Corrupted(e) => {
                    error!(
                        "Corrupted player cache at {}, removing and starting fresh: {e}",
                        path.display()
                    );
                    self.discard_cache_file(&path).await;
                }
            },
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
            let file = CacheFile {
                version: CACHE_FORMAT_VERSION,
                teams: data.clone(),
            };
            match serde_json::to_string_pretty(&file) {
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

    /// Builds a roster map from `(id, first, last)` triples.
    fn roster(players: &[(i64, &str, &str)]) -> HashMap<i64, PlayerName> {
        players
            .iter()
            .map(|(id, first, last)| (*id, PlayerName::new(*first, *last)))
            .collect()
    }

    #[tokio::test]
    async fn test_insert_and_get_players() {
        let store = PlayerNameStore::default();
        store
            .insert_team(
                "TPS",
                roster(&[(100, "Mikko", "Koivu"), (200, "Teemu", "Selänne")]),
            )
            .await;
        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
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
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
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
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
            .await;
        store
            .insert_team("TPS", roster(&[(200, "Teemu", "Selänne")]))
            .await;

        // Both players should be present under TPS
        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
            .await;
        let result = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(result.get(&200), Some(&"Selänne".to_string()));
    }

    #[tokio::test]
    async fn test_empty_roster_does_not_create_team_entry() {
        let store = PlayerNameStore::default();
        store.insert_team("TPS", HashMap::new()).await;
        assert!(!store.is_dirty(), "an empty roster is not new data");

        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
            .await;

        // TPS must still read as "not cached" so a roster fetch is attempted.
        assert!(store.get_players(Some("TPS"), Some("HIFK")).await.is_none());
    }

    #[tokio::test]
    async fn test_reinserting_identical_roster_leaves_store_clean() {
        let store = PlayerNameStore::default();
        let tps = roster(&[(100, "Mikko", "Koivu")]);

        store.insert_team("TPS", tps.clone()).await;
        assert!(store.is_dirty());

        store.dirty_seq.store(0, Ordering::Release);
        store.insert_team("TPS", tps).await;
        assert!(
            !store.is_dirty(),
            "re-inserting an unchanged roster must not schedule a disk write"
        );
    }

    // --- read-time disambiguation ---

    #[tokio::test]
    async fn test_display_names_are_disambiguated_on_read() {
        let store = PlayerNameStore::default();
        store
            .insert_team(
                "TPS",
                roster(&[(100, "Mikko", "Koivu"), (101, "Saku", "Koivu")]),
            )
            .await;
        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
            .await;

        let names = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu M.".to_string()));
        assert_eq!(names.get(&101), Some(&"Koivu S.".to_string()));
    }

    #[tokio::test]
    async fn test_later_teammate_upgrades_earlier_players_display_name() {
        let store = PlayerNameStore::default();
        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
            .await;

        // First game: only one Koivu dressed, so the surname alone is unambiguous.
        store
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
            .await;
        let names = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));

        // A later game adds a second Koivu — the first one must gain an initial
        // rather than keeping the display name frozen from the earlier game.
        store
            .insert_team("TPS", roster(&[(101, "Saku", "Koivu")]))
            .await;
        let names = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu M.".to_string()));
        assert_eq!(names.get(&101), Some(&"Koivu S.".to_string()));
    }

    #[tokio::test]
    async fn test_disambiguation_is_team_scoped() {
        let store = PlayerNameStore::default();
        store
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
            .await;
        store
            .insert_team("HIFK", roster(&[(200, "Saku", "Koivu")]))
            .await;

        // Same surname on opposing teams must not trigger disambiguation.
        let names = store.get_players(Some("TPS"), Some("HIFK")).await.unwrap();
        assert_eq!(names.get(&100), Some(&"Koivu".to_string()));
        assert_eq!(names.get(&200), Some(&"Koivu".to_string()));
    }

    // --- on-disk format handling ---

    #[tokio::test]
    async fn test_legacy_format_file_is_discarded() {
        let temp_dir = TempDir::new().unwrap();
        let season = 2026;
        let path = temp_dir.path().join(format!("players_{season}.json"));

        // Version 1 layout: pre-formatted display names, no envelope.
        tokio::fs::write(&path, r#"{"TPS":{"100":"Koivu M."}}"#)
            .await
            .unwrap();

        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        store.load_from_disk(season).await;

        assert_eq!(store.len().await, 0);
        assert!(!path.exists(), "outdated cache file should be removed");
    }

    #[test]
    fn test_decode_cache_classifies_contents() {
        assert!(matches!(
            decode_cache(r#"{"version":2,"teams":{}}"#),
            DecodedCache::Current(_)
        ));
        assert!(matches!(
            decode_cache(r#"{"version":99,"teams":{}}"#),
            DecodedCache::Outdated
        ));
        assert!(matches!(
            decode_cache(r#"{"TPS":{"100":"Koivu"}}"#),
            DecodedCache::Outdated
        ));
        assert!(matches!(
            decode_cache("garbage"),
            DecodedCache::Corrupted(_)
        ));
    }

    #[tokio::test]
    async fn test_dirty_flag_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());
        assert!(!store.is_dirty());

        store.load_from_disk(2026).await;
        assert!(!store.is_dirty());

        store
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
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
                roster(&[(100, "Mikko", "Koivu"), (200, "Teemu", "Selänne")]),
            )
            .await;
        store
            .insert_team("HIFK", roster(&[(300, "Aleksander", "Barkov")]))
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
        assert_eq!(names.get(&200), Some(&"Selänne".to_string()));
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
            .insert_team("TPS", roster(&[(100, "Testi", "Pelaaja")]))
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
            .insert_team("TPS", roster(&[(100, "Testi", "Pelaaja")]))
            .await;
        assert!(store.get_players(Some("TPS"), Some("TPS")).await.is_some());
    }

    #[tokio::test]
    async fn test_load_idempotent_for_same_season() {
        let temp_dir = TempDir::new().unwrap();
        let store = PlayerNameStore::with_base_path(temp_dir.path().to_path_buf());

        store.load_from_disk(2026).await;

        store
            .insert_team("TPS", roster(&[(100, "Testi", "Pelaaja")]))
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
            .insert_team("TPS", roster(&[(100, "Mikko", "Koivu")]))
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
            .insert_team("TPS", roster(&[(100, "Testi", "Pelaaja")]))
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
        let file = CacheFile {
            version: CACHE_FORMAT_VERSION,
            teams: HashMap::from([(
                "TPS".to_string(),
                roster(&[(100, "Mikko", "Koivu"), (200, "Teemu", "Selänne")]),
            )]),
        };

        let json = serde_json::to_string_pretty(&file).unwrap();
        match decode_cache(&json) {
            DecodedCache::Current(teams) => {
                assert_eq!(teams.len(), 1);
                let tps = teams.get("TPS").unwrap();
                assert_eq!(tps.len(), 2);
                assert_eq!(tps.get(&100), Some(&PlayerName::new("Mikko", "Koivu")));
            }
            _ => panic!("round-tripped cache file should decode as current"),
        }
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
