# cosmic-text 0.15.0, patched

The published `cosmic-text 0.15.0` source (`src/`, manifest, licenses; the
`tests/`, `benches/` and `fonts/` directories and their manifest entries are
dropped) with one change in `src/font/fallback/mod.rs`: the fallback iterator
matches a family at its closest weight when it has no face at the exact one.

## Why

`FontFallbackIter` chose a face for a run in three phases — the requested
family, then the platform's per-script and common fallback lists, then every
face in the database in weight-distance order, re-shaping the whole run
against each candidate until the glyph was covered. The first two phases only
considered faces whose weight matched the request EXACTLY
(`font_weight_diff == 0`). fontdb registers a variable font once at its OS/2
default weight (400 for `Geist[wght]`), and the bundled fallback faces
(`NotoColorEmoji.ttf`, the system CJK and symbol faces) are static 400s, so a
run asked for at 500/600/700:

- skipped the requested family itself, even though `get_font(id, weight)`
  sets the `wght` axis correctly once the face is reached;
- skipped every listed fallback family, so an emoji or CJK run fell into the
  third phase and re-shaped against hundreds of system faces — one emoji
  paragraph went from 26us at 400 to 2,963us at 600 on the rig;
- let a common-list family with a face at exactly that weight win a plain
  Latin run before the requested family was reached: DejaVu Sans Bold on
  Debian/Ubuntu, Menlo Bold on macOS ([pop-os/cosmic-text#416]).

## The change

`family_match_key(family_name)` returns the family's first face at the exact
weight, else the first face naming the family in the already weight-sorted
key list (its closest weight; fontdb's own query result sits at index 0).
`default_font_match_key` and the script-list and common-list loops use it.
The monospace path and the final walk are untouched, so the 400 case costs
exactly what it did: the exact pass returns before the second scan runs.

`app/src/tests/font_fallback.rs` pins it — shape allocations at semibold and
bold within 2x of regular for an emoji, a symbol, a Hangul and a Latin run,
and Latin glyphs shaped with Geist at every weight.

## Drop condition

Delete this directory and the `[patch.crates-io]` entry in the workspace
`Cargo.toml` when the cosmic-text release iced pins carries both halves:

- [pop-os/cosmic-text#486] (merged for 0.19) matches a VARIABLE font at any
  weight its `wght` axis covers. That is the requested-family half only; a
  static fallback face at 400 is still invisible at 500/600/700 in that code.
- The static-fallback half (this patch's closest-weight second pass, or an
  equivalent) has no upstream change yet — cite it here once filed.

iced 0.14 pins cosmic-text 0.15, so both the pin bump and the upstream fix
have to land before this can go.

[pop-os/cosmic-text#416]: https://github.com/pop-os/cosmic-text/issues/416
[pop-os/cosmic-text#486]: https://github.com/pop-os/cosmic-text/pull/486
