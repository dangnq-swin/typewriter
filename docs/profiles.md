# Machine profiles

A profile describes a typewriter as data: its type pitch, line pitch, paper, margins, bell
and which of its mechanisms make a sound. The built-in **Olympia SM9**
([`profiles/olympia-sm9.toml`](../profiles/olympia-sm9.toml)) is one; you can add your own
without changing any code.

## Adding a machine

1. Copy `profiles/olympia-sm9.toml` to `$XDG_DATA_HOME/typewriter/profiles/` (usually
   `~/.local/share/typewriter/profiles/`), under any name ending in `.toml`.
2. Give it a new `name` and change what differs.
3. Open **Settings** (the gear at the bottom left) and choose it under **Machine**. New
   projects are typed on it; a project always keeps the machine it was started on.

Profiles are read at start and whenever the settings are opened. A file with a mistake is
listed under **Machine** with the reason, and is skipped. A project typed on a machine
that is not found (e.g. a removed profile) cannot be opened until the profile is back.

A profile brings no sounds or typeface of its own yet: every machine uses the bundled
clips and Courier Prime, at the profile's pitch.

## Schema

TOML. Unknown keys are an error, so typos are caught.

| Key | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | Shown in the settings and stored in each project file. Must be unique; a user profile cannot reuse a built-in name |
| `pitch_cpi` | integer > 0 | yes | Characters per inch: 10 for Pica, 12 for Elite |
| `lines_per_inch` | integer > 0 | yes | Line pitch, usually 6. The platen turns in half-line steps |
| `bell_columns_before_margin` | integer | yes | How many columns before the right margin the bell rings |
| `tab_stops` | array of integers | no (empty) | Tab stops set when a project starts, as column numbers (0 = the paper's left edge) |
| `[paper]` `width_mm`, `height_mm` | numbers > 0 | yes | Paper size. With the pitches it sets the grid: whole columns across and half-lines down |
| `[margins]` `left_column` | integer | yes | Where the carriage returns to |
| `[margins]` `right_column` | integer | yes | The first column the carriage locks at: typing stops before it. Must be greater than `left_column` and at most the number of columns |
| `[margins]` `top_lines` | integer | yes | Blank lines above the first typed line of a sheet. Must leave room on the sheet |
| `[sounds]` `carriage_return` | bool | no (true) | Whether the carriage return makes a sound |
| `[sounds]` `line_feed` | bool | no (true) | Whether the platen ratchet clicks when the paper is rolled |

Columns across the sheet are `floor(width_mm / 25.4 × pitch_cpi)`, lines down are
`floor(height_mm / 25.4 × lines_per_inch)`. A4 at Pica is 82 columns by 70 lines.

## Example: an Elite machine on US Letter

```toml
name = "Elite on Letter"
pitch_cpi = 12
lines_per_inch = 6
bell_columns_before_margin = 8
tab_stops = [17, 30]

[paper]
width_mm = 215.9
height_mm = 279.4

[margins]
left_column = 12
right_column = 90
top_lines = 6

[sounds]
carriage_return = true
```
