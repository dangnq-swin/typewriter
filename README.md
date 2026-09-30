# typewriter

[![Latest release](https://img.shields.io/github/v/release/dangnq-swin/typewriter?label=latest%20release)](https://github.com/dangnq-swin/typewriter/releases/latest)

## TLDR

It's a typewriter simulator, so expect the writing process to be slow and tedious. The app is very opinionated:
it partly forces you to type carefully and to manage your drafts by hand.

Organization-wise, each file is structured virtually like a Manila folder, containing loose pieces of paper,
just like how drafting processes would have looked like.

When you finish a sheet, feeding in a new one files it in the Manila folder. The icons at the bottom left open
the desk: the folder of finished sheets, a notebook for notes, and a calendar of words per day.

There's also other modes + goodies that are nice-to-have without being too bloated.

## Install

Download a build for Linux or Windows from the [latest release](https://github.com/dangnq-swin/typewriter/releases/latest),
or build it yourself (Linux only). You need [Rust](https://rustup.rs) and, to build the sound, the ALSA headers and
`pkg-config` (e.g. `alsa-lib` on Arch and Gentoo, `libasound2-dev` on Debian and Ubuntu,
`alsa-lib-devel` on Fedora).

```sh
git clone https://github.com/dangnq-swin/typewriter
cd typewriter
scripts/install.sh
```

Install script install these:

- the `typewriter` command, in `~/.local/bin`
- **Typewriter** in your desktop's app menu, with its icon
- the `.typr` project file type: project files open in Typewriter from your file manager, in the
  window already open if there is one

Start it from the menu, or run `typewriter`, optionally with a project file to open
(`typewriter novel.typr`). To install elsewhere, set `PREFIX`, e.g.
`sudo PREFIX=/usr/local scripts/install.sh`.

To uninstall the app, run `scripts/install.sh --uninstall`.

Manual install, without mime types and desktop entry:
```sh
cargo install --locked --git https://github.com/dangnq-swin/typewriter typewriter-app
```

## Controls

### Typing

| Key | Does |
|---|---|
| Backspace | Move back one step. After a type jam: free the typebars |
| Shift+Backspace or Delete | Move back one step while fixing your mistake |
| Tab | Jump to the next tab stop |
| Shift + Tab (tap while holding Shift) | Once: set a tab stop at the carriage. Twice: clear the nearest stop. Three times: clear all stops |
| Insert | Take the sheet out and feed a new one. Enter on the last line does the same |
| Up / Down | Platen knob: roll the paper a half-line, for superscripts and footnote marks (or drag or scroll a knob either side of the paper). Faint guides show where the next letter will sit (Settings → Look) |
| Left / Right | Move the carriage without typing (only with *Free movement* on in the settings) |
| Home | Margin release: type past the margins until the next return (or click a margin stop) |
| Shift+Home / Shift+End | Set the left / right margin at the carriage (or drag the stops on the scale) |
| F1/F2/F3 | Line spacing 1/1.5/2 |
| F4 | Change correction method |
| 1 or ! | Open the notebook (Esc or a click away puts it back) |
| Page Up / Page Down | In the open notebook: turn a leaf (or click the arrows in its bottom corners) |
| Esc | Calm mode on/off |
| F11 | Fullscreen |
| Ctrl+S | Save |
| Mouse wheel | Zoom |

### Fixing mistakes

- **Correction paper** (default): Shift+Backspace will put a piece of correction paper on.
You can then write over your mistakes. Shift+Backspace again to remove the correction paper piece
- **Eraser**: Shift+Backspace rubs out the letter before the carriage.
- **Correction fluid**: Shift+Backspace dabs fluid on the letter before the carriage. Wait
3 seconds for it to dry or observe visually, because typing on wet fluid will cause the text to be smudged.
- **Delete** (only when *Delete in the correction cycle* is on in the settings):
Shift+Backspace removes the letter before the carriage without a trace, digital style.

### On screen

| Control | Does |
|---|---|
| Platen knobs (either side of the paper) | Drag or scroll: roll the paper a half-line a notch |
| Red margin stops on the scale | Drag: move the margin. Click: margin release |
| **Spacing** plate | Click: next line spacing |
| **Zoom** plate | Double-click: 100 % |
| **Correct** plate | Click: next mistake-fixing method |
| **Goal** plate | Click: next session goal (words or minutes, or off) |
| **Autosave** plate (right) | How the project is kept; the dot is the status. Click: save|
| Folder icon (bottom left) | The finished sheets and the project menus |
| Notebook icon | Open the notebook |
| Sheet icon | Calm mode on/off |
| Gear icon | Settings |

### Folder of finished sheets

Opened with **Page Up** or the folder icon.

| Key | Does |
|---|---|
| Arrow keys, Page Up / Page Down | Choose a sheet (up / left = older) |
| Shift + arrow keys | Move the chosen sheet one place |
| Delete | Scrunch up the chosen sheet |
| Enter or click | Read the chosen sheet; the same keys flip through sheets |
| Esc | Back one level |
| Typing | Back to the typewriter |
| 1 or ! | Open the notebook (or click the notebook beside the folder) |

The calendar logs the writing progress, eg. how many words are written per day. Click to bring up close.

In an open sheet, click its top margin to pencil a note there (Enter for a new line, as many
lines as the margin has room for). Click elsewhere or press Esc when done.

### Exports

The folder's **Export…** plate writes the project as `.md`, `.txt` or `.pdf` (the sheets as typed), beside the
project file. Margin notes are included while the notebook isn't.

There's also printing options.