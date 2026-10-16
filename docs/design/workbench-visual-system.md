# Workbench visual system: tokens and rules (M6 lane V)

**Status: direction chosen (M6.O1, 2026-10-16): Workbench.** Token system (M6.V3), status
component (M6.V4), panel frame and table style (M6.V5) landed. **The style guide is
`docs/design/workbench-style.md` (M6.V6)**: the tokens, components, the status rule, the
panel frame, and how a panel is added moved there (lane V round 3). This page keeps the
choice and its record.

## Chosen direction

The owner, in chat, 2026-10-16 (M6.O1), verbatim: "Workbench, themes look great only keep the mocks if there's a good reason."

- **Workbench** is the default preset: neutral graphite and cool grey, dense, 13 pt body,
  20 pt zebra rows, square corners, one steel-blue accent, designed dark first. Without
  `--theme` or a `theme` in `ui.json` the window opens in Workbench and follows Windows'
  light or dark setting.
- **Studio** stays selectable (`--theme studio`, or the Start tab's picker): the other
  mockup direction, kept because it costs nothing (it is data in `tokens.rs`) and keeps
  the token system honest (every panel must look right under two token sets).
- **Classic** stays selectable as **legacy** (the picker says so): egui's default style
  from before the tokens, below AA in light mode, kept only for comparison.
- **The mockups were removed** (`docs/design/mockups/2026-10-16/`, 12 PNGs, 3.6 MB). No
  good reason to keep them was found: the choice is recorded here and in
  `plan/execution-plan.md` (M6.O1), the images stay in the history at commit `48beef1`, and
  any of them can be made again with the screenshot tour and `--theme`
  (`docs/workbench.md`, "Screenshot tour"). Tour images stay outside the repo.

Why: the owner's pains on 2026-10-16 were "look, feel, performance". The director's brief
named plain egui styling, cramped tables and no theming. The tokens (style guide) are the answer
for look and feel. They are data, in one place, so a direction is chosen by values and
not by editing panels.

## Contrast as measured at M6.V3

Lowest contrast of each status color against the six surfaces (measured by the test's
own WCAG 2.x luminance code):

| Preset, mode | pass | fail | warn | not-run | inconclusive | muted text | accent |
| ------------ | ---: | ---: | ---: | ------: | -----------: | ---------: | -----: |
| Workbench light | 5.0 | 5.0 | 5.4 | 5.5 | 5.8 | 5.5 | 5.3 |
| Workbench dark | 5.6 | 4.6 | 6.3 | 5.1 | 5.2 | 4.6 | 5.3 |
| Studio light | 4.9 | 5.4 | 5.5 | 5.8 | 5.9 | 5.8 | 5.3 |
| Studio dark | 6.2 | 5.3 | 6.8 | 5.0 | 5.9 | 4.8 | 5.6 |

`control_border` and `focus` measure at least 3.2:1 against every surface in all four
palettes (test `control_border_and_focus_meet_non_text_contrast_in_both_modes`), and
`status.unknown` at least 5.1:1.

`classic` keeps the pre-token four constants (`WARN #e6aa3c`, `BAD #e65a50`, `GOOD #6ebe6e`,
`CODE #8cbeeb`, the same in both modes) on egui's default style. On egui's light panel
(`#f8f8f8`) they measure warning 1.9, pass 2.1 and fail 3.3: below AA. That is the
problem the presets fix; `classic` stays as it is, selectable and marked legacy, and the
contrast rules below cover the two token presets.

## Layout history

- M6.V3 round 2: the session text moved from the top bar to a status line at the bottom;
  Records became two panes with a draggable line (`records_split` in `ui.json`).
- M6.V5 (round 3): every tab in one panel frame (head, action bar, result header with the
  limitations strip, body); the coverage and cockpit tables became virtual tables; the
  Start tab's About line and the debug build's title bar (`docs/design/workbench-style.md`).
