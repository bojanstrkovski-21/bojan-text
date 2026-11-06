# Bojan Text — mechanics sheet

Values read from the original program (`sublime_text.exe`, decompiled with the
built-in lifter — Ghidra's analysis failed on this file) and from the default
package it loads at run time (`Packages/Default.sublime-package`: settings files
and its Python command scripts). Every value is marked with where it came from;
"estimated" means it was looked for and not found.

## Status bar — sub_14036805e

Rules in the code, used by `selection_description()` in the rebuild:

* Empty selection → `"Line " + (line+1) + ", Column " + (col+1)` (strings "Line ", ", Column ").
* Column type comes from `selection_description_column_type` (default `"virtual"`,
  Preferences.sublime-settings). In virtual mode the loop over the line's characters
  adds 1 per char, and for a tab (char 9) `col += tab_size - col % tab_size`
  (the `idiv` / `ebx += ebp - edx` sequence).
* `show_line_column`: `"enabled"` (default) → long form; `"compact"` → `L:C`; `"disabled"` → nothing.
* One non-empty selection → `"N lines, "` prefix when it spans line breaks
  (N = breaks + 1), then `"1 character selected"` or `"<n> characters selected"`.
* Several selection regions → `"<n> selection regions"`; while searching →
  `"<i> of <n> matches"` / `"1 match"`; snippet fields → `"Field i of n"`.
  The rebuild shows "i of n matches" while the find bar is open.
* Indentation item, sub_1400eb0e2: `"Spaces: %d"` when translate_tabs_to_spaces,
  else `"Tab Size: %d"`, with the view's tab size (+0x34c).

## Copy / Cut — sub_14038dafa

* Enabled with an empty selection when `copy_with_empty_selection` is true; its
  default inside the code is `1` (the flag is set to 1 before the setting is read),
  and Preferences.sublime-settings also says `true`. With nothing selected the whole
  line is copied (the rebuild does the same; a copied whole line pastes above the
  current line).

## Wrap Paragraph — sub_14039217e and Default/paragraph.py

* Width = command argument `width`; else setting `wrap_width`; else the first entry of
  `rulers`; else **78** (`0x4e` in the code, `width = 78` in paragraph.py).
* The width is reduced by the width of the line prefix (comment / quote marker),
  measured with tabs as `tab_size` spaces.
* The rebuild: View ▸ Word Wrap Column is the `wrap_width` setting; the fallback is 78.

## Minimap — sub_140387134 (control setup) and sub_1403876e2 (mouse)

* Viewport colour default: the 64-bit constant `0x3f80000060444444` stored at +0x120 →
  colour bytes 44 44 44 60 = rgba(68, 68, 68, 96/255), followed by 1.0f.
  The rebuild draws the viewport box with exactly this colour.
* Mouse press inside the viewport box starts a drag that keeps the grab offset.
* Press outside the box with `minimap_scroll_to_clicked_text` (default true) scrolls to
  the clicked text; the centring uses `(bottom - top) * 0.5` (f64 0.5 at 0x1405aa1d8).
  The rebuild centres the clicked line in the editor.
* Other constants in the click handler: 0.1 (0x1405d8530) and 0.05 (0x1405c2ca8),
  used for the scroll animation (not reproduced; the rebuild scrolls immediately).
* `minimap_horizontal_scrolling` is read at setup (not reproduced).
* A minimap measure is computed as `(font line height + line_padding_top +
  line_padding_bottom) * 0.5` (f32 0.5 at 0x1405aa7a4). Its exact use was not
  confirmed, so the rebuild's minimap line height (2.6 px) stays **estimated**.

## Settings read by the view — sub_14037d6b4

The view reads `margin`, `fold_buttons`, `line_padding_top`, `rulers`, `wrap_width`,
`highlight_line`, `draw_indent_guides`, `caret_style`, `block_caret`,
`caret_extra_width` and `drag_text` (string references at 0x14037e441 … 0x14037f7b5).
Their defaults come from Preferences.sublime-settings:

| Setting | Original value | Used in Bojan Text |
|---|---|---|
| font_face / font_size (Windows) | Consolas, 11 pt (14.67 px) | same |
| tab_size | 4 | 4 |
| translate_tabs_to_spaces | false | false |
| wrap_width | 0 (= window width) | 0 |
| rulers | [] | [] |
| scroll_past_end | true | true |
| indent_guide_options | ["draw_normal"] | indent guides on |
| highlight_line | false | **true** (theming change, View menu toggle) |
| draw_minimap_border / always_show_minimap_viewport | false / false | same |
| hot_exit | "always" | unsaved text kept between runs |
| file_tab_style | "rounded" | rounded |
| default_line_ending | "system" | Windows (CRLF) for new files; detected for opened files |

## Commands from the original's scripts (Default.sublime-package)

* sort.py: Sort Lines is case-insensitive by default (`key=lambda x: x.lower()`),
  "Case Sensitive" uses plain sort; Permute Lines = reverse / unique / shuffle.
* transpose.py: empty selection inside a word swaps the two characters around the
  cursor; at the end of a word it swaps that word with the next word (or the previous
  one when there is no next word). Implemented in `textops::transpose`.
* sub_140388bec registers the built-in text commands (split_selection_into_lines,
  single_selection, indent, unindent, join_lines, start/stop_record_macro,
  fold_tag_attributes, …); macro recording is not offered in widget views.

## Tabs — sub_1400e3d5a (tab control properties and their defaults)

The function registers each property with a 64-bit float default (decoded from the
IEEE-754 bytes in the code):

| Property | Bytes in code | Value | Used in Bojan Text |
|---|---|---|---|
| `tab_width` | 0x4066800000000000 | 180.0 | tab maximum width 180 px |
| `tab_min_width` | 0x4048000000000000 | 48.0 | tab minimum width 48 px |
| `tab_height` | 0x4041800000000000 | 35.0 | tab bar height 35 px (was an estimated 36) |
| `connector_height` | 0 | 0.0 | — |
| `tab_overlap` | 0x4031000000000000 | 17.0 | (theme-dependent; not reproduced) |

Also read: `mouse_wheel_switch` (wheel over the tab bar switches tabs) and `spacing`.
Titles longer than the tab are cut off before the close button.

## Split panes — sub_14008f5e2 (layout parsing), sub_14004367c (session save)

* A layout is `{"cols": [...], "rows": [...], "cells": [[x1, y1, x2, y2], ...]}`
  (strings "rows", "cols", "cells" at 0x1406b5cc7 / 0x1406b6b0c / 0x1406b6b64). Every
  cell index must lie inside the column / row lists, otherwise the code reports
  `"invalid cell: "`. Each cell is one group (pane).
* The presets in Default (Windows).sublime-keymap (Alt+Shift+1…5, 8, 9):
  Single `cols [0,1]`; Columns 2 `[0,.5,1]`; Columns 3 `[0,.33,.66,1]`;
  Columns 4 `[0,.25,.5,.75,1]`; Rows 2 `[0,.5,1]`; Rows 3 `[0,.33,.66,1]`;
  Grid 4 = 2 × 2. The rebuild uses the same fractions (the 3-way layouts are
  0.33 / 0.66, not exact thirds).
* Ctrl+1…9 = focus_group 0…8, Ctrl+Shift+1…9 = move_to_group 0…8 (keymap).
* `remember_layout` (read at 0x1406b48a2 by sub_14004367c) defaults to **false**: the
  layout is only written to the saved session when it is on, so the rebuild starts with
  a single pane each time. The session also stores `distraction_free`, `side_bar_width`,
  groups / sheets, buffers, `selected_group`, folders, expanded folders and the project.

## Find panel — sub_140070bdd (saved find options)

Options saved with the find history: `in_selection`, `regex`, `case_sensitive`,
`whole_word`, `reverse`, `wrap`, `highlight`, `preserve_case`, `show_context`,
`use_buffer2`, `use_gitignore`, `scrollbar_highlights` (+ `find_history`,
`replace_history`). The rebuild offers regex, case sensitive, whole word, wrap around
(default on), in selection and highlight matches (default on); whole word wraps the
query in `\b…\b`. `find_selected_text` (default true) fills the find box from the
selection when it opens.

## Find in Files results — sub_140067264, sub_140068cc4, sub_140068eaa, sub_140069544, sub_140069d7a

The results view is a text view named "Find Results" (syntax
`Packages/Default/Find Results.hidden-tmLanguage`, tag `find_in_files:output`) with
`line_numbers` off, `scroll_past_end` on, `detect_indentation` off, and the regexes
`result_file_regex = ^([^ \t].*):$` and `result_line_regex = ^ +([0-9]+):` (used to jump
to a result). The text, as written by the code (rebuilt in the Find Results panel):

1. Header (sub_140068cc4): `Searching <N> files for "<query>"`, then, if regex / case
   sensitive / whole word are set, ` (regex, case sensitive, whole word)`, then a newline.
2. For each file with matches (sub_140068eaa): `<path>:` newline; each hit as
   `" %4d"` (line number, width 4) followed by `": "` and the line text (context lines
   use two spaces instead of `": "`); a blank line after the file. A binary file shows
   `<path>:` / `    <binary>`; failures show `ERROR: Unable to open file`,
   `File too large, skipping`, `ERROR: Regex complexity too high searching file`,
   `ERROR: Regex exhausted stack searching file`.
3. Summary (sub_140069544): `<n> matches` (or `1 match`), then ` across <m> files` or
   ` in 1 file`; before it `Cancelled!` when stopped, or `Reached maximum result size.
   Check "find_in_files_max_result_size".`; ` error suppressed` / ` errors suppressed`
   when `find_in_files_suppress_errors` is on.
4. Per-file counting (sub_140069d7a): total matches += the file's hits; files with matches
   += 1; the messages `Unable to open …`, `skipping binary file: …`,
   `Regex exhausted stack searching …`, `Regex complexity too high searching …` are
   reported for files that failed. With nothing to show the code says `No find …`
   (results); sub_140074d8c appends ` (%d files)` to the window title for searches over
   2 or more folders.
The rebuild shows the header, per-file blocks and summary in exactly this layout
(binary / error blocks are not reproduced: such files are skipped).

## Enter and Tab — Default (Windows).sublime-keymap

* Enter inserts a newline; with `auto_indent` (default true) the new line copies the
  indent; with `trim_automatic_white_space` (true) a whitespace-only line is emptied.
  When `{` is directly before the cursor and `}` directly after it, the macro
  "Add Line in Braces" runs (blank, indented line between the braces).
* Tab, last matching rule wins: multi-line selection → indent; empty line with
  auto_indent → reindent; snippet trigger before the cursor → expand_snippet;
  `tab_completion` (true) with a word before the cursor → completion (commit when there
  is exactly one candidate); else insert a tab.
* Shift+Tab: unindent when only whitespace precedes the cursor or the selection spans
  lines, otherwise insert a tab.
* Ctrl+Shift+A = expand_selection "smart", or "tag" only where the selector
  `(text.html, text.xml) - source` matches; Ctrl+Shift+Space = scope; Ctrl+L = line;
  Alt+L = previous line; Ctrl+M = move to brackets; Ctrl+K, Ctrl+] = reindent one line.

## Color schemes and themes

* sub_14017b513 reads `color_scheme`; for `"auto"` it switches to the `dark_` or
  `light_` prefixed key. Defaults: Mariana (dark) and Breakers (light).
* sub_140173198 parses colour expressions: rgb, rgba, hsl, hsla, blend, blenda, alpha,
  lightness, saturation, min-contrast. Bojan Text implements the same set
  (`min-contrast` is ignored).
* Mariana background hsl(210, 15%, 22%); Default Dark base hue hsl(215, 10%, 50%).

## Packages, plugins, language servers

* `*.sublime-syntax` (0x1406b3c1a, sub_14033c950) and inheritance
  ("extends must be a path to a .sublime-syntax file", sub_140317f6c).
* No language-server code in the program (no "LSP" / "textDocument" strings); that
  comes from the Python LSP package. Bojan Text has its own LSP client.

## Menus and key bindings

* Menu bar from Default.sublime-package / Main.sublime-menu, same items and order;
  labels, shortcuts and greyed-out states were checked against the person's screenshots
  of the original's menus (e.g. Join Lines = Ctrl+Shift+J, Expand Selection =
  Ctrl+Shift+A, Expand to Scope = Ctrl+Shift+Space, Quick Switch Project =
  Alt+Shift+P, Next / Previous Modification = Ctrl+. / Ctrl+,, View items read
  "Hide Minimap" / "Hide Tabs" / "Hide Status Bar" / "Hide Menu").
* Split panes follow the original's View ▸ Layout (Single, Columns 2–4, Rows 2–3,
  Grid 4 = Alt+Shift+1…5, 8, 9), Groups (New Group Ctrl+K Ctrl+Shift+Up, Close Group
  Ctrl+K Ctrl+Down, Max Columns 1–5), Focus Group (Ctrl+1…) and Move File to Group
  (Ctrl+Shift+1…); File ▸ Split View opens a synced copy in the next group.
* The syntax chooser opens as a narrow list just above the syntax name in the status
  bar (the original's popup), with a check mark on the current syntax.
* Items needing multiple cursors, code folding or spell checking are shown greyed
  out with the reason.

## Estimated (looked for, not found)

* Minimap line height 2.6 px, character width 1.15 px, panel width 110 px.
* Status bar 26 px, side bar default 220 px, first window 1280 × 800 (the tab bar height is
  now read from the code: 35 px).
