# listening-history helpers

Small, dependency-free Python helpers for the workflow in
[`docs/listening-history.md`](../../docs/listening-history.md). The heavy lifting
(parsing, aggregation, taste profile) is done by the CLI:

```sh
spotify_player import history --aggregate     "$EXPORT" > out/aggregate_all.json
spotify_player import history --taste-profile "$EXPORT" > out/taste_profile.json
spotify_player import history                 "$EXPORT" > out/events_all.json
```

Then:

| Script | Purpose |
| --- | --- |
| `report.py taste_profile.json [--top N]` | Pretty-print the profile (overall / last 90d / last 365d / per-year) |
| `played_sets.py aggregate_all.json OUTDIR` | Write `played_albums.txt` + `played_artists.txt` for de-duplication |
| `check_candidates.py played_albums.txt candidates.tsv` | Mark each `album<TAB>artist` candidate `NEW` or `PLAYED` |

Typical recommendation pass:

```sh
python contrib/listening-history/report.py out/taste_profile.json --top 15
python contrib/listening-history/played_sets.py out/aggregate_all.json out/
python contrib/listening-history/check_candidates.py out/played_albums.txt candidates.tsv
```

Requires Python 3.9+; no third-party packages.
