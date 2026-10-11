# Editor configuration

Run `cargo run -p configuration`. This example keeps two connected nodes and
exposes grid spacing, curve offset, LOD thresholds, and minimum node dimensions
as window properties. Change their initial values in `ui/configuration.slint`
to experiment; zoom with Ctrl/Command+scroll to see the node height change.

LOD and minimum dimensions are host policies: the UI passes editor settings
into `SimpleNode`, which chooses world-space dimensions. They do not clamp a
node's screen size. Public reporting functions also demonstrate seeding geometry
without a rendered delegate.

`cargo test -p configuration` verifies configuration propagation, geometry
reporting, grid regeneration, and node double-click events. For the smallest
integration without these additional surfaces, use `minimal`.
