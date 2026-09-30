# Changelog

## Unreleased

- `sugiyama_layout` returns top-left corners as documented; it returned
  rust-sugiyama's centres, so cards of unequal size overlapped. The layout
  starts at the origin, and each component's first rank starts at 0 on the
  layer axis.
- `SugiyamaConfig::rank_alignment` aligns a rank's nodes on their leading edge
  (`Start`) or their centre (`Center`, the default).
- `LinkData.accent` indexes `NodeEditor.link-accent-colors` from 1, a palette
  the application binds, and colours a link that has no known status; 0, the
  default, is no accent, and an index outside the palette leaves the link its
  `color`. The rule is `LinkColor.of`, exported for hosts that draw their own
  links. A new field on `LinkData`: a Rust struct literal naming every field
  must add it or end in `..Default::default()`; `LinkData::new`'s signature and
  the link it builds are unchanged.
- `LinkData.dimmed` draws a link faded to `NodeEditor.dimmed-link-alpha`, over
  whatever colour its status, accent or `color` gives it. A new field on
  `LinkData`, defaulting to `false`.

## 1.0.1 — 2026-09-17

Documentation-only patch. The library API and implementation are unchanged.

- Replace the broken README demo attachment with the verified GitHub release
  video and use explicit, versioned screenshot URLs on crates.io and docs.rs.
- Remove pre-publication setup instructions from the published README and guide.

## 1.0.0 — 2026-09-16

First stable release. Requires Slint 1.18.0 or newer and Rust 1.92 or newer.

- Distribute Slint components through the `@nodeeditor` library module.
  Consumers enable `experimental-module-builds` on slint-build and require
  Rust 1.92. Slint 1.18.0 is the minimum supported registry release.
- Routes update after programmatic geometry changes and drag commits; removal
  retires node and pin geometry. Picking uses the rendered world-space curve.
- Normalize connection direction before topology validation and choose the
  nearest eligible pin deterministically.
- Make layout input ordering and disconnected-component packing deterministic.
- Test public consumer gestures and configured marquee behavior.
- Focus the editor on canvas, node, pin, minimap, and reserved marquee presses,
  while preserving embedded text input and application-directed focus changes.
  Adapted from Olivier de Gaalon's focus fix.
- Reconcile rejected and snapped drag commits to the synchronous host model.
- Reject invalid controller viewport updates and require both normalization
  endpoints to exist. Add LinkData::new for explicitly colored links and
  standard Error support for ValidationError.

### Migration from earlier git snapshots

Windows using `wire_node_editor!` must expose
`public function focus-editor() { editor.focus(); }`. Manual integrations must
connect `NodeEditorInternalCallbacks.take-editor-focus` synchronously to that
function. See the integration guide for the complete window interface.

`Link.path-commands` is replaced by `Link.geometry: LinkPath`, containing commands
relative to its bounding box. Custom routes must supply that box.
`compute_link_path_callback` now takes a LinkPath constructor.

Use NodeEditorComputations for computations and NodeEditorEvents for BaseNode
double-clicks. Use the current quick-start macros and generated global exports;
removed component callbacks are not compatibility aliases. Selection remains
host-owned and must be projected synchronously before a drag proceeds.

### Release limitations

Independent editors in one window, complete keyboard accessibility, and
measured large-graph performance remain future work. See CONTRIBUTING.md for configuration coverage.
