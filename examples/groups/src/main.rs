//! Node groups: backing sheets behind associated nodes.
//!
//! Groups are model data, like selection. Membership is the `group_id` on
//! each node row; each group is a `GroupData` row with authoritative bounds,
//! `collapsed` and `selected`. The editor renders a sheet per row below the
//! links and nodes, drags members with the sheet, and hides them while it is
//! collapsed. Everything else — creating, fitting, dissolving, deleting — is
//! host policy, built here from `GroupLogic`.

use slint::{Color, Model, ModelRc, SharedString, VecModel};
use slint_node_editor::{
    selection, validate_and_normalize_link, wire_groups, wire_node_editor, wire_selection, Bounds,
    Dragged, GroupData, GroupLogic, GroupMember, LinkData, LinkModel, LinkPath, MinimapNode,
    MovableNode, NoDuplicatesValidator, NodeEditorSetup,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

slint::include_modules!();

/// Node size as `SimpleNode` in `ui/groups.slint` draws it. Knowing it lets
/// the host fit a sheet from its rows synchronously, inside a drag commit,
/// without waiting for the geometry reports that follow the model change.
const NODE_WIDTH: f32 = 180.0;
const NODE_HEIGHT: f32 = 64.0;
/// Space between a sheet's edge and its outermost member.
const SHEET_PADDING: f32 = 28.0;

impl MovableNode for NodeData {
    fn id(&self) -> i32 {
        self.id
    }
    fn x(&self) -> f32 {
        self.x
    }
    fn y(&self) -> f32 {
        self.y
    }
    fn selected(&self) -> bool {
        self.selected
    }
    fn set_x(&mut self, x: f32) {
        self.x = x;
    }
    fn set_y(&mut self, y: f32) {
        self.y = y;
    }
}

impl GroupMember for NodeData {
    fn group_id(&self) -> i32 {
        self.group_id
    }
    fn set_group_id(&mut self, group_id: i32) {
        self.group_id = group_id;
    }
}

fn node(id: i32, title: &str, x: f32, y: f32, group_id: i32, color: Color) -> NodeData {
    NodeData {
        id,
        title: SharedString::from(title),
        x,
        y,
        selected: false,
        group_id,
        color,
    }
}

fn link(id: i32, from_node: i32, to_node: i32) -> LinkData {
    LinkData {
        id,
        start_pin_id: from_node * 2 + 1,
        end_pin_id: to_node * 2,
        color: Color::from_rgb_u8(150, 170, 200),
        line_width: 2.0,
        status: -1,
        selected: false,
    }
}

/// The sheet that encloses `members` at their current row positions, with
/// room for the sheet's header band above them.
fn sheet_around(nodes: &VecModel<NodeData>, members: &[i32], header: f32) -> Option<Bounds> {
    let rects = (0..nodes.row_count())
        .filter_map(|i| nodes.row_data(i))
        .filter(|n| members.contains(&n.id))
        .map(|n| (n.x, n.y, NODE_WIDTH, NODE_HEIGHT));
    Bounds::enclosing(rects, SHEET_PADDING).map(|b| Bounds {
        y: b.y - header,
        height: b.height + header,
        ..b
    })
}

/// Refit `group_id` around its members, or dissolve it when none are left.
fn refit_or_dissolve(
    nodes: &VecModel<NodeData>,
    groups: &VecModel<GroupData>,
    window: &MainWindow,
    header: f32,
    group_id: i32,
) {
    let members = GroupLogic::members(nodes, group_id);
    if members.is_empty() {
        GroupLogic::dissolve_group(nodes, groups, group_id);
        window
            .global::<NodeEditorInternalCallbacks>()
            .invoke_remove_group(group_id);
    } else if let Some(bounds) = sheet_around(nodes, &members, header) {
        GroupLogic::set_bounds(groups, group_id, bounds);
    }
}

/// The window plus what the tests need to drive it: `main` only runs the
/// window, so the models and the header height are read by tests alone.
struct App {
    window: MainWindow,
    #[cfg(test)]
    nodes: Rc<VecModel<NodeData>>,
    #[cfg(test)]
    groups: Rc<VecModel<GroupData>>,
    /// The sheet header height, read from `GroupStyleDefaults` so the fit
    /// arithmetic and the drawn header share one number.
    #[cfg(test)]
    header: f32,
}

fn build_app() -> App {
    let window = MainWindow::new().unwrap();
    let header = window.global::<GroupStyleDefaults>().get_header_height();

    let blue = Color::from_rgb_u8(90, 138, 208);
    let green = Color::from_rgb_u8(96, 176, 120);
    let amber = Color::from_rgb_u8(220, 160, 70);
    let nodes = Rc::new(VecModel::from(vec![
        node(1, "Camera", 120.0, 180.0, 1, blue),
        node(2, "Microphone", 120.0, 300.0, 1, blue),
        node(3, "Denoise", 460.0, 180.0, 2, green),
        node(4, "Colour grade", 460.0, 300.0, 2, green),
        node(5, "Mix", 460.0, 420.0, 2, green),
        node(6, "Encoder", 820.0, 300.0, 0, amber),
        node(7, "Preview", 820.0, 440.0, 0, amber),
    ]));
    let group = |id: i32, title: &str, color: Color| {
        let bounds = sheet_around(&nodes, &GroupLogic::members(&*nodes, id), header)
            .expect("group has members");
        GroupData {
            id,
            title: SharedString::from(title),
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            color,
            collapsed: false,
            selected: false,
        }
    };
    let groups = Rc::new(VecModel::from(vec![
        group(1, "Inputs", Color::from_rgb_u8(80, 130, 210)),
        group(2, "Processing", Color::from_rgb_u8(80, 170, 110)),
    ]));
    let links = Rc::new(VecModel::from(vec![
        link(1, 1, 3),
        link(2, 3, 4),
        link(3, 2, 5),
        link(4, 4, 6),
        link(5, 5, 6),
        link(6, 4, 7),
    ]));
    window.set_nodes(ModelRc::from(nodes.clone()));
    window.set_groups(ModelRc::from(groups.clone()));
    window.set_links(ModelRc::from(links.clone()));
    let next_id = Rc::new(RefCell::new(8));

    // The status line and the minimap follow the rows: sheets (a collapsed
    // one by its band) and the nodes not hidden by a collapsed group.
    let minimap = Rc::new(VecModel::<MinimapNode>::default());
    window.set_minimap_nodes(ModelRc::from(minimap.clone()));
    let refresh = {
        let window = window.as_weak();
        let nodes = nodes.clone();
        let groups = groups.clone();
        Rc::new(move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            let groups: Vec<GroupData> = (0..groups.row_count())
                .filter_map(|i| groups.row_data(i))
                .collect();
            let collapsed: HashSet<i32> = groups
                .iter()
                .filter(|g| g.collapsed)
                .map(|g| g.id)
                .collect();
            let nodes: Vec<NodeData> = (0..nodes.row_count())
                .filter_map(|i| nodes.row_data(i))
                .collect();
            let visible = nodes.iter().filter(|n| !collapsed.contains(&n.group_id));
            window.set_status(
                format!(
                    "{} nodes in {} groups, {} hidden",
                    nodes.len(),
                    groups.len(),
                    nodes.len() - visible.clone().count()
                )
                .into(),
            );

            let sheets = groups.iter().map(|g| MinimapNode {
                id: -g.id,
                x: g.x,
                y: g.y,
                width: g.width,
                height: if g.collapsed { header } else { g.height },
                color: g.color.with_alpha(0.35),
            });
            let boxes = visible.map(|n| MinimapNode {
                id: n.id,
                x: n.x,
                y: n.y,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
                color: n.color,
            });
            let rows: Vec<MinimapNode> = sheets.chain(boxes).collect();
            if let Some(b) =
                Bounds::enclosing(rows.iter().map(|m| (m.x, m.y, m.width, m.height)), 0.0)
            {
                window.set_graph_min_x(b.x);
                window.set_graph_min_y(b.y);
                window.set_graph_max_x(b.x + b.width);
                window.set_graph_max_y(b.y + b.height);
            }
            minimap.set_vec(rows);
        })
    };

    // One movement rule for both gestures. After a node drag, sheets whose
    // members moved refit around them (host policy: the requirements leave
    // "member dragged past the edge" open, and growing is the friendliest
    // reading); a sheet drag keeps its size.
    let setup = NodeEditorSetup::with_drag_commit({
        let nodes = nodes.clone();
        let groups = groups.clone();
        let refresh = refresh.clone();
        move |dragged, dx, dy| {
            GroupLogic::commit_drag(&nodes, &groups, dragged, dx, dy);
            if let Dragged::Node(id) = dragged {
                let moving = GroupLogic::moving_groups(&*groups, dragged);
                let touched: HashSet<i32> = (0..nodes.row_count())
                    .filter_map(|i| nodes.row_data(i))
                    .filter(|n| n.group_id != 0 && (n.id == id || n.selected))
                    .map(|n| n.group_id)
                    .filter(|g| !moving.contains(g))
                    .collect();
                for group_id in touched {
                    let members = GroupLogic::members(&*nodes, group_id);
                    if let Some(bounds) = sheet_around(&nodes, &members, header) {
                        GroupLogic::set_bounds(&groups, group_id, bounds);
                    }
                }
            }
            refresh();
        }
    });
    wire_node_editor!(window, setup);
    wire_selection!(window, setup, nodes, links, groups);
    wire_groups!(window, groups, nodes);
    // The sheet's toggle changes the rows through `wire_groups!`; the host
    // also wants its status and minimap to follow, so it re-installs the
    // handler (the last installed handler wins) with a refresh on the end.
    window
        .global::<NodeEditorEvents>()
        .on_group_collapse_requested({
            let nodes = nodes.clone();
            let groups = groups.clone();
            let refresh = refresh.clone();
            move |group_id, collapsed| {
                GroupLogic::set_collapsed(&groups, &nodes, group_id, collapsed, |n| {
                    &mut n.selected
                });
                refresh();
            }
        });
    refresh();

    // Link picking and creation, as in the other examples.
    let ctrl = setup.controller().clone();
    window.on_compute_link_at({
        let ctrl = ctrl.clone();
        let links = links.clone();
        move |x, y| {
            let rows = (0..links.row_count())
                .filter_map(|i| links.row_data(i))
                .map(|l| (l.id, l.start_pin_id, l.end_pin_id));
            ctrl.cache().borrow().find_bezier_link_at_world(
                x,
                y,
                rows,
                ctrl.screen_distance_to_world(8.0),
                ctrl.bezier_offset(),
                20,
            )
        }
    });
    window.on_link_requested({
        let links = links.clone();
        let next_id = next_id.clone();
        let ctrl = ctrl.clone();
        move |start_pin, end_pin| {
            let existing: Vec<LinkData> = (0..links.row_count())
                .filter_map(|i| links.row_data(i))
                .collect();
            let normalized = validate_and_normalize_link(
                start_pin,
                end_pin,
                &ctrl.cache().borrow(),
                &existing,
                2, // PinTypes.output
                &NoDuplicatesValidator,
            );
            if let Ok(normalized) = normalized {
                let id = *next_id.borrow();
                *next_id.borrow_mut() += 1;
                links.push(LinkData {
                    id,
                    start_pin_id: normalized.output_pin_id,
                    end_pin_id: normalized.input_pin_id,
                    color: Color::from_rgb_u8(150, 170, 200),
                    line_width: 2.0,
                    status: -1,
                    selected: false,
                });
            }
        }
    });

    // --- Group commands ---------------------------------------------------

    window.on_group_selection({
        let nodes = nodes.clone();
        let groups = groups.clone();
        let next_id = next_id.clone();
        let refresh = refresh.clone();
        move || {
            let members = GroupLogic::selected_ungrouped(&*nodes);
            let Some(bounds) = sheet_around(&nodes, &members, header) else {
                return;
            };
            let id = *next_id.borrow();
            *next_id.borrow_mut() += 1;
            GroupLogic::assign_members(&nodes, &members, id);
            selection::clear_selection(&*groups, |g| &mut g.selected);
            groups.push(GroupData {
                id,
                title: SharedString::from(format!("Group {}", id)),
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
                color: Color::from_rgb_u8(170, 110, 200),
                collapsed: false,
                selected: true,
            });
            refresh();
        }
    });

    window.on_ungroup_selection({
        let window = window.as_weak();
        let nodes = nodes.clone();
        let groups = groups.clone();
        let refresh = refresh.clone();
        move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            for id in selection::selected_rows(&*groups, |g| g.id, |g| g.selected) {
                GroupLogic::set_collapsed(&groups, &nodes, id, false, |n| &mut n.selected);
                GroupLogic::dissolve_group(&nodes, &groups, id);
                window
                    .global::<NodeEditorInternalCallbacks>()
                    .invoke_remove_group(id);
            }
            refresh();
        }
    });

    window.on_leave_group({
        let window = window.as_weak();
        let nodes = nodes.clone();
        let groups = groups.clone();
        let refresh = refresh.clone();
        move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            let leaving =
                selection::selected_rows(&*nodes, |n| n.id, |n| n.selected && n.group_id != 0);
            let touched: HashSet<i32> = leaving
                .into_iter()
                .filter_map(|id| GroupLogic::remove_member(&nodes, id))
                .collect();
            for group_id in touched {
                refit_or_dissolve(&nodes, &groups, &window, header, group_id);
            }
            refresh();
        }
    });

    window.on_toggle_collapse({
        let nodes = nodes.clone();
        let groups = groups.clone();
        let refresh = refresh.clone();
        move || {
            for id in selection::selected_rows(&*groups, |g| g.id, |g| g.selected) {
                let collapsed = GroupLogic::is_collapsed(&*groups, id);
                GroupLogic::set_collapsed(&groups, &nodes, id, !collapsed, |n| &mut n.selected);
            }
            refresh();
        }
    });

    window.on_fit_groups({
        let nodes = nodes.clone();
        let groups = groups.clone();
        let refresh = refresh.clone();
        move || {
            for id in selection::selected_rows(&*groups, |g| g.id, |g| g.selected) {
                if let Some(bounds) =
                    sheet_around(&nodes, &GroupLogic::members(&*nodes, id), header)
                {
                    GroupLogic::set_bounds(&groups, id, bounds);
                }
            }
            refresh();
        }
    });

    window.on_delete_selection({
        let window = window.as_weak();
        let nodes = nodes.clone();
        let groups = groups.clone();
        let links = links.clone();
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        move || {
            let Some(window) = window.upgrade() else {
                return;
            };
            let lifecycle = window.global::<NodeEditorInternalCallbacks>();
            // Nodes first: their links and pins go with them.
            let deleted: Vec<i32> = selection::selected_rows(&*nodes, |n| n.id, |n| n.selected);
            {
                let cache = ctrl.cache();
                let cache = cache.borrow();
                let owner = |pin: i32| cache.pin_positions.get(&pin).map(|p| p.node_id);
                for i in (0..links.row_count()).rev() {
                    let doomed = links.row_data(i).is_some_and(|l| {
                        owner(l.start_pin_id()).is_some_and(|n| deleted.contains(&n))
                            || owner(l.end_pin_id()).is_some_and(|n| deleted.contains(&n))
                    });
                    if doomed {
                        links.remove(i);
                    }
                }
            }
            let mut touched = HashSet::new();
            for i in (0..nodes.row_count()).rev() {
                let Some(n) = nodes.row_data(i) else {
                    continue;
                };
                if deleted.contains(&n.id) {
                    if n.group_id != 0 {
                        touched.insert(n.group_id);
                    }
                    nodes.remove(i);
                    lifecycle.invoke_remove_node(n.id);
                }
            }
            // Selected sheets dissolve (their members stay); a group that lost
            // its last member dissolves too, the rest refit.
            for id in selection::selected_rows(&*groups, |g| g.id, |g| g.selected) {
                GroupLogic::set_collapsed(&groups, &nodes, id, false, |n| &mut n.selected);
                GroupLogic::dissolve_group(&nodes, &groups, id);
                lifecycle.invoke_remove_group(id);
                touched.remove(&id);
            }
            for group_id in touched {
                refit_or_dissolve(&nodes, &groups, &window, header, group_id);
            }
            selection::clear_selection(&*links, |l| &mut l.selected);
            refresh();
        }
    });

    App {
        window,
        #[cfg(test)]
        nodes,
        #[cfg(test)]
        groups,
        #[cfg(test)]
        header,
    }
}

fn main() {
    build_app().window.run().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::ComponentHandle;

    fn app() -> App {
        use std::cell::Cell;
        thread_local! {
            static INITIALIZED: Cell<bool> = const { Cell::new(false) };
        }
        INITIALIZED.with(|initialized| {
            if !initialized.get() {
                i_slint_backend_testing::init_no_event_loop();
                initialized.set(true);
            }
        });
        let app = build_app();
        app.window.show().unwrap();
        slint::platform::update_timers_and_animations();
        slint::platform::update_timers_and_animations();
        app
    }

    fn select_nodes(app: &App, ids: &[i32]) {
        selection::project_selection(&*app.nodes, |n| ids.contains(&n.id), |n| &mut n.selected);
    }

    fn group_row(app: &App, id: i32) -> Option<GroupData> {
        GroupLogic::find_group(&*app.groups, id).map(|(_, g)| g)
    }

    #[test]
    fn initial_sheets_enclose_their_members_with_a_header() {
        let app = app();
        let inputs = group_row(&app, 1).unwrap();
        assert_eq!(inputs.x, 120.0 - SHEET_PADDING);
        assert_eq!(inputs.y, 180.0 - SHEET_PADDING - app.header);
        assert_eq!(inputs.width, NODE_WIDTH + 2.0 * SHEET_PADDING);
        assert_eq!(
            inputs.height,
            (300.0 + NODE_HEIGHT - 180.0) + 2.0 * SHEET_PADDING + app.header
        );
        assert_eq!(
            app.header, 32.0,
            "GroupStyleDefaults.header-height as shipped"
        );
    }

    #[test]
    fn grouping_the_selection_creates_a_selected_sheet_around_ungrouped_nodes() {
        let app = app();
        select_nodes(&app, &[6, 7, 3]);
        app.window.invoke_group_selection();

        let created = group_row(&app, 8).expect("new group");
        assert!(created.selected);
        assert_eq!(GroupLogic::members(&*app.nodes, 8), vec![6, 7]);
        assert_eq!(
            app.nodes.row_data(2).unwrap().group_id,
            2,
            "node 3 stays in Processing"
        );
        assert_eq!(created.x, 820.0 - SHEET_PADDING);
        assert!(app.window.get_status().starts_with("7 nodes in 3 groups"));
    }

    #[test]
    fn grouping_with_nothing_eligible_creates_nothing() {
        let app = app();
        select_nodes(&app, &[1]);
        app.window.invoke_group_selection();
        assert_eq!(app.groups.row_count(), 2);
    }

    #[test]
    fn leaving_refits_the_sheet_and_the_last_member_dissolves_it() {
        let app = app();
        select_nodes(&app, &[2]);
        app.window.invoke_leave_group();
        assert_eq!(app.nodes.row_data(1).unwrap().group_id, 0);
        let inputs = group_row(&app, 1).unwrap();
        assert_eq!(
            inputs.height,
            NODE_HEIGHT + 2.0 * SHEET_PADDING + app.header
        );

        select_nodes(&app, &[1]);
        app.window.invoke_leave_group();
        assert!(group_row(&app, 1).is_none(), "empty group dissolved");
        assert_eq!(app.nodes.row_data(0).unwrap().group_id, 0);
    }

    #[test]
    fn deleting_a_selected_sheet_keeps_its_members_and_deleting_all_members_dissolves_it() {
        let app = app();
        selection::project_selection(&*app.groups, |g| g.id == 1, |g| &mut g.selected);
        app.window.invoke_delete_selection();
        assert!(group_row(&app, 1).is_none());
        assert_eq!(app.nodes.row_count(), 7);
        assert_eq!(app.nodes.row_data(0).unwrap().group_id, 0);

        select_nodes(&app, &[3, 4, 5]);
        app.window.invoke_delete_selection();
        assert_eq!(app.nodes.row_count(), 4);
        assert!(
            group_row(&app, 2).is_none(),
            "group with no members left dissolved"
        );
    }

    #[test]
    fn a_member_drag_refits_its_sheet_but_a_sheet_drag_keeps_the_size() {
        let app = app();
        let before = group_row(&app, 2).unwrap();
        app.window
            .global::<NodeEditorInternalCallbacks>()
            .invoke_end_node_drag(5, 0.0, 100.0);
        let after = group_row(&app, 2).unwrap();
        assert_eq!(after.height, before.height + 100.0);
        assert_eq!(app.nodes.row_data(4).unwrap().y, 520.0);

        app.window
            .global::<NodeEditorInternalCallbacks>()
            .invoke_end_group_drag(2, 10.0, 0.0);
        let moved = group_row(&app, 2).unwrap();
        assert_eq!(
            (moved.x, moved.width, moved.height),
            (after.x + 10.0, after.width, after.height)
        );
        assert_eq!(app.nodes.row_data(2).unwrap().x, 470.0);
    }

    #[test]
    fn toggling_collapse_flips_selected_sheets_and_the_status_follows() {
        let app = app();
        selection::project_selection(&*app.groups, |g| g.id == 2, |g| &mut g.selected);
        app.window.invoke_toggle_collapse();
        assert!(group_row(&app, 2).unwrap().collapsed);
        assert!(!group_row(&app, 1).unwrap().collapsed);
        assert!(app.window.get_status().ends_with("3 hidden"));
        app.window.invoke_toggle_collapse();
        assert!(!group_row(&app, 2).unwrap().collapsed);
        assert!(app.window.get_status().ends_with("0 hidden"));
    }
}
