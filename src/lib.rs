//! # Slint Node Editor Library
//!
//! A flexible, generic Slint component library for building visual graph editors.
//! Supports data flow diagrams, state machines, shader graphs, and any visual
//! node-based interface.
//!
//! ## Features
//!
//! - **Generic Design** - Works with any node and link data structures
//! - **Trait-Based Architecture** - Zero coupling via `NodeGeometry` and `LinkModel` traits
//! - **Callback-Based Computation** - Delegates expensive operations to Rust for performance
//! - **Opaque Pin IDs** - Library never prescribes pin encoding; applications choose
//! - **Extensible** - Easy to customize pins, styling, node types, and behaviors
//!
//! ## Quick Start
//!
//! This is a composition fragment. The
//! [compiled downstream quick start](https://github.com/tilladam/slint-node-editor/tree/main/smoke/downstream)
//! contains the Cargo manifest, build script, Slint UI and Rust host.
//!
//! ```slint
//! import { NodeEditor, BaseNode, Pin, PinTypes } from "@nodeeditor";
//!
//! export component MainWindow inherits Window {
//!     NodeEditor {
//!         // Your nodes and links here
//!     }
//! }
//! ```
//!
//! ## Core Components
//!
//! These are Slint components, imported from `@nodeeditor`. Only the top-level
//! editor has a generated Rust counterpart ([`nodeeditor::NodeEditor`]); the
//! rest exist solely in `.slint`.
//!
//! - `NodeEditor` - Main graph editor component
//! - `BaseNode` - Base component for creating custom nodes
//! - `Pin` - Connection point component
//! - `Link` - Bezier curve link component
//! - `BaseGroup` / `GroupSheet` - Backing sheet behind a group of nodes
//! - `Minimap` - Bird's-eye view component
//!
//! ## Accessibility
//!
//! Library components include accessibility roles for screen readers and MCP
//! introspection. When creating custom node components, set `accessible-label`
//! to the node's display title:
//!
//! ```slint
//! component MyNode inherits BaseNode {
//!     in property <string> title;
//!     accessible-label: title;  // override default "Node <id>"
//! }
//! ```
//!
//! ## Rust Helpers
//!
//! This crate provides Rust helper functions for common operations:
//!
//! - [`generate_grid_commands`] - Generate SVG path for grid rendering
//! - [`generate_bezier_path`] - Generate SVG path for bezier curves
//! - [`find_pin_at`] - Hit-test pins in a shared coordinate space
//! - [`find_link_route_at`] - Hit-test rendered link routes in a shared coordinate space
//! - [`GeometryCache`] - Cache node and pin geometry for fast lookups
//! - [`selection`] - Resolve selection gestures and project the result into rows
//! - [`GraphLogic`] - Helper for managing node graph state
//! - [`groups`] - Node groups: membership, movement, collapse and bounds
//!
//! ## Limitations
//!
//! **One NodeEditor per window.** The library uses Slint globals (`ViewportState`,
//! `DragState`, `NodeEditorInternalCallbacks`, etc.) for internal communication between
//! `BaseNode`/`Pin` components and the `NodeEditor`. Since Slint globals are
//! window-level singletons, only one `NodeEditor` instance per `Window` is
//! supported. Multiple editors in separate windows work fine. This limitation
//! will be lifted once Slint introduces component-scoped globals.
//!
//! See the [README](https://github.com/tilladam/slint-node-editor)
//! for detailed documentation and examples.

/// The Slint components, compiled as a library module.
///
/// A Slint consumer reaches these through `import { … } from "@nodeeditor";`
/// and never names this module directly — but the code the Slint compiler
/// generates on their side resolves to `slint_node_editor::nodeeditor::…`,
/// so the path has to exist and has to match `rust_module` in `build.rs`.
pub mod nodeeditor {
    include!(concat!(env!("OUT_DIR"), "/node-editor.rs"));
}

pub mod controller;
pub mod graph;
pub mod grid;
pub mod groups;
pub mod hit_test;
#[cfg(feature = "layout")]
pub mod layout;
pub mod links;
pub mod path;
pub mod selection;
pub mod setup;
pub mod state;
pub mod tracking;

// Re-export the data types declared in the Slint sources.
//
// A consumer's own generated code re-exports the *globals* it imports from
// `@nodeeditor` but not the structs and enums (slint 1.18's `as_library`
// forwards those only into its private inner module), so without this a
// consumer building a `LinkData` has no name to reach for.
pub use nodeeditor::{
    BoxSelectionModifier, GroupData, LinkCreationState, LinkData, LinkPath, MinimapNode,
    MinimapPosition,
};

// Re-export traits and functions
pub use grid::generate_grid_commands;
pub use hit_test::{
    distance_to_polyline, find_link_at, find_link_route_at, find_pin_at, links_in_selection_box,
    nodes_in_selection_box, BezierLinkRoute, LinkGeometry, LinkRoute, NodeGeometry, PinGeometry,
    PolylineLinkRoute, SimpleLinkGeometry, SimpleNodeGeometry,
};
pub use path::{generate_bezier_path, generate_partial_bezier_path};
pub use state::{GeometryCache, StoredPin};
// `selection` is deliberately NOT re-exported at the crate root: it is the
// host's half of selection, a family of its own, and a bare `resolve_click`
// would say nothing about what it resolves.
pub use controller::NodeEditorController;
pub use graph::{
    validate_and_normalize_link,
    BasicLinkValidator,
    CompositeValidator,
    GraphLogic,
    LinkModel,
    // Link validation framework
    LinkValidator,
    MovableNode,
    NoDuplicatesValidator,
    NormalizedLink,
    SimpleLink,
    ValidationError,
    ValidationResult,
};
pub use groups::{Bounds, Dragged, GroupLogic, GroupMember, GroupModel};
#[cfg(feature = "layout")]
pub use layout::{
    sugiyama_layout, sugiyama_layout_from_cache, Direction, NodePosition, RankAlignment,
    SugiyamaConfig,
};
pub use links::LinkManager;
pub use setup::NodeEditorSetup;
pub use tracking::GeometryTracker;

/// Wire up all NodeEditor callbacks with a single macro call.
///
/// This macro sets up default behavior for geometry tracking, computations, grid updates,
/// and keyboard focus on editor presses.
/// You can override any callback after calling this macro - the last `.on_*()` call wins.
///
/// The window must expose `grid-commands`, `width_`, `height_`, and
/// `public function focus-editor() { editor.focus(); }`.
///
/// # Generated-UI integration
///
/// This fragment assumes the generated globals and window members from the
/// [compiled quick start](https://github.com/tilladam/slint-node-editor/tree/main/smoke/downstream).
///
/// ```text
/// use slint_node_editor::{NodeEditorSetup, wire_node_editor};
///
/// let setup = NodeEditorSetup::new(|node_id, dx, dy| {
///     // Update your model
/// });
///
/// wire_node_editor!(window, setup);
///
/// // Override specific callbacks if needed:
/// // window.global::<NodeEditorComputations>().on_compute_pin_at(|x, y, radius| { ... });
/// ```
#[macro_export]
macro_rules! wire_node_editor {
    ($window:expr, $setup:expr) => {{
        // Geometry tracking
        let gc = $window.global::<NodeEditorInternalCallbacks>();
        gc.on_report_node_rect($setup.report_node_rect());
        gc.on_report_pin_position($setup.report_pin_position());
        gc.on_remove_node_from_cache($setup.remove_node());
        gc.on_remove_pin_from_cache($setup.remove_pin());
        gc.on_reset_graph_cache($setup.reset_graph());
        gc.on_start_node_drag($setup.start_node_drag());
        gc.on_cancel_node_drag($setup.cancel_node_drag());
        gc.on_end_node_drag($setup.end_node_drag());

        // Groups: sheet geometry, the collapse visibility projection, and the
        // sheet drag. The drag commit itself is installed by
        // `setup.on_group_drag_committed` or `wire_groups!`.
        gc.on_report_group_rect($setup.report_group_rect());
        gc.on_remove_group_from_cache($setup.remove_group());
        gc.on_set_node_hidden($setup.set_node_hidden());
        gc.on_start_group_drag($setup.start_group_drag());
        gc.on_end_group_drag($setup.end_group_drag());

        // Focus synchronously, before the press's consequences can focus another control.
        let w = $window.as_weak();
        gc.on_take_editor_focus(move || {
            if let Some(w) = w.upgrade() {
                w.invoke_focus_editor();
            }
        });

        // Computations
        let computations = $window.global::<NodeEditorComputations>();
        computations.on_compute_link_path($setup.controller().compute_link_path_callback(
            |commands, x, y, width, height| LinkPath {
                commands: commands.into(),
                x,
                y,
                width,
                height,
            },
        ));

        let ctrl = $setup.controller().clone();
        computations
            .on_compute_pin_at(move |x, y, radius| ctrl.cache().borrow().find_pin_at(x, y, radius));

        computations.on_compute_link_preview_path(
            |start_x, start_y, end_x, end_y, zoom, bezier_offset| {
                $crate::generate_bezier_path(start_x, start_y, end_x, end_y, zoom, bezier_offset)
                    .into()
            },
        );

        // Keep the Rust controller aligned with the component's public settings.
        $setup
            .controller()
            .set_grid_spacing(computations.get_grid_spacing());
        $setup
            .controller()
            .set_bezier_offset(computations.get_bezier_min_offset());

        let ctrl = $setup.controller().clone();
        computations.on_configuration_changed(move |grid_spacing, bezier_offset| {
            ctrl.set_grid_spacing(grid_spacing);
            ctrl.set_bezier_offset(bezier_offset);
        });

        // Viewport state and grid generation are separate public contracts:
        // viewport changes update coordinates, while request-grid-update also
        // covers resize and spacing changes. viewport-resized is left available
        // for hosts that track the editor's dimensions independently of the grid.
        let ctrl = $setup.controller().clone();
        computations.on_viewport_changed(move |zoom, pan_x, pan_y| {
            ctrl.set_viewport(zoom, pan_x, pan_y);
        });

        let ctrl = $setup.controller().clone();
        let w = $window.as_weak();
        computations.on_request_grid_update(move || {
            if let Some(w) = w.upgrade() {
                w.set_grid_commands(ctrl.generate_current_grid(w.get_width_(), w.get_height_()));
            }
        });

        // Initial grid
        let ctrl = $setup.controller().clone();
        let w = $window.as_weak();
        if let Some(w) = w.upgrade() {
            w.set_grid_commands(ctrl.generate_current_grid(w.get_width_(), w.get_height_()));
        }
    }};
}

/// Wire the editor's selection intents, using your model rows as the store.
///
/// The editor holds no selection state: it reports gestures (`node-selected`,
/// `select-link`, `selection-cleared`, `box-selection-committed`) and renders
/// whatever `selected` flag it finds on the rows. This macro closes that loop
/// without introducing a second copy of the selection — each gesture reads the
/// current set back out of the rows, resolves it through
/// [`resolve_click`] / [`resolve_box`], and projects the result in. There is
/// nothing to keep in sync, because there is only ever one record of what is
/// selected.
///
/// Rows need an `id` and a `selected` field.
///
/// Three arms: node selection alone, nodes and links together, or nodes,
/// links and groups. Applications with several node models or their own
/// gesture semantics wire the callbacks by hand out of the [`selection`]
/// module — see the `advanced` example.
///
/// [`resolve_click`]: crate::selection::resolve_click
/// [`resolve_box`]: crate::selection::resolve_box
/// [`selection`]: crate::selection
///
/// # Generated-UI integration
///
/// This fragment assumes the generated callbacks and models from the
/// [compiled quick start](https://github.com/tilladam/slint-node-editor/tree/main/smoke/downstream).
///
/// ```text
/// wire_node_editor!(window, setup);
/// wire_selection!(window, setup, nodes);
/// // …or, with selectable links:
/// wire_selection!(window, setup, nodes, links);
/// // …or, with groups as well (the window exposes `group-selected`):
/// wire_selection!(window, setup, nodes, links, groups);
/// ```
#[macro_export]
macro_rules! wire_selection {
    // A click on one kind: a plain click first clears every other kind (and
    // the clicked kind itself, which the click then replaces), shift extends.
    (@click $window:expr, $on:ident, $target:expr, [$($model:expr),*]) => {{
        $window.$on({
            let target = $target.clone();
            let clearers: Vec<Box<dyn Fn()>> = vec![$({
                let model = $model.clone();
                Box::new(move || $crate::selection::clear_selection(&*model, |r| &mut r.selected))
                    as Box<dyn Fn()>
            }),*];
            move |id, shift| {
                if !shift {
                    for clear in &clearers {
                        clear();
                    }
                }
                $crate::selection::apply_click(&*target, |r| r.id, |r| &mut r.selected, id, shift)
            }
        });
    }};

    // The background was clicked: every kind is dropped.
    (@clear $window:expr, [$($model:expr),*]) => {{
        $window.on_selection_cleared({
            let clearers: Vec<Box<dyn Fn()>> = vec![$({
                let model = $model.clone();
                Box::new(move || $crate::selection::clear_selection(&*model, |r| &mut r.selected))
                    as Box<dyn Fn()>
            }),*];
            move || {
                for clear in &clearers {
                    clear();
                }
            }
        });
    }};

    // A marquee was released: each kind selects its own hits, computed by the
    // given closure over the cache and the link rows `(id, start, end)`.
    (@box $window:expr, $setup:expr, $links:expr, [$(($model:expr, $hits:expr)),*]) => {{
        $window.on_box_selection_committed({
            let ctrl = $setup.controller().clone();
            let links = $links.clone();
            let kinds: Vec<($crate::BoxApply, $crate::BoxHits)> = vec![$({
                let model = $model.clone();
                (
                    $crate::box_apply(move |hits, shift| {
                        $crate::selection::apply_box(&*model, |r| r.id, |r| &mut r.selected, hits, shift)
                    }),
                    $crate::box_hits($hits),
                )
            }),*];
            move |x, y, w, h, shift| {
                let rows: Vec<(i32, i32, i32)> = (0..slint::Model::row_count(&*links))
                    .filter_map(|i| slint::Model::row_data(&*links, i))
                    .map(|l| (l.id, l.start_pin_id, l.end_pin_id))
                    .collect();
                let hits: Vec<Vec<i32>> = {
                    let cache = ctrl.cache();
                    let cache = cache.borrow();
                    kinds.iter().map(|(_, find)| find(&cache, x, y, w, h, &rows)).collect()
                };
                for ((apply, _), hits) in kinds.iter().zip(hits) {
                    apply(hits, shift);
                }
            }
        });
    }};

    ($window:expr, $setup:expr, $nodes:expr) => {{
        // `select-link` is deliberately not wired: the editor only emits it
        // when the host opts into `has-link-selection`, and a host doing that
        // wants the four-argument arm.
        $crate::wire_selection!(@click $window, on_node_selected, $nodes, []);
        $crate::wire_selection!(@clear $window, [$nodes]);
        $window.on_box_selection_committed({
            let nodes = $nodes.clone();
            let ctrl = $setup.controller().clone();
            move |x, y, w, h, shift| {
                let hits = ctrl.cache().borrow().nodes_in_selection_box(x, y, w, h);
                $crate::selection::apply_box(&nodes, |n| n.id, |n| &mut n.selected, hits, shift)
            }
        });
    }};

    // A plain click is exclusive across kinds — picking a node drops the
    // links and vice versa. Shift means "add to what I have", so it leaves
    // the other kind standing: the same mixed selection a shift-marquee
    // builds, which is also what a delete then acts on.
    ($window:expr, $setup:expr, $nodes:expr, $links:expr) => {{
        $crate::wire_selection!(@click $window, on_node_selected, $nodes, [$links]);
        $crate::wire_selection!(@click $window, on_select_link, $links, [$nodes]);
        $crate::wire_selection!(@clear $window, [$nodes, $links]);
        $crate::wire_selection!(@box $window, $setup, $links, [
            ($nodes, |cache, x, y, w, h, _rows| cache.nodes_in_selection_box(x, y, w, h)),
            ($links, |cache, x, y, w, h, rows| cache.links_in_selection_box(x, y, w, h, rows.iter().copied()))
        ]);
    }};

    // Groups join the same contract; a marquee selects sheets by rectangle
    // intersection like nodes, and members hidden by a collapsed group are
    // never marquee hits (the cache excludes them).
    ($window:expr, $setup:expr, $nodes:expr, $links:expr, $groups:expr) => {{
        $crate::wire_selection!(@click $window, on_node_selected, $nodes, [$links, $groups]);
        $crate::wire_selection!(@click $window, on_select_link, $links, [$nodes, $groups]);
        $crate::wire_selection!(@click $window, on_group_selected, $groups, [$nodes, $links]);
        $crate::wire_selection!(@clear $window, [$nodes, $links, $groups]);
        $crate::wire_selection!(@box $window, $setup, $links, [
            ($nodes, |cache, x, y, w, h, _rows| cache.nodes_in_selection_box(x, y, w, h)),
            ($links, |cache, x, y, w, h, rows| cache.links_in_selection_box(x, y, w, h, rows.iter().copied())),
            ($groups, |cache, x, y, w, h, _rows| cache.groups_in_selection_box(x, y, w, h))
        ]);
    }};
}

/// Boxed "apply these marquee hits" step used by `wire_selection!`.
#[doc(hidden)]
pub type BoxApply = Box<dyn Fn(Vec<i32>, bool)>;
/// Boxed "find the marquee hits of one kind" step used by `wire_selection!`.
/// Arguments: the geometry cache, the world-space box, and the link rows as
/// `(id, start_pin, end_pin)`.
#[doc(hidden)]
pub type BoxHits = Box<dyn Fn(&GeometryCache, f32, f32, f32, f32, &[(i32, i32, i32)]) -> Vec<i32>>;

#[doc(hidden)]
pub fn box_apply(f: impl Fn(Vec<i32>, bool) + 'static) -> BoxApply {
    Box::new(f)
}

#[doc(hidden)]
pub fn box_hits(
    f: impl Fn(&GeometryCache, f32, f32, f32, f32, &[(i32, i32, i32)]) -> Vec<i32> + 'static,
) -> BoxHits {
    Box::new(f)
}

/// Wire node groups for a host with one node model and one group model.
///
/// Installs the three host answers the editor needs for groups:
///
/// - `NodeEditorComputations.group-in-drag`: does a group move with the
///   current drag? Answered by [`GroupLogic::group_moves`], the same rule the
///   commit uses, so the live preview and the committed positions agree.
/// - `NodeEditorComputations.group-collapsed`: is a group collapsed? Members
///   hide themselves from the answer.
/// - `NodeEditorEvents.group-collapse-requested`: the sheet's toggle asked
///   for an explicit collapsed state; applied with [`GroupLogic::set_collapsed`].
///
/// The drag commit is not wired here: build the setup with
/// [`NodeEditorSetup::with_drag_commit`] so a node drag and a sheet drag
/// share one rule. Node rows implement [`GroupMember`], group rows
/// [`GroupModel`], and the window imports `NodeEditorEvents` and
/// `NodeEditorComputations` from `@nodeeditor`. Hosts with several node
/// models install the same three answers by hand.
///
/// # Generated-UI integration
///
/// ```text
/// let setup = NodeEditorSetup::with_drag_commit({
///     let (nodes, groups) = (nodes.clone(), groups.clone());
///     move |dragged, dx, dy| GroupLogic::commit_drag(&nodes, &groups, dragged, dx, dy)
/// });
/// wire_node_editor!(window, setup);
/// wire_selection!(window, setup, nodes, links, groups);
/// wire_groups!(window, groups, nodes);
/// ```
#[macro_export]
macro_rules! wire_groups {
    ($window:expr, $groups:expr, $nodes:expr) => {{
        let computations = $window.global::<NodeEditorComputations>();

        computations.on_group_in_drag({
            let groups = $groups.clone();
            move |group_id, dragged_node_id, dragged_group_id| {
                $crate::groups::GroupLogic::group_moves(
                    &*groups,
                    group_id,
                    $crate::groups::Dragged::from_ids(dragged_node_id, dragged_group_id),
                )
            }
        });

        computations.on_group_collapsed({
            let groups = $groups.clone();
            move |group_id, _version| $crate::groups::GroupLogic::is_collapsed(&*groups, group_id)
        });

        $window
            .global::<NodeEditorEvents>()
            .on_group_collapse_requested({
                let groups = $groups.clone();
                let nodes = $nodes.clone();
                move |group_id, collapsed| {
                    $crate::groups::GroupLogic::set_collapsed(
                        &groups,
                        &nodes,
                        group_id,
                        collapsed,
                        |n| &mut n.selected,
                    );
                }
            });
    }};
}
