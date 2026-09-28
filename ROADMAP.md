# Roadmap

Milestones are ordered. Each one should leave the app in a working, testable state.
Decisions already made are listed in [`AGENTS.md`](AGENTS.md#decision-log). Items marked
❓ need input from the maintainer before implementation starts.

---

## M0: Project foundation

- [x] `git init`, `.gitignore`, GPL-3.0 `LICENSE`
- [x] Cargo workspace with `typewriter-core` (lib) and `typewriter-app` (bin `typewriter`)
- [x] Workspace lints (`unsafe_code` forbidden, `unwrap`/`expect` warned)
- [x] Empty eframe window opens on Wayland
- [x] `assets/LICENSES.md`, Courier Prime vendored
- [x] CI: none for now

## M1: Core typewriter model (no GUI)

- [x] Page model: fixed grid of cells derived from paper size + pitch + line spacing
- [x] Cells hold a **stack of glyphs** (overtyping and strikeouts are preserved)
- [x] Carriage: column position, left/right margin stops, margin release
- [x] Margin bell event a configurable number of columns before the right margin
- [x] Carriage return + line feed as one action. Line spacing 1 / 1.5 / 2
- [x] Tab stops (set / clear / tab)
- [x] Constraint settings (each toggleable): backspace allowed?, erase mode (off / digital /
      white-out / correction tape), typing past right margin blocked?, cursor movement allowed?
- [x] Olympia SM9 profile loaded from `profiles/olympia-sm9.toml`:
      Pica 10 cpi, 6 lpi, A4, default margins, bell offset
- [x] Event stream (`KeyStrike`, `Space`, `Backspace`, `Tab`, `Erase`, `Bell`, `CarriageReturn`,
      `PageEnd`, `Blocked`)
- [x] Unit + scenario tests for all of the above

## M2: Minimal typing GUI

- [x] Render a single A4 sheet with a monospace typewriter font at correct pitch
- [x] Keyboard input → core commands. Enter = carriage return, Backspace = carriage back,
      Shift+Backspace / Delete = erase
- [x] **Platen view**: the typing line stays at a fixed vertical position, and the paper
      scrolls up and moves sideways like a carriage (option to disable horizontal travel)
- [x] Visual feedback for blocked input (e.g. at right margin in strict mode)
- [x] Embed Courier Prime from `assets/fonts/` and size it so one advance = 1/10 in (Courier metrics: 0.6 em → 12 pt)
- [x] Carriage scale below the typing line: column ticks, margin brackets, tab stop pointers
- [x] Line spacing indicator ("Spacing:" + two circles) right below the scale, clickable

## M3: Paper & page view

- [x] Load the maintainer-supplied off-white paper texture as default background (whole window,
      bundled at JPEG quality 70 by `build.rs`)
- [x] Flat colour fallback when texture is missing
- [x] Sheet drawn as a margin frame (1 mm clearance) with its sides extended to the paper edges
- [x] Multi-page documents: Insert feeds a new sheet, finished sheets are kept
- [x] Folder of finished sheets (2.5D, Page Up or desk icon), read-only sheet view
- [x] Zoom levels (mouse wheel, percentage plate below the scale, double-click for 100 %). Window resize keeps the typing point centred
- [x] Subtle ink realism (on by default): slight per-glyph offset/opacity variance

## M4: Sound

- [x] Audio playback via `rodio`, non-blocking (clips decoded once, mixed on the output device)
- [x] Key strike (six variants, never the same twice in a row), space bar, backspace, tab,
      erase, sheet feed (input waits until it has played), blocked input, margin bell (two variants, picked at random)
- [x] Held Enter rolls the paper (line feed) with a platen ratchet click per line; Space,
      Backspace and erase keys do not auto-repeat
- [x] Carriage return sound, silenced per profile (`[sounds]` in the profile; the SM9's is silent)
- [x] Sheet feed animation: old sheet rolls out, new sheet rises with the knob turns (curled
      edge, shadow), typing pointer fades in at the end
- [x] Sound source: CC0 samples (mostly a Hermes Precisa 305), cut by `scripts/prepare-sounds.sh`
- Master volume, per-sound toggles and mute move to settings (M8)

## M5: Calm mode

- [x] Toggle for distraction-free mode (Esc, or the icon next to the folder icon, which stays shown)
- [x] Lines dim progressively with distance from the current line (falloff and minimum opacity
      are constants; configurable with settings in M8)
- [x] Hide the chrome (scale, plates, folder icon) in calm mode, fading over 250 ms.
      Fullscreen on F11

## M6: Persistence & export

- [ ] Native document format (serde, versioned schema) storing pages, glyph stacks,
      profile used, and session stats
- [ ] ❓ Serialization format: RON, JSON, or TOML? (RON recommended for readability of nested data)
- [ ] Autosave at intervals and on quit. Crash-safe writes (write temp + rename)
- [ ] Open / new / recent documents
- [ ] Export to **plain text** (last glyph wins, or strikeouts removed, configurable)
- [ ] Export to **Markdown** (page breaks as `---`, optionally strikeouts as `~~text~~`)

## M7: Focus goals

- [ ] Session goals: word count and/or timer (e.g. 500 words, 25 minutes)
- [ ] Unobtrusive progress indicator (hidden in calm mode, optional gentle bell on reach)
- [ ] Session stats stored with the document (words per session, time written)

## M8: Settings & profiles

- [ ] Settings window: constraints, sounds (master volume, per-sound toggles, mute), calm mode, goals
- [ ] Config persisted to `$XDG_CONFIG_HOME/typewriter/config.toml`
- [ ] Profile selection. User profiles loaded from `$XDG_DATA_HOME/typewriter/profiles/`
- [ ] Documented profile schema so new machines can be added as data only

## M9: Release (Linux)

- [ ] README with screenshots and usage
- [ ] `cargo install` support
- [ ] ❓ Packaging: Flatpak, AppImage, distro package (e.g. Gentoo ebuild), or none?
- [ ] `.desktop` file and icon

---

## Later / ideas

- Verify and support X11 (Wayland is the current target)
- Configurable background: own texture path or a flat colour instead of the bundled paper photo
- Margin controls: key bindings for margin release and setting the left/right margin at the
  carriage (already supported by the core), margin-release indicator on the scale
- Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with its own
  pitch, typeface, bell offset and sounds
- Elite (12 cpi) type option for the SM9
- Two-colour ribbon (black / red) switch
- Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"
- Carriage-return lever animation
- Print-ready PDF export that keeps the look of the typed page
