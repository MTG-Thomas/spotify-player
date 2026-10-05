use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// A single play parsed from a Spotify account-export file.
///
/// Mirrors the shape of the Web API's recently-played data (`played_at` plus a
/// track), but IDs are optional because the account export often omits them.
#[derive(Debug, Serialize)]
pub struct ImportedPlay {
    pub played_at: Option<String>,
    pub ms_played: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<bool>,
    pub track: ImportedTrack,
}

#[derive(Debug, Serialize)]
pub struct ImportedTrack {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub artists: Vec<ImportedArtist>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<ImportedAlbum>,
}

#[derive(Debug, Serialize)]
pub struct ImportedArtist {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ImportedAlbum {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
}

/// Per-artist roll-up.
#[derive(Debug, Serialize)]
pub struct ArtistAggregate {
    pub name: String,
    pub plays: u64,
    pub ms_played: u64,
}

/// Per-album roll-up.
#[derive(Debug, Serialize)]
pub struct AlbumAggregate {
    pub name: String,
    pub artist: String,
    pub plays: u64,
    pub ms_played: u64,
}

/// Aggregate listening history, sorted by listened time.
#[derive(Debug, Serialize)]
pub struct HistoryAggregate {
    pub total_plays: u64,
    pub total_ms_played: u64,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub artists: Vec<ArtistAggregate>,
    pub albums: Vec<AlbumAggregate>,
}

/// Loads and normalizes listening history from export files or directories.
///
/// Each path may be a JSON file or a directory containing JSON files (the
/// account export ships several `Streaming*.json` files). Entries that are not
/// music (podcast/video history) are skipped.
pub fn load_plays(paths: &[PathBuf]) -> Result<Vec<ImportedPlay>> {
    let mut plays = Vec::new();
    for file in collect_json_files(paths)? {
        plays.extend(parse_file(&file)?);
    }
    Ok(plays)
}

/// Drops plays shorter than `min_ms` milliseconds.
pub fn filter_min_ms(plays: Vec<ImportedPlay>, min_ms: u64) -> Vec<ImportedPlay> {
    if min_ms == 0 {
        return plays;
    }
    plays
        .into_iter()
        .filter(|p| p.ms_played >= min_ms)
        .collect()
}

/// Rolls up plays by artist and album, sorted by total listened time.
pub fn aggregate(plays: &[ImportedPlay]) -> HistoryAggregate {
    let mut artist_map: HashMap<String, (u64, u64)> = HashMap::new();
    let mut album_map: HashMap<(String, String), (u64, u64)> = HashMap::new();
    let mut total_ms_played = 0u64;
    let mut period_start: Option<String> = None;
    let mut period_end: Option<String> = None;

    for play in plays {
        total_ms_played += play.ms_played;

        for artist in &play.track.artists {
            let entry = artist_map.entry(artist.name.clone()).or_default();
            entry.0 += 1;
            entry.1 += play.ms_played;
        }

        if let Some(album) = &play.track.album {
            let artist = play
                .track
                .artists
                .first()
                .map_or_else(String::new, |a| a.name.clone());
            let entry = album_map.entry((album.name.clone(), artist)).or_default();
            entry.0 += 1;
            entry.1 += play.ms_played;
        }

        if let Some(ts) = &play.played_at {
            if period_start.as_ref().is_none_or(|start| ts < start) {
                period_start = Some(ts.clone());
            }
            if period_end.as_ref().is_none_or(|end| ts > end) {
                period_end = Some(ts.clone());
            }
        }
    }

    let mut artists: Vec<ArtistAggregate> = artist_map
        .into_iter()
        .map(|(name, (plays, ms_played))| ArtistAggregate {
            name,
            plays,
            ms_played,
        })
        .collect();
    artists.sort_by(|a, b| b.ms_played.cmp(&a.ms_played).then(b.plays.cmp(&a.plays)));

    let mut albums: Vec<AlbumAggregate> = album_map
        .into_iter()
        .map(|((name, artist), (plays, ms_played))| AlbumAggregate {
            name,
            artist,
            plays,
            ms_played,
        })
        .collect();
    albums.sort_by(|a, b| b.ms_played.cmp(&a.ms_played).then(b.plays.cmp(&a.plays)));

    HistoryAggregate {
        total_plays: plays.len() as u64,
        total_ms_played,
        period_start,
        period_end,
        artists,
        albums,
    }
}

fn collect_json_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        let metadata =
            fs::metadata(path).with_context(|| format!("read metadata for {}", path.display()))?;
        if metadata.is_dir() {
            for entry in
                fs::read_dir(path).with_context(|| format!("read directory {}", path.display()))?
            {
                let candidate = entry?.path();
                if is_json(&candidate) {
                    files.push(candidate);
                }
            }
        } else {
            files.push(path.clone());
        }
    }
    files.sort();
    Ok(files)
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

fn parse_file(path: &Path) -> Result<Vec<ImportedPlay>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read history file {}", path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("parse JSON from {}", path.display()))?;
    let entries = value
        .as_array()
        .with_context(|| format!("history file {} is not a JSON array", path.display()))?;
    Ok(parse_entries(entries))
}

fn parse_entries(entries: &[Value]) -> Vec<ImportedPlay> {
    entries.iter().filter_map(parse_entry).collect()
}

fn parse_entry(entry: &Value) -> Option<ImportedPlay> {
    // Extended streaming history uses these fields; the basic history uses `endTime`/`artistName`.
    if entry.get("master_metadata_track_name").is_some() || entry.get("spotify_track_uri").is_some()
    {
        parse_extended(entry)
    } else if entry.get("endTime").is_some() || entry.get("artistName").is_some() {
        parse_basic(entry)
    } else {
        None
    }
}

fn parse_extended(entry: &Value) -> Option<ImportedPlay> {
    // Non-music entries (podcasts, videos) have no track name.
    let name = entry
        .get("master_metadata_track_name")?
        .as_str()?
        .to_string();
    let artist = entry
        .get("master_metadata_album_artist_name")
        .and_then(Value::as_str)
        .unwrap_or("Unknown artist")
        .to_string();
    let album = entry
        .get("master_metadata_album_album_name")
        .and_then(Value::as_str)
        .map(str::to_string);

    Some(ImportedPlay {
        played_at: entry
            .get("ts")
            .and_then(Value::as_str)
            .map(normalize_timestamp),
        ms_played: entry.get("ms_played").and_then(Value::as_u64).unwrap_or(0),
        skipped: entry.get("skipped").and_then(Value::as_bool),
        track: ImportedTrack {
            id: entry
                .get("spotify_track_uri")
                .and_then(Value::as_str)
                .and_then(strip_track_uri),
            name,
            artists: vec![ImportedArtist {
                id: None,
                name: artist,
            }],
            album: album.map(|name| ImportedAlbum { id: None, name }),
        },
    })
}

fn parse_basic(entry: &Value) -> Option<ImportedPlay> {
    let name = entry.get("trackName")?.as_str()?.to_string();
    let artist = entry
        .get("artistName")
        .and_then(Value::as_str)
        .unwrap_or("Unknown artist")
        .to_string();

    Some(ImportedPlay {
        played_at: entry
            .get("endTime")
            .and_then(Value::as_str)
            .map(normalize_timestamp),
        ms_played: entry.get("msPlayed").and_then(Value::as_u64).unwrap_or(0),
        skipped: None,
        track: ImportedTrack {
            id: None,
            name,
            artists: vec![ImportedArtist {
                id: None,
                name: artist,
            }],
            album: None,
        },
    })
}

fn strip_track_uri(uri: &str) -> Option<String> {
    uri.strip_prefix("spotify:track:")
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// Normalizes export timestamps (`YYYY-MM-DD HH:MM` or RFC 3339) to RFC 3339 UTC.
///
/// The basic export's naive local timestamps are treated as UTC, since the
/// export carries no offset.
fn normalize_timestamp(raw: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return dt.with_timezone(&chrono::Utc).to_rfc3339();
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M") {
        return naive.and_utc().to_rfc3339();
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entries(value: serde_json::Value) -> Vec<ImportedPlay> {
        parse_entries(value.as_array().expect("test input must be a JSON array"))
    }

    #[test]
    fn parses_basic_history() {
        let plays = entries(json!([
            {"endTime": "2023-05-01 14:30", "artistName": "The Dare", "trackName": "LCA", "msPlayed": 200000}
        ]));

        assert_eq!(plays.len(), 1);
        assert_eq!(plays[0].track.name, "LCA");
        assert_eq!(plays[0].track.artists[0].name, "The Dare");
        assert!(plays[0].track.id.is_none());
        assert!(plays[0].track.album.is_none());
        assert_eq!(plays[0].ms_played, 200_000);
        assert_eq!(
            plays[0].played_at.as_deref(),
            Some("2023-05-01T14:30:00+00:00")
        );
    }

    #[test]
    fn filters_plays_shorter_than_min_ms() {
        let plays = entries(json!([
            {"endTime": "2023-05-01 14:30", "artistName": "A", "trackName": "skip", "msPlayed": 0},
            {"endTime": "2023-05-02 14:30", "artistName": "A", "trackName": "play", "msPlayed": 60000}
        ]));

        let filtered = filter_min_ms(plays, 30_000);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].track.name, "play");
    }

    #[test]
    fn parses_extended_history_and_skips_non_music() {
        let plays = entries(json!([
            {
                "ts": "2024-01-02T03:04:05Z",
                "ms_played": 180000,
                "master_metadata_track_name": "Cheeky",
                "master_metadata_album_artist_name": "The Dare",
                "master_metadata_album_album_name": "What's Wrong With New York?",
                "spotify_track_uri": "spotify:track:4uLU6hMCjMI75M1A2tKUQC",
                "skipped": false
            },
            {
                "ts": "2024-01-02T03:10:00Z",
                "ms_played": 60000,
                "master_metadata_track_name": null,
                "episode_name": "Some Podcast",
                "spotify_episode_uri": "spotify:episode:abc"
            }
        ]));

        assert_eq!(plays.len(), 1, "non-music entries should be skipped");
        assert_eq!(plays[0].track.id.as_deref(), Some("4uLU6hMCjMI75M1A2tKUQC"));
        assert_eq!(
            plays[0].track.album.as_ref().map(|a| a.name.as_str()),
            Some("What's Wrong With New York?")
        );
        assert_eq!(plays[0].skipped, Some(false));
        assert_eq!(
            plays[0].played_at.as_deref(),
            Some("2024-01-02T03:04:05+00:00")
        );
    }

    #[test]
    fn aggregates_artists_and_period() {
        let plays = entries(json!([
            {"endTime": "2023-05-01 14:30", "artistName": "A", "trackName": "T1", "msPlayed": 1000},
            {"endTime": "2023-05-02 14:30", "artistName": "A", "trackName": "T2", "msPlayed": 2000},
            {"endTime": "2023-05-03 14:30", "artistName": "B", "trackName": "T3", "msPlayed": 500}
        ]));

        let agg = aggregate(&plays);

        assert_eq!(agg.total_plays, 3);
        assert_eq!(agg.total_ms_played, 3500);
        assert_eq!(agg.artists[0].name, "A");
        assert_eq!(agg.artists[0].plays, 2);
        assert_eq!(agg.artists[0].ms_played, 3000);
        assert_eq!(
            agg.period_start.as_deref(),
            Some("2023-05-01T14:30:00+00:00")
        );
        assert_eq!(agg.period_end.as_deref(), Some("2023-05-03T14:30:00+00:00"));
        assert!(agg.albums.is_empty(), "basic history has no album data");
    }

    #[test]
    fn aggregates_albums_from_extended_history() {
        let plays = entries(json!([
            {"ts": "2024-01-02T03:04:05Z", "ms_played": 1000, "master_metadata_track_name": "T1", "master_metadata_album_artist_name": "A", "master_metadata_album_album_name": "Album", "spotify_track_uri": "spotify:track:t1"},
            {"ts": "2024-01-02T03:05:05Z", "ms_played": 1000, "master_metadata_track_name": "T2", "master_metadata_album_artist_name": "A", "master_metadata_album_album_name": "Album", "spotify_track_uri": "spotify:track:t2"}
        ]));

        let agg = aggregate(&plays);

        assert_eq!(agg.albums.len(), 1);
        assert_eq!(agg.albums[0].name, "Album");
        assert_eq!(agg.albums[0].artist, "A");
        assert_eq!(agg.albums[0].plays, 2);
        assert_eq!(agg.albums[0].ms_played, 2000);
    }
}
