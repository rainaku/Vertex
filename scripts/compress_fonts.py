"""Losslessly convert bundled OTF fonts to WOFF2; requires fonttools and brotli.

python scripts/compress_fonts.py frontend/public/fonts
Originals are retained by this script. Remove them from public/ only after checking
the generated files, so Vite does not bundle both representations.
"""
import sys
from pathlib import Path

from fontTools.ttLib import TTFont


def compress(source: Path) -> None:
    font = TTFont(source, recalcTimestamp=False)
    target = source.with_suffix(".woff2")
    font.flavor = "woff2"
    font.save(target)
    restored = TTFont(target, recalcTimestamp=False)
    # WOFF2 marks its lossless compression in head.flags bit 11.
    restored["head"].flags = font["head"].flags
    assert font.getGlyphOrder() == restored.getGlyphOrder(), source
    assert font.getBestCmap() == restored.getBestCmap(), source
    # SF Pro uses CFF outlines, so WOFF2 need not transform any glyph table.
    for tag in font.keys():
        if tag != "GlyphOrder":
            before, after = font.getTableData(tag), restored.getTableData(tag)
            if tag == "head":
                # Container offsets change the global SFNT checksum, not glyphs.
                before = before[:8] + b"\0" * 4 + before[12:]
                after = after[:8] + b"\0" * 4 + after[12:]
            assert before == after, (source, tag)
    print(f"{source.name}: {source.stat().st_size} -> {target.stat().st_size} bytes; all tables identical")


if __name__ == "__main__":
    for source in sorted(Path(sys.argv[1]).glob("*.otf")):
        compress(source)
