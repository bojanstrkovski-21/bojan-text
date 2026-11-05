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

* Menu bar from Default.sublime-package / Main.sublime-menu, same items and order.
* Shortcuts from Default (Windows).sublime-keymap (Ctrl+K chords included).
* Items needing multiple cursors, split panes, code folding or spell check are shown
  greyed out with the reason.

## Estimated (looked for, not found)

* Minimap line height 2.6 px, character width 1.15 px, panel width 110 px.
* Tab bar 36 px, status bar 26 px, side bar default 220 px, first window 1280 × 800.
