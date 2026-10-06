#!/usr/bin/env python3
"""Write played album/artist sets from an `import history --aggregate` dump.

Usage:
    python played_sets.py aggregate_all.json OUTDIR

Writes OUTDIR/played_albums.txt ("album ||| artist" per line, lowercased) and
OUTDIR/played_artists.txt (one artist per line, lowercased). Use these to filter
recommendation candidates down to albums the listener has not played.
"""
import json
import os
import sys


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src, outdir = sys.argv[1], sys.argv[2]
    os.makedirs(outdir, exist_ok=True)

    with open(src, encoding="utf-8-sig") as f:
        aggregate = json.load(f)

    albums = sorted(
        {
            (a["name"].strip().lower(), a["artist"].strip().lower())
            for a in aggregate["albums"]
        }
    )
    with open(os.path.join(outdir, "played_albums.txt"), "w", encoding="utf-8") as f:
        for name, artist in albums:
            f.write(f"{name} ||| {artist}\n")

    artists = sorted({a["name"].strip().lower() for a in aggregate["artists"]})
    with open(os.path.join(outdir, "played_artists.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(artists) + "\n")

    print(f"wrote {len(albums)} albums and {len(artists)} artists to {outdir}")


if __name__ == "__main__":
    main()
