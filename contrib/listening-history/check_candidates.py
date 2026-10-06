#!/usr/bin/env python3
"""Check candidate albums against a played-albums set.

Usage:
    python check_candidates.py played_albums.txt candidates.tsv

`candidates.tsv` has one "album<TAB>artist" per line (blank lines and lines
starting with `#` are ignored). Prints one line per candidate:

    NEW      the album is not in the listening history (a valid recommendation)
    PLAYED   the album is already in the listening history

Matching is case-insensitive on the trimmed album and artist names.
"""
import sys


def load_played(path: str) -> set:
    played = set()
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if " ||| " in line:
                name, artist = line.split(" ||| ", 1)
                played.add((name.strip().lower(), artist.strip().lower()))
    return played


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    played = load_played(sys.argv[1])

    rows = []
    with open(sys.argv[2], encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) < 2:
                continue
            album, artist = parts[0].strip(), parts[1].strip()
            status = "PLAYED" if (album.lower(), artist.lower()) in played else "NEW"
            rows.append((status, album, artist))

    width = max((len(album) for _, album, _ in rows), default=0)
    for status, album, artist in rows:
        print(f"{status:<7} {album:<{width}}  —  {artist}")

    new = sum(1 for status, _, _ in rows if status == "NEW")
    print(f"\n{new}/{len(rows)} candidates are new")


if __name__ == "__main__":
    main()
