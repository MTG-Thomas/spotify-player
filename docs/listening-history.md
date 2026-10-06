# Listening history import & recommendation setup

This fork adds a CLI workflow that turns a Spotify **account data export** into machine-readable listening history, a taste profile, and the inputs an agent needs to recommend albums. It runs entirely in the CLI (no TUI, no network calls), so it works offline and over large multi-year exports.

## 1. Get the export from Spotify

1. Open Spotify's privacy page: <https://www.spotify.com/account/privacy/>.
2. Under **Download your data**, request **Extended streaming history**.
3. Spotify emails a ZIP when it is ready (can take a few days). Extract it; you get a folder like:

   ```
   Spotify Extended Streaming History/
     ReadMeFirst_ExtendedStreamingHistory.pdf
     Streaming_History_Audio_2015.json
     ...
     Streaming_History_Audio_2026.json
     Streaming_History_Video_2017.json
     ...
   ```

The `Audio` files are the ones you want; `Video`/podcast rows are skipped automatically.

Notes:

- The export is **split by year** and covers your full account history (audio can reach back to account creation).
- The older **basic** export (`StreamingHistory_music_*.json`, using `endTime`/`artistName`/`trackName`/`msPlayed`) is also supported.
- The Web API endpoint `/me/player/recently-played` only returns the last ~50 plays, so the account export is the only multi-year source.

## 2. Run the importer

Build the CLI (or use a release binary), then point `import history` at the folder (or at individual files):

```sh
spotify_player import history "/path/to/Spotify Extended Streaming History"
```

It writes JSON to stdout.

| Command | Output |
| --- | --- |
| `import history <paths>...` | Play **events**, one per play (recently-played shape) |
| `import history --aggregate <paths>...` | Per-artist and per-album roll-ups, total time, period |
| `import history --taste-profile <paths>...` | Overall + last-90d/365d + per-year top artists/albums |
| `import history --min-ms 30000 <paths>...` | Drop plays shorter than 30 s before output |
| `import history --taste-profile --top 100 <paths>...` | 100 entries per list (default 50) |

`--aggregate` and `--taste-profile` are mutually exclusive. `<paths>` accepts files or directories; directories are scanned (non-recursively) for `*.json`.

Redirect to files:

```sh
mkdir -p out
spotify_player import history --aggregate   "$EXPORT" > out/aggregate_all.json
spotify_player import history --taste-profile "$EXPORT" > out/taste_profile.json
spotify_player import history               "$EXPORT" > out/events_all.json
```

### Output schemas

**Event** (`ImportedPlay`):

```json
{
  "played_at": "2026-10-03T21:31:33+00:00",
  "ms_played": 195212,
  "skipped": false,
  "track": {
    "id": "4L7kaptgY6py2G3nRW9BVQ",
    "name": "Masseduction",
    "artists": [{ "name": "St. Vincent" }],
    "album": { "name": "MASSEDUCTION" }
  }
}
```

**Taste profile** (`TasteProfile`):

```json
{
  "latest_play": "2026-10-03T21:31:33+00:00",
  "overall": {
    "plays": 66736,
    "ms_played": 12562560000,
    "top_artists": [{ "name": "Run The Jewels", "plays": 2782, "ms_played": 556000000 }],
    "top_albums": [{ "name": "Coloring Book", "artist": "Chance the Rapper", "plays": 1364, "ms_played": 289000000 }]
  },
  "windows": { "last_90d": { "…": "…" }, "last_365d": { "…": "…" } },
  "by_year": { "2015": { "…": "…" } }
}
```

Each `TasteBlock` (`overall`, each window, each year) has `plays`, `ms_played`, `top_artists`, and `top_albums`. `--aggregate` instead returns `total_plays`, `total_ms_played`, `period_start`, `period_end`, `artists`, and `albums`.

## 3. Recommendation engine workflow

The importer produces the data; the "engine" is the loop an agent runs over it:

1. **Build the profile** with `import history --taste-profile`. Use `overall` for identity, `windows.last_90d` / `windows.last_365d` for what the listener is into *now*, and `by_year` for drift.
2. **Enumerate what is already played** so candidates can be filtered:

   ```sh
   spotify_player import history --aggregate "$EXPORT" > out/aggregate_all.json
   python contrib/listening-history/played_sets.py out/aggregate_all.json out/
   ```

   This writes `out/played_albums.txt` and `out/played_artists.txt`.
3. **Collect candidates** — new releases by the listener's top artists plus adjacent acts. Research the current year; a model's training data is usually stale.
4. **Filter and rank** candidates against the played set:

   ```sh
   python contrib/listening-history/check_candidates.py out/played_albums.txt candidates.tsv
   ```

   `candidates.tsv` is one `album<TAB>artist` per line.
5. **Report** a ranked shortlist with a reason per pick, weighted to `last_90d` / `last_365d`.

### Pitfalls

- **Match on normalized names.** Comparisons are lowercase; watch trailing punctuation (`"this music may contain hope."` vs `"this music may contain hope"`) and apostrophe/Unicode variants (`man's best friend`).
- **Album-level matching misses singles.** If only singles from an album were played, the album may not appear in the played set.
- **Export coverage.** The export ends at the last play; releases after that point are not filtered and may not be reflected in the profile.
- **Video/podcast rows are skipped** (no `master_metadata_track_name`), so counts are music-only.
- **`genres` is deprecated** by Spotify and frequently empty; do not rely on it for genre-based ranking.

## 4. Helper scripts

See [`contrib/listening-history/`](../contrib/listening-history/) for the small stdlib-only Python helpers used above (`played_sets.py`, `check_candidates.py`, `report.py`).
