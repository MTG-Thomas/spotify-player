#!/usr/bin/env python3
"""Pretty-print a taste profile produced by `import history --taste-profile`.

Usage:
    python report.py taste_profile.json [--top N]

Prints the overall, last-90d, and last-365d blocks, then a one-line summary per
year. `--top` controls how many artists/albums are shown per list (default 10).
"""
import json
import sys


def hours(ms: int) -> float:
    return ms / 3_600_000


def block(title: str, data: dict, top: int) -> None:
    print(f"\n{title}: {data['plays']:,} plays, {hours(data['ms_played']):,.1f} h")
    print("  top artists:")
    for a in data["top_artists"][:top]:
        print(f"    {hours(a['ms_played']):>8.1f} h  {a['plays']:>7,}  {a['name']}")
    print("  top albums:")
    for a in data["top_albums"][:top]:
        print(
            f"    {hours(a['ms_played']):>8.1f} h  {a['plays']:>7,}  "
            f"{a['name']}  —  {a['artist']}"
        )


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    top = 10
    if "--top" in sys.argv:
        top = int(sys.argv[sys.argv.index("--top") + 1])

    with open(sys.argv[1], encoding="utf-8-sig") as f:
        profile = json.load(f)

    print(f"latest play: {profile.get('latest_play')}")
    block("overall", profile["overall"], top)
    block("last 90d", profile["windows"]["last_90d"], top)
    block("last 365d", profile["windows"]["last_365d"], top)

    for year in sorted(profile.get("by_year", {})):
        data = profile["by_year"][year]
        top_artist = data["top_artists"][0]["name"] if data["top_artists"] else "-"
        print(
            f"\n{year}: {data['plays']:,} plays, "
            f"{hours(data['ms_played']):,.1f} h — top: {top_artist}"
        )


if __name__ == "__main__":
    main()
