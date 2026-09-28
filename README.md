# typewriter

## Concept

It's a typewriter simulator, so expect the writing process to be slow and tedious.

Organization-wise, each file is structured virtually like a Manila folder, containing loose pieces of paper,
just like how drafting processes would have had looked like. You can access already-typed documents within the
typewriter app using the GUI. You can only view them but not re-edit them.

When you finished typing the document, inserting a new page will push the finished document into Manila folder

So this app partly forces you to type more carefully as well as manage your document writing process manually.

## Install

Linux only. You need [Rust](https://rustup.rs) and, to build the sound, the ALSA headers and
`pkg-config` (e.g. `alsa-lib` on Arch and Gentoo, `libasound2-dev` on Debian and Ubuntu,
`alsa-lib-devel` on Fedora).

```sh
git clone https://github.com/dangnq-swin/typewriter
cd typewriter
scripts/install.sh
```

This builds the app and installs, for your user only:

- the `typewriter` command, in `~/.local/bin`
- **Typewriter** in your desktop's app menu, with its icon

Start it from the menu, or run `typewriter`, optionally with a project file to open
(`typewriter novel.folder.ron`). To install elsewhere, set `PREFIX`, e.g.
`sudo PREFIX=/usr/local scripts/install.sh`.

To uninstall, run `scripts/install.sh --uninstall` (with the same `PREFIX`, if you set one).
Your projects, drafts and settings stay where they are.

Just the command, without the menu entry, also installs with Cargo:

```sh
cargo install --locked --git https://github.com/dangnq-swin/typewriter typewriter-app
```

## Controls

### Typing

| Key | Does |
|---|---|
| Backspace | Move back one step |
| Shift+Backspace or Delete | Move back one step while fixing your mistake |
| Tab | Jump to the next tab stop |
| Shift + Tab (tap while holding Shift) | Once: set a tab stop at the carriage. Twice: clear the nearest stop. Three times: clear all stops |
| Insert | Take the sheet out and feed a new one. Enter on the last line does the same |
| F1/F2/F3 | Line spacing 1/1.5/2 |
| F4 | Change correction method |
| Esc | Calm mode on/off |
| F11 | Fullscreen |
| Mouse wheel | Zoom |

### Fixing mistakes

- **Correction paper** (default): Shift+Backspace will put a piece of correction paper on.
You can then write over your mistakes. Shift+Backspace again to remove the correction paper piece
- **Eraser**: Shift+Backspace rubs out the letter before the carriage.
- **Correction fluid**: Shift+Backspace dabs fluid on the letter before the carriage. Wait
3 seconds for it to dry or observe visually, because typing on wet fluid will cause the text to be smudged.

### On screen

| Control | Does |
|---|---|
| **Spacing** plate | Click: next line spacing |
| **Zoom** plate | Double-click: 100 % |
| **Correct** plate | Click: next mistake-fixing method |
| **Goal** plate | Click: next session goal (words or minutes, or off) |
| Folder icon (bottom left) | The finished sheets and the project menus |
| Sheet icon | Calm mode on/off |
| Gear icon | Settings |

### Folder of finished sheets

Opened with **Page Up** or the folder icon.

| Key | Does |
|---|---|
| Arrow keys, Page Up / Page Down | Choose a sheet (up / left = older) |
| Enter or click | Read the chosen sheet; the same keys flip through sheets |
| Esc | Back one level |
| Typing | Back to the typewriter |

The plates below the folder save, open, rename and export the project; click the name on
the folder's tab to rename it.
