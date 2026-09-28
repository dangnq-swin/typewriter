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
- [ ] Verify the window on X11
- [x] `assets/LICENSES.md`, Courier Prime vendored
- [x] CI: none for now

## M1: Core typewriter model (no GUI)

- [ ] Page model: fixed grid of cells derived from paper size + pitch + line spacing
- [ ] Cells hold a **stack of glyphs** (overtyping and strikeouts are preserved)
- [ ] Carriage: column position, left/right margin stops, margin release
- [ ] Margin bell event a configurable number of columns before the right margin
- [ ] Carriage return + line feed as one action. Line spacing 1 / 1.5 / 2
- [ ] Tab stops (set / clear / tab)
- [ ] Constraint settings (each toggleable): backspace allowed?, delete allowed?,
      typing past right margin blocked?, cursor movement allowed?
- [ ] Olympia SM9 profile loaded from `profiles/olympia-sm9.toml`:
      Pica 10 cpi, 6 lpi, A4, default margins, bell offset
- [ ] Event stream (`KeyStrike`, `Space`, `Bell`, `CarriageReturn`, `PageEnd`, `Blocked`)
- [ ] Unit + scenario tests for all of the above

## M2: Minimal typing GUI

- [ ] Render a single A4 sheet with a monospace typewriter font at correct pitch
- [ ] Keyboard input → core commands. Enter = carriage return
- [ ] **Platen view**: the typing line stays at a fixed vertical position, and the paper
      scrolls up and moves sideways like a carriage (option to disable horizontal travel)
- [ ] Visual feedback for blocked input (e.g. at right margin in strict mode)
- [ ] Load Courier Prime from `assets/fonts/` and size it so one advance = 1/10 in (Courier metrics: 0.6 em → 12 pt)

## M3: Paper & page view

- [ ] Load the maintainer-supplied off-white paper texture as default background
- [ ] Flat colour fallback when texture is missing
- [ ] Configurable background: texture path, colour, desk/surround colour
- [ ] Multi-page documents: page end → "feed a new sheet" action
- [ ] Zoom levels. Window resize keeps the paper centred
- [ ] Subtle ink realism (optional, toggleable): slight per-glyph offset/opacity variance

## M4: Sound

- [ ] Audio playback via `rodio` (or `kira` if latency is a problem), non-blocking
- [ ] Key strike (several variants to avoid repetition), space bar, carriage return, bell
- [ ] Master volume + per-sound toggles, mute shortcut
- [ ] ❓ Sound source: record a real SM9, maintainer-supplied, or CC0 samples?

## M5: Calm mode

- [ ] Toggle for distraction-free mode
- [ ] Lines dim progressively with distance from the current line (configurable falloff curve
      and minimum opacity)
- [ ] Hide all chrome (menus, counters) in calm mode. Fullscreen shortcut

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

- [ ] Settings window: constraints, sounds, background, calm mode, goals
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

- Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with its own
  pitch, typeface, bell offset and sounds
- Elite (12 cpi) type option for the SM9
- Two-colour ribbon (black / red) switch
- Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"
- Carriage-return lever animation
- Print-ready PDF export that keeps the look of the typed page
