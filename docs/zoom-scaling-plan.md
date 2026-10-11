# Zoom-dependent detail

The [zoom-stress-test example](../examples/zoom-stress-test) demonstrates
application-owned level of detail (LOD) with text, numeric inputs, toggles,
sliders, and displays. Run `cargo run -p zoom-stress-test`.

| Zoom | Content |
|---|---|
| Above 0.5 | Full widgets |
| Above 0.25, up to 0.5 | Simplified title, pins, and optional summary |
| Up to 0.25 | Small coloured box and title; pins hidden |

The node components read `ViewportState.zoom` and choose conditional content
and **world-space** dimensions. The world container applies zoom once; do not
multiply node dimensions by zoom again. The 80×40 minimum-detail dimensions
are world units, so at 0.1 zoom the box is 8×4 logical pixels. They do not
provide a minimum screen-size guarantee.

`NodeEditor` exposes suggested `lod-full-threshold`,
`lod-simplified-threshold`, `min-node-width`, and `min-node-height` values. Hosts
pass them into node delegates; the editor does not apply LOD automatically.
The [configuration example](../examples/configuration) demonstrates that wiring.

Hidden pins remain routing endpoints for existing links but cannot be picked.
Links retain their normal routing at every LOD. This example does not implement
selection or commit node drags; a transient drag preview returns to the model
position on release. Widget callbacks log edits rather than persisting them
into the host's model. It is a rendering demonstration, not an editing template.
Use [minimal](../examples/minimal) or the [quick start](integration-guide.md)
for selection and drag commits.

Use Ctrl/Command+scroll to zoom and ordinary scroll or middle-button drag to
pan. Preset buttons preserve the graph point at the viewport centre. The
example clamps zoom to 0.1–3.0. At lower detail, widgets are deliberately removed;
users zoom in to edit them.

LOD reduces the content rendered inside each node. It does not provide
viewport culling, virtualization, or a measured frame-time guarantee. The
2,500-node [sugiyama-stress-test](../examples/sugiyama-stress-test) separately
exercises larger graph layout.
