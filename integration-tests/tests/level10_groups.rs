//! Level 10: node groups through real pointer input.
//!
//! Groups are model data like selection: membership is `group_id` on the node
//! rows, and each group is a row with authoritative bounds, `collapsed` and
//! `selected`. Every test drives the real surface — a pointer gesture on the
//! sheet, or the intent it emits — and asserts on the rows and on the geometry
//! cache, which is where the visibility projection lands.

mod common;

use common::harness::{
    DragState, GroupData, LinkData, MinimalTestHarness, NodeEditorComputations, NodeEditorEvents,
    NodeEditorInternalCallbacks,
};
use slint::{Color, ComponentHandle, SharedString};
use slint_node_editor::GroupLogic;

const GROUP: i32 = 7;
const HEADER_HEIGHT: f32 = 32.0;

fn node(id: i32, x: f32, y: f32, group_id: i32) -> common::harness::NodeData {
    common::harness::NodeData {
        id,
        title: SharedString::from(format!("Node {id}")),
        x,
        y,
        selected: false,
        group_id,
    }
}

/// Nodes 1 and 2 (150x100) are members of group 7, node 3 stands outside. The
/// sheet is 80,60 to 580,340: both members plus padding. One internal link
/// joins the members; one boundary link leaves the group to node 3.
fn harness() -> MinimalTestHarness {
    let harness = MinimalTestHarness::with_nodes_links_and_groups(
        vec![
            node(1, 100.0, 100.0, GROUP),
            node(2, 400.0, 200.0, GROUP),
            node(3, 650.0, 420.0, 0),
        ],
        vec![
            LinkData {
                id: 1,
                start_pin_id: 3, // node 1 output
                end_pin_id: 4,   // node 2 input
                color: Color::from_argb_u8(255, 100, 180, 255),
                line_width: 2.0,
                status: -1,
                selected: false,
                ..Default::default()
            },
            LinkData {
                id: 2,
                start_pin_id: 5, // node 2 output
                end_pin_id: 6,   // node 3 input
                color: Color::from_argb_u8(255, 100, 180, 255),
                line_width: 2.0,
                status: -1,
                selected: false,
                ..Default::default()
            },
        ],
        vec![GroupData {
            id: GROUP,
            title: SharedString::from("Seven"),
            x: 80.0,
            y: 60.0,
            width: 500.0,
            height: 280.0,
            color: Color::from_argb_u8(0, 0, 0, 0),
            collapsed: false,
            selected: false,
        }],
    );
    harness.window.show().unwrap();
    settle(&harness);
    harness
}

/// Two event-loop turns: geometry reports and the collapse visibility
/// projection each land one `changed` round after the model write.
fn settle(harness: &MinimalTestHarness) {
    harness.pump_events();
    harness.pump_events();
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

fn position(harness: &MinimalTestHarness, id: i32) -> (f32, f32) {
    let node = harness.node_data(id).unwrap();
    (node.x, node.y)
}

fn cached_rect(harness: &MinimalTestHarness, id: i32) -> (f32, f32) {
    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    let rect = cache.node_rects[&id];
    (rect.x, rect.y)
}

fn sheet_corner(harness: &MinimalTestHarness) -> (f32, f32) {
    harness
        .group_point(GROUP, (8.0, 8.0))
        .expect("sheet reported")
}

fn is_dragging(harness: &MinimalTestHarness) -> bool {
    harness.window.global::<DragState>().get_is_dragging()
}

// ============================================================================
// The sheet reports itself
// ============================================================================

#[test]
fn sheet_reports_its_authoritative_rect_and_members_keep_theirs() {
    let harness = harness();
    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    let sheet = cache.group_rects[&GROUP];
    assert_eq!(
        (sheet.x, sheet.y, sheet.width, sheet.height),
        (80.0, 60.0, 500.0, 280.0)
    );
    assert!(!cache.is_node_hidden(1));
    assert_eq!(cache.node_rects[&1].x, 100.0);
}

// ============================================================================
// Movement: one rule for preview and commit
// ============================================================================

#[test]
fn sheet_drag_moves_members_live_and_commits_them_with_the_sheet() {
    let harness = harness();
    let start = sheet_corner(&harness);

    harness.mouse_down(start.0, start.1);
    harness.mouse_move(start.0 + 40.0, start.1 + 30.0);
    assert!(is_dragging(&harness));
    assert_eq!(harness.ctrl.dragged_group_id(), GROUP);
    assert_eq!(harness.ctrl.dragged_node_id(), 0);
    // Live preview: members and the sheet have already moved in the cache,
    // while the rows are untouched until release.
    assert_eq!(cached_rect(&harness, 1), (140.0, 130.0));
    assert_eq!(cached_rect(&harness, 2), (440.0, 230.0));
    assert_eq!(cached_rect(&harness, 3), (650.0, 420.0));
    assert_eq!(position(&harness, 1), (100.0, 100.0));

    harness.mouse_up(start.0 + 40.0, start.1 + 30.0);

    assert_eq!(position(&harness, 1), (140.0, 130.0));
    assert_eq!(position(&harness, 2), (440.0, 230.0));
    assert_eq!(position(&harness, 3), (650.0, 420.0), "outsider stays");
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (120.0, 90.0));
    assert_eq!(
        &*harness.tracker.group_drag_ended.borrow(),
        &[(GROUP, 40.0, 30.0)]
    );
    assert!(harness.tracker.node_drag_ended.borrow().is_empty());
    assert!(!is_dragging(&harness));
    assert_eq!(harness.ctrl.dragged_group_id(), 0);
    // Membership survives the move.
    assert_eq!(harness.node_data(1).unwrap().group_id, GROUP);
}

#[test]
fn sheet_drag_converts_screen_delta_at_non_unit_zoom() {
    let harness = harness();
    harness.window.set_zoom(2.0);
    harness.window.set_pan_x(15.0);
    harness.pump_events();
    let start = sheet_corner(&harness);

    harness.drag(start.0, start.1, start.0 + 60.0, start.1 + 20.0);

    assert_eq!(position(&harness, 1), (130.0, 110.0));
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (110.0, 70.0));
}

#[test]
fn node_drag_carries_a_selected_group_and_its_members() {
    let harness = harness();
    harness.window.invoke_group_selected(GROUP, false);
    assert_eq!(harness.selected_group_ids(), vec![GROUP]);

    // Shift keeps the group selected while node 3 joins the selection.
    let start = harness.node_center(3).unwrap();
    harness.shift_down();
    harness.mouse_down(start.0, start.1);
    harness.mouse_move(start.0 + 10.0, start.1 + 20.0);
    // Preview: the unselected member follows because its group is selected.
    assert_eq!(cached_rect(&harness, 1), (110.0, 120.0));
    harness.mouse_up(start.0 + 10.0, start.1 + 20.0);
    harness.shift_up();

    assert_eq!(position(&harness, 3), (660.0, 440.0));
    assert_eq!(position(&harness, 1), (110.0, 120.0));
    assert_eq!(position(&harness, 2), (410.0, 220.0));
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (90.0, 80.0));
    assert!(harness.tracker.group_drag_ended.borrow().is_empty());
    assert_eq!(harness.tracker.node_drag_ended.borrow().len(), 1);
}

#[test]
fn member_drag_moves_only_the_member_and_keeps_its_membership() {
    let harness = harness();
    let start = harness.node_center(1).unwrap();

    harness.drag(start.0, start.1, start.0 + 25.0, start.1 + 5.0);

    assert_eq!(position(&harness, 1), (125.0, 105.0));
    assert_eq!(position(&harness, 2), (400.0, 200.0));
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!(
        (group.x, group.y),
        (80.0, 60.0),
        "authoritative bounds untouched"
    );
    assert_eq!(harness.node_data(1).unwrap().group_id, GROUP);
}

#[test]
fn rejected_commit_restores_sheet_and_member_geometry() {
    let harness = harness();
    harness
        .window
        .global::<NodeEditorInternalCallbacks>()
        .on_end_group_drag(|_, _, _| {});
    let start = sheet_corner(&harness);

    harness.drag(start.0, start.1, start.0 + 40.0, start.1 + 30.0);
    harness.pump_events();

    assert_eq!(position(&harness, 1), (100.0, 100.0));
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (80.0, 60.0));
    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    assert_eq!(
        (cache.group_rects[&GROUP].x, cache.group_rects[&GROUP].y),
        (80.0, 60.0)
    );
    assert_eq!(
        (cache.node_rects[&1].x, cache.node_rects[&1].y),
        (100.0, 100.0)
    );
    drop(cache);
    // A second gesture must not inherit an offset from the rejected one.
    let start = sheet_corner(&harness);
    assert_close(start.0, 88.0);
    assert_close(start.1, 68.0);
}

#[test]
fn removed_member_stays_put_and_no_longer_follows() {
    let harness = harness();
    assert_eq!(GroupLogic::remove_member(&harness.nodes, 2), Some(GROUP));
    harness.pump_events();
    let start = sheet_corner(&harness);

    harness.drag(start.0, start.1, start.0 + 40.0, start.1 + 0.0);

    assert_eq!(position(&harness, 1), (140.0, 100.0));
    assert_eq!(position(&harness, 2), (400.0, 200.0));
    assert_eq!(harness.node_data(2).unwrap().group_id, 0);
    assert_eq!(GroupLogic::members(&*harness.nodes, GROUP), vec![1]);
}

// ============================================================================
// Collapse: a visibility projection
// ============================================================================

fn link_commands(harness: &MinimalTestHarness, start_pin: i32, end_pin: i32) -> String {
    harness
        .window
        .global::<NodeEditorComputations>()
        .invoke_compute_link_path(start_pin, end_pin, harness.window.get_geometry_version())
        .commands
        .to_string()
}

#[test]
fn collapsing_hides_members_from_rendering_and_every_hit_test() {
    let harness = harness();
    harness.window.invoke_node_selected(1, false);
    assert!(
        !link_commands(&harness, 3, 4).is_empty(),
        "internal link routed"
    );
    assert!(
        !link_commands(&harness, 5, 6).is_empty(),
        "boundary link routed"
    );

    harness
        .window
        .global::<NodeEditorEvents>()
        .invoke_group_collapse_requested(GROUP, true);
    settle(&harness);

    assert!(harness.group_data(GROUP).unwrap().collapsed);
    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    assert!(cache.is_node_hidden(1) && cache.is_node_hidden(2));
    assert!(!cache.is_node_hidden(3));
    assert!(cache.node_rects.contains_key(&1), "geometry kept");
    // The sheet shrinks to its header band; that is what the marquee sees.
    assert_eq!(cache.group_rects[&GROUP].height, HEADER_HEIGHT);
    // Marquee over the whole former area finds the sheet but no member.
    assert!(cache
        .nodes_in_selection_box(0.0, 0.0, 600.0, 400.0)
        .is_empty());
    assert_eq!(
        cache.groups_in_selection_box(0.0, 0.0, 600.0, 400.0),
        vec![GROUP]
    );
    drop(cache);
    // Collapsing deselected the member it hid.
    assert!(harness.selected_node_ids().is_empty());
    // Hidden pins are not pickable; links touching a hidden node have no route.
    assert_eq!(
        harness
            .window
            .global::<NodeEditorComputations>()
            .invoke_compute_pin_at(250.0, 150.0, 10.0),
        0
    );
    assert!(
        link_commands(&harness, 3, 4).is_empty(),
        "internal link hidden"
    );
    assert!(
        link_commands(&harness, 5, 6).is_empty(),
        "boundary link hidden (FR-13 deferred)"
    );
    // A press where node 1 used to be reaches the background, not the node.
    harness.click(150.0, 150.0);
    assert!(harness.selected_node_ids().is_empty());
    assert!(harness.tracker.node_drag_started.borrow().is_empty());

    // Expand: members return at their previous positions and route again.
    harness
        .window
        .global::<NodeEditorEvents>()
        .invoke_group_collapse_requested(GROUP, false);
    settle(&harness);
    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    assert!(!cache.is_node_hidden(1));
    assert_eq!(
        (cache.node_rects[&1].x, cache.node_rects[&1].y),
        (100.0, 100.0)
    );
    assert_eq!(cache.group_rects[&GROUP].height, 280.0);
    drop(cache);
    assert!(!link_commands(&harness, 3, 4).is_empty());
    assert_eq!(position(&harness, 1), (100.0, 100.0));
    assert!(
        harness.selected_node_ids().is_empty(),
        "expand restores no selection"
    );
}

#[test]
fn collapse_requests_are_idempotent_and_the_sheet_still_drags_collapsed() {
    let harness = harness();
    let events = harness.window.global::<NodeEditorEvents>();
    events.invoke_group_collapse_requested(GROUP, true);
    events.invoke_group_collapse_requested(GROUP, true);
    settle(&harness);
    assert!(harness.group_data(GROUP).unwrap().collapsed);

    // Dragging the collapsed band moves the preserved member layout with it.
    let start = harness.group_point(GROUP, (8.0, 8.0)).unwrap();
    harness.drag(start.0, start.1, start.0 + 30.0, start.1 + 10.0);

    assert_eq!(position(&harness, 1), (130.0, 110.0));
    assert_eq!(position(&harness, 2), (430.0, 210.0));
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (110.0, 70.0));
}

#[test]
fn collapsing_during_a_sheet_drag_cancels_it_without_a_commit() {
    let harness = harness();
    let start = sheet_corner(&harness);
    harness.mouse_down(start.0, start.1);
    harness.mouse_move(start.0 + 40.0, start.1 + 30.0);
    assert!(is_dragging(&harness));

    GroupLogic::set_collapsed(&harness.groups, &harness.nodes, GROUP, true, |n| {
        &mut n.selected
    });
    settle(&harness);

    assert!(!is_dragging(&harness));
    assert_eq!(harness.ctrl.dragged_group_id(), 0);
    harness.mouse_up(start.0 + 40.0, start.1 + 30.0);

    assert!(harness.tracker.group_drag_ended.borrow().is_empty());
    let group = harness.group_data(GROUP).unwrap();
    assert_eq!((group.x, group.y), (80.0, 60.0));
    assert_eq!(position(&harness, 1), (100.0, 100.0));
}

#[test]
fn membership_change_during_a_drag_cancels_it() {
    let harness = harness();
    let start = sheet_corner(&harness);
    harness.mouse_down(start.0, start.1);
    harness.mouse_move(start.0 + 40.0, start.1 + 30.0);
    assert!(is_dragging(&harness));

    GroupLogic::remove_member(&harness.nodes, 1);
    settle(&harness);

    assert!(!is_dragging(&harness));
    harness.mouse_up(start.0 + 40.0, start.1 + 30.0);
    assert!(harness.tracker.group_drag_ended.borrow().is_empty());
    assert_eq!(position(&harness, 2), (400.0, 200.0));
}

#[test]
fn removing_the_group_row_during_its_drag_cancels_it() {
    let harness = harness();
    let start = sheet_corner(&harness);
    harness.mouse_down(start.0, start.1);
    harness.mouse_move(start.0 + 40.0, start.1 + 30.0);
    assert!(is_dragging(&harness));

    assert!(GroupLogic::dissolve_group(
        &harness.nodes,
        &harness.groups,
        GROUP
    ));
    harness
        .window
        .global::<NodeEditorInternalCallbacks>()
        .invoke_remove_group(GROUP);
    harness.pump_events();

    assert!(!is_dragging(&harness));
    harness.mouse_up(start.0 + 40.0, start.1 + 30.0);
    assert!(harness.tracker.group_drag_ended.borrow().is_empty());
    assert!(!harness
        .ctrl
        .cache()
        .borrow()
        .group_rects
        .contains_key(&GROUP));
    assert_eq!(harness.node_data(1).unwrap().group_id, 0);
    assert_eq!(position(&harness, 1), (100.0, 100.0));
}

// ============================================================================
// Selection: groups join the click, shift and marquee contract
// ============================================================================

#[test]
fn sheet_click_selects_the_group_exclusively_and_shift_extends() {
    let harness = harness();
    harness.window.invoke_node_selected(3, false);
    let point = sheet_corner(&harness);

    harness.click(point.0, point.1);

    assert_eq!(harness.selected_group_ids(), vec![GROUP]);
    assert!(
        harness.selected_node_ids().is_empty(),
        "plain click is exclusive"
    );

    harness.shift_down();
    let node = harness.node_center(3).unwrap();
    harness.click(node.0, node.1);
    harness.shift_up();

    assert_eq!(harness.selected_group_ids(), vec![GROUP]);
    assert_eq!(harness.selected_node_ids(), vec![3]);

    // A member click wins over the sheet below it.
    let member = harness.node_center(1).unwrap();
    harness.click(member.0, member.1);
    assert_eq!(harness.selected_node_ids(), vec![1]);
    assert!(harness.selected_group_ids().is_empty());
}

#[test]
fn marquee_selects_the_sheet_by_intersection() {
    let harness = harness();
    // Start on empty canvas above the sheet and cross into its top-left
    // padding only: the sheet intersects, no node does.
    harness.drag(20.0, 20.0, 95.0, 90.0);

    assert_eq!(harness.selected_group_ids(), vec![GROUP]);
    assert!(harness.selected_node_ids().is_empty());

    harness.click(20.0, 500.0);
    assert!(
        harness.selected_group_ids().is_empty(),
        "background click clears"
    );
}

#[test]
fn creating_a_group_from_the_selection_excludes_members_of_other_groups() {
    let harness = harness();
    harness.window.invoke_node_selected(2, false);
    harness.window.invoke_node_selected(3, true);

    let members = GroupLogic::selected_ungrouped(&*harness.nodes);
    assert_eq!(members, vec![3], "node 2 already belongs to group 7");
    let bounds = GroupLogic::fit_bounds(&harness.ctrl.cache().borrow(), &members, 20.0).unwrap();
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (630.0, 400.0, 190.0, 140.0)
    );
    GroupLogic::assign_members(&harness.nodes, &members, 8);
    harness.groups.push(GroupData {
        id: 8,
        title: SharedString::from("Eight"),
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        color: Color::from_argb_u8(0, 0, 0, 0),
        collapsed: false,
        selected: false,
    });
    // A row pushed after the window is shown is realized on the next item
    // tree traversal; a pointer move is the cheapest way to force one here.
    harness.mouse_move(1.0, 1.0);
    settle(&harness);

    let cache = harness.ctrl.cache();
    let cache = cache.borrow();
    assert_eq!(cache.group_rects[&8].x, 630.0);
    drop(cache);
    let start = harness.group_point(8, (5.0, 5.0)).unwrap();
    harness.drag(start.0, start.1, start.0 + 10.0, start.1);
    assert_eq!(position(&harness, 3), (660.0, 420.0));
    assert_eq!(position(&harness, 2), (400.0, 200.0));
}
