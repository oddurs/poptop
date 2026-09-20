#!/usr/bin/env python3
"""Render a real poptop frame as an SVG.

Not a drawing and not a photograph. This runs the release binary under a
pseudo-terminal, feeds it keystrokes, lets a terminal emulator interpret
everything it wrote, and then emits the resulting grid of cells as SVG text
with the colours poptop actually asked for. Regenerating it after a change
shows the change.

The reason it exists: poptop's palette is measured — `--check-theme` reports
the separation between every pair of meaning-bearing hues under simulated
protanopia, deuteranopia and tritanopia, and the default replaces green with
cyan because green and yellow separate by only dE 3.7 for roughly 8% of men.
Every frame in the README is plain text, so none of that is visible to anyone
who has not run the program.

Usage:
    python3 tools/screenshot.py docs/media/poptop.svg

Needs `pyte` (pip install pyte) and a release build. Nothing in the test
suite or in CI depends on this; it is run by hand when the screen changes.
"""

import fcntl
import json
import os
import pty
import select
import struct
import sys
import termios
import time
import xml.sax.saxutils as sax

import pyte

# The sixteen ANSI slots are the terminal's to define, which is the same
# reason `--check-theme` reports INCOMPLETE for a theme written with colour
# names. An SVG has no terminal behind it, so one palette has to be chosen and
# stated: this is xterm's, which is what most terminals ship or approximate.
ANSI = {
    "black": "#000000", "red": "#cd0000", "green": "#00cd00",
    "brown": "#cdcd00", "yellow": "#cdcd00", "blue": "#0000ee",
    "magenta": "#cd00cd", "cyan": "#00cdcd", "white": "#e5e5e5",
    "brightblack": "#7f7f7f", "brightred": "#ff0000",
    "brightgreen": "#00ff00", "brightbrown": "#ffff00",
    "brightyellow": "#ffff00", "brightblue": "#5c5cff",
    "brightmagenta": "#ff00ff", "brightcyan": "#00ffff",
    "brightwhite": "#ffffff",
}
FG = "#d8d8d8"
BG = "#151515"

# A cell, in pixels at font-size 15. Every run of text is emitted with an
# explicit `textLength`, so a reader whose monospace font has a different
# advance still gets the columns lined up — a table drawn with box characters
# is unreadable one pixel out.
CELL_W = 9.0
CELL_H = 19.0
FONT_SIZE = 15
# DejaVu first: it has the braille patterns the timeline is drawn with, and it
# is the one font a Linux reader is most likely to have. The rest are what
# macOS and Windows ship.
FONT = (
    "DejaVu Sans Mono,Menlo,SFMono-Regular,ui-monospace,"
    "Consolas,Liberation Mono,monospace"
)


def capture(binary, cols, rows, steps, args=()):
    """Run `binary` under a pty, send `steps`, return the final screen."""
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.environ["COLORTERM"] = "truecolor"
        os.execv(binary, [os.path.basename(binary), *args])
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    screen = pyte.Screen(cols, rows)
    stream = pyte.ByteStream(screen)

    # Entering the alternate screen. A real terminal keeps two buffers and
    # gives the first one back on exit; pyte has one, so anything poptop
    # wrote before this point — a warning about a counter it is not allowed
    # to read — would stay under the frame, in whatever cells the frame does
    # not cover. Clearing at exactly this byte is what the second buffer
    # would have done.
    ALT = b"\x1b[?1049h"

    def feed(chunk):
        while True:
            at = chunk.find(ALT)
            if at < 0:
                stream.feed(chunk)
                return
            stream.feed(chunk[: at + len(ALT)])
            screen.reset()
            chunk = chunk[at + len(ALT) :]

    def pump(seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.05)
            if ready:
                try:
                    feed(os.read(fd, 65536))
                except OSError:
                    return

    for delay, keys in steps:
        pump(delay)
        if keys:
            os.write(fd, keys.encode())
            pump(0.5)
    # Stop feeding the emulator here. poptop prints its held warnings after
    # it gives the terminal back — that is the point of holding them — and
    # pyte, having no second buffer to return to, would scroll the frame up
    # by a line to make room. Drained, not read: a process writing into a
    # full pty blocks instead of exiting.
    try:
        os.write(fd, b"q")
        end = time.time() + 0.5
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.05)
            if ready and not os.read(fd, 65536):
                break
    except OSError:
        pass
    os.close(fd)
    os.waitpid(pid, 0)
    return screen


def colour(name, default):
    """pyte gives a name, a six-digit hex string, or 'default'."""
    if name == "default":
        return default
    if name in ANSI:
        return ANSI[name]
    if len(name) == 6:
        return "#" + name
    return default


def runs(line, cols):
    """Consecutive cells sharing a style, as (start, text, fg, bg, bold)."""
    out = []
    start = 0
    while start < cols:
        cell = line[start]
        style = (cell.fg, cell.bg, cell.bold, cell.reverse)
        end = start
        text = []
        while end < cols:
            c = line[end]
            if (c.fg, c.bg, c.bold, c.reverse) != style:
                break
            text.append(c.data or " ")
            end += 1
        fg, bg = colour(cell.fg, FG), colour(cell.bg, BG)
        if cell.reverse:
            fg, bg = bg, fg
        out.append((start, "".join(text), fg, bg, cell.bold))
        start = end
    return out


def svg(screen, cols, rows, title):
    w, h = cols * CELL_W, rows * CELL_H
    pad = 12
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'width="{w + 2 * pad:.0f}" height="{h + 2 * pad:.0f}" '
        f'viewBox="0 0 {w + 2 * pad:.0f} {h + 2 * pad:.0f}" '
        f'font-family="{FONT}" font-size="{FONT_SIZE}">',
        f"<title>{sax.escape(title)}</title>",
        f'<rect width="100%" height="100%" rx="6" fill="{BG}"/>',
    ]
    for y in range(rows):
        line = screen.buffer[y]
        top = pad + y * CELL_H
        baseline = top + CELL_H - 5
        for x, text, fg, bg, bold in runs(line, cols):
            if not text.strip() and bg == BG:
                continue
            left = pad + x * CELL_W
            length = len(text) * CELL_W
            if bg != BG:
                parts.append(
                    f'<rect x="{left:.1f}" y="{top:.1f}" '
                    f'width="{length:.1f}" height="{CELL_H:.1f}" fill="{bg}"/>'
                )
            if not text.strip():
                continue
            weight = ' font-weight="bold"' if bold else ""
            parts.append(
                f'<text x="{left:.1f}" y="{baseline:.1f}" fill="{fg}"'
                f'{weight} textLength="{length:.1f}" lengthAdjust="spacing" '
                f'xml:space="preserve">{sax.escape(text)}</text>'
            )
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


def main():
    args = sys.argv[1:]
    out = "docs/media/poptop.svg"
    binary = "./target/release/poptop"
    cols, rows = 100, 28
    rest = []
    while args:
        a = args.pop(0)
        if a == "--binary":
            binary = args.pop(0)
        elif a == "--size":
            cols, rows = (int(v) for v in args.pop(0).split("x"))
        else:
            rest.append(a)
    if rest:
        out = rest[0]
    # Long enough for the timeline to have something in it, then a process
    # selected so the accent margin is in the picture.
    steps = [(9.0, None), (0.2, "\x1b[B\x1b[B"), (0.2, None)]
    if not os.path.exists(binary):
        sys.exit(f"{binary} is not built: cargo build --release")
    screen = capture(binary, cols, rows, steps)
    os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
    with open(out, "w") as f:
        f.write(svg(screen, cols, rows, "poptop"))
    # The plain text beside it, so a change to the frame is legible in a diff
    # of something other than coordinates.
    with open(os.path.splitext(out)[0] + ".txt", "w") as f:
        f.write("\n".join(l.rstrip() for l in screen.display) + "\n")
    print(json.dumps({"svg": out, "cols": cols, "rows": rows}))


if __name__ == "__main__":
    main()
