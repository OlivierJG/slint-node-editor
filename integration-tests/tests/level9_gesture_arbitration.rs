//! Gesture arbitration and cancellation across editor input surfaces.

mod common;

use common::harness::{
    BoxSelectionModifier, DragState, LinkCreation, LinkCreationState, MinimalTestHarness,
    NodeEditorEvents, NodeEditorInternalCallbacks,
};
use slint::{platform::Key, ComponentHandle, Model};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Copy, Debug)]
enum Surface {
    Background,
    Link,
    Node,
    Pin,
    EmbeddedControl,
}

const SURFACES: [Surface; 5] = [
    Surface::Background,
    Surface::Link,
    Surface::Node,
    Surface::Pin,
    Surface::EmbeddedControl,
];

fn realize(harness: &MinimalTestHarness) {
    harness.window.show().unwrap();
    harness.pump_events();
    harness.pump_events();
    harness.window.invoke_focus_editor();
}

fn surface_point(harness: &MinimalTestHarness, surface: Surface) -> (f32, f32) {
    match surface {
        Surface::Background => (50.0, 50.0),
        Surface::Link => {
            let output = harness.pin_position(3).expect("node 1 output pin");
            let input = harness.pin_position(4).expect("node 2 input pin");
            ((output.0 + input.0) / 2.0, (output.1 + input.1) / 2.0)
        }
        Surface::Node => (120.0, 180.0),
        Surface::Pin => harness.pin_position(3).expect("node 1 output pin"),
        Surface::EmbeddedControl => (180.0, 124.0),
    }
}

fn assert_forced_marquee(modifier: BoxSelectionModifier, key: Key, surface: Surface) {
    let harness = MinimalTestHarness::new();
    harness.window.set_box_selection_modifier(modifier);
    realize(&harness);

    let start = surface_point(&harness, surface);
    harness.key_press(key);
    harness.mouse_down(start.0, start.1);
    assert!(
        harness.window.get_is_selecting(),
        "{modifier:?} did not reserve {surface:?}"
    );
    assert!(!harness.window.global::<DragState>().get_is_dragging());
    assert!(!harness.window.get_is_creating_link());

    harness.mouse_move(650.0, 450.0);
    assert!(harness.window.get_is_selecting());
    harness.mouse_up(650.0, 450.0);
    assert!(!harness.window.get_is_selecting());
    harness.key_release(key);
}

#[test]
fn configured_modifier_reserves_every_input_surface_for_marquee() {
    for (modifier, key) in [
        (BoxSelectionModifier::Ctrl, Key::Control),
        (BoxSelectionModifier::Alt, Key::Alt),
        (BoxSelectionModifier::Shift, Key::Shift),
    ] {
        for surface in SURFACES {
            assert_forced_marquee(modifier, key, surface);
        }
    }
}

#[test]
fn command_is_the_platform_alias_for_the_ctrl_marquee_setting() {
    for surface in SURFACES {
        assert_forced_marquee(BoxSelectionModifier::Ctrl, Key::Meta, surface);
    }
}

#[test]
fn node_and_pin_forward_reserved_gestures_when_modifier_press_was_not_captured() {
    for surface in [Surface::Node, Surface::Pin] {
        let harness = MinimalTestHarness::new();
        realize(&harness);
        harness.window.invoke_clear_editor_focus();

        let point = surface_point(&harness, surface);
        harness.key_press(Key::Control);
        harness.mouse_down(point.0, point.1);
        harness.mouse_move(650.0, 450.0);
        assert!(
            harness.window.get_is_selecting(),
            "{surface:?} did not forward the reserved gesture"
        );
        assert!(!harness.window.get_is_creating_link());
        assert!(!harness.window.global::<DragState>().get_is_dragging());
        harness.mouse_up(650.0, 450.0);
        harness.key_release(Key::Control);
        assert_idle(&harness);
    }
}

#[test]
fn ordinary_embedded_control_input_survives_every_configuration() {
    for modifier in [
        BoxSelectionModifier::None,
        BoxSelectionModifier::Ctrl,
        BoxSelectionModifier::Alt,
        BoxSelectionModifier::Shift,
    ] {
        let harness = MinimalTestHarness::new();
        harness.window.set_box_selection_modifier(modifier);
        realize(&harness);

        let point = surface_point(&harness, Surface::EmbeddedControl);
        harness.click(point.0, point.1);
        assert_eq!(
            harness.window.get_embedded_control_click_count(),
            1,
            "ordinary control click lost under {modifier:?} configuration"
        );
        assert!(!harness.window.get_is_selecting());
    }
}

#[test]
fn none_configuration_preserves_each_surfaces_normal_gesture() {
    let background = MinimalTestHarness::new();
    background
        .window
        .set_box_selection_modifier(BoxSelectionModifier::None);
    realize(&background);
    let point = surface_point(&background, Surface::Background);
    background.mouse_down(point.0, point.1);
    assert!(background.window.get_is_selecting());
    background.mouse_up(300.0, 300.0);

    let link = MinimalTestHarness::new();
    link.window
        .set_box_selection_modifier(BoxSelectionModifier::None);
    realize(&link);
    let point = surface_point(&link, Surface::Link);
    link.click(point.0, point.1);
    assert_eq!(link.selected_link_ids(), vec![1]);
    assert!(!link.window.get_is_selecting());

    let node = MinimalTestHarness::new();
    node.window
        .set_box_selection_modifier(BoxSelectionModifier::None);
    realize(&node);
    let point = surface_point(&node, Surface::Node);
    node.mouse_down(point.0, point.1);
    node.mouse_move(point.0 + 40.0, point.1 + 30.0);
    assert!(node.window.global::<DragState>().get_is_dragging());
    node.mouse_up(point.0 + 40.0, point.1 + 30.0);
    assert_eq!(node.nodes.row_data(0).unwrap().x, 140.0);

    let pin = MinimalTestHarness::new();
    pin.window
        .set_box_selection_modifier(BoxSelectionModifier::None);
    realize(&pin);
    let point = surface_point(&pin, Surface::Pin);
    pin.mouse_down(point.0, point.1);
    assert!(pin.window.get_is_creating_link());
    pin.pointer_exit();

    let control = MinimalTestHarness::new();
    control
        .window
        .set_box_selection_modifier(BoxSelectionModifier::None);
    realize(&control);
    let point = surface_point(&control, Surface::EmbeddedControl);
    control.click(point.0, point.1);
    assert_eq!(control.window.get_embedded_control_click_count(), 1);
}

fn start_marquee(harness: &MinimalTestHarness) {
    let point = surface_point(harness, Surface::Background);
    harness.mouse_down(point.0, point.1);
    harness.mouse_move(300.0, 300.0);
    assert!(harness.window.get_is_selecting());
}

fn start_node_drag(harness: &MinimalTestHarness) {
    let point = surface_point(harness, Surface::Node);
    harness.mouse_down(point.0, point.1);
    harness.mouse_move(point.0 + 40.0, point.1 + 30.0);
    assert!(harness.window.global::<DragState>().get_is_dragging());
}

fn start_link(harness: &MinimalTestHarness) {
    let point = surface_point(harness, Surface::Pin);
    harness.mouse_down(point.0, point.1);
    harness.mouse_move(point.0 + 30.0, point.1 + 30.0);
    assert!(harness.window.get_is_creating_link());
}

fn assert_idle(harness: &MinimalTestHarness) {
    assert!(!harness.window.get_is_selecting(), "marquee still active");
    assert!(
        !harness.window.global::<DragState>().get_is_dragging(),
        "node drag still active"
    );
    assert!(!harness.window.get_is_creating_link(), "link still active");
    assert_eq!(
        harness.window.global::<LinkCreation>().get_state(),
        LinkCreationState::Idle
    );
    assert_eq!(
        harness.ctrl.dragged_node_id(),
        0,
        "controller drag still active"
    );
}

#[test]
fn pointer_cancel_leaves_marquee_drag_and_link_idle() {
    let marquee = MinimalTestHarness::new();
    realize(&marquee);
    start_marquee(&marquee);
    marquee.pointer_exit();
    assert_idle(&marquee);

    let drag = MinimalTestHarness::new();
    realize(&drag);
    start_node_drag(&drag);
    drag.pointer_exit();
    assert_idle(&drag);
    assert_eq!(drag.nodes.row_data(0).unwrap().x, 100.0);
    assert_eq!(drag.ctrl.cache().borrow().node_rects[&1].x, 100.0);

    let link = MinimalTestHarness::new();
    realize(&link);
    start_link(&link);
    link.pointer_exit();
    assert_idle(&link);
    assert_eq!(*link.tracker.link_cancelled.borrow(), 1);
}

#[test]
fn release_outside_leaves_marquee_drag_and_link_idle() {
    let marquee = MinimalTestHarness::new();
    realize(&marquee);
    start_marquee(&marquee);
    marquee.mouse_up(900.0, 700.0);
    assert_idle(&marquee);

    let drag = MinimalTestHarness::new();
    realize(&drag);
    start_node_drag(&drag);
    drag.mouse_up(900.0, 700.0);
    assert_idle(&drag);

    let link = MinimalTestHarness::new();
    realize(&link);
    start_link(&link);
    link.mouse_up(900.0, 700.0);
    assert_idle(&link);
}

#[test]
fn focus_loss_leaves_marquee_drag_and_link_idle() {
    let marquee = MinimalTestHarness::new();
    realize(&marquee);
    start_marquee(&marquee);
    marquee.set_window_active(false);
    assert_idle(&marquee);

    let drag = MinimalTestHarness::new();
    realize(&drag);
    start_node_drag(&drag);
    drag.set_window_active(false);
    assert_idle(&drag);

    let link = MinimalTestHarness::new();
    realize(&link);
    start_link(&link);
    link.set_window_active(false);
    assert_idle(&link);
}

#[test]
fn public_removal_hook_leaves_marquee_drag_and_link_idle() {
    let marquee = MinimalTestHarness::new();
    realize(&marquee);
    start_marquee(&marquee);
    marquee.window.invoke_cancel_interactions();
    assert_idle(&marquee);

    let drag = MinimalTestHarness::new();
    realize(&drag);
    start_node_drag(&drag);
    drag.window.invoke_cancel_interactions();
    assert_idle(&drag);
    drag.mouse_move(500.0, 400.0);
    drag.mouse_up(500.0, 400.0);
    assert_idle(&drag);
    assert_eq!(drag.nodes.row_data(0).unwrap().x, 100.0);

    let link = MinimalTestHarness::new();
    realize(&link);
    start_link(&link);
    link.window.invoke_cancel_interactions();
    assert_idle(&link);
}

#[test]
fn hiding_or_disabling_the_editor_cancels_active_input() {
    let hidden = MinimalTestHarness::new();
    realize(&hidden);
    start_marquee(&hidden);
    hidden.window.set_editor_visible(false);
    hidden.pump_events();
    assert_idle(&hidden);

    let disabled = MinimalTestHarness::new();
    realize(&disabled);
    start_node_drag(&disabled);
    disabled.window.set_editor_enabled(false);
    disabled.pump_events();
    assert_idle(&disabled);
}

#[test]
fn global_remove_node_cancels_the_consumers_drag_before_release() {
    let harness = MinimalTestHarness::new();
    realize(&harness);
    start_node_drag(&harness);

    // Keep the component alive to check the public hook itself, rather than
    // letting destruction of the model delegate hide a stale local drag.
    harness
        .window
        .global::<NodeEditorInternalCallbacks>()
        .invoke_remove_node(1);
    harness.pump_events();
    assert_idle(&harness);

    harness.mouse_move(500.0, 400.0);
    harness.mouse_up(500.0, 400.0);
    assert_idle(&harness);
    assert!(harness.tracker.node_drag_ended.borrow().is_empty());
    assert_eq!(harness.node_data(1).unwrap().x, 100.0);
    assert_eq!(harness.node_data(1).unwrap().y, 100.0);
}

#[test]
fn global_reset_graph_cancels_the_consumers_marquee_drag_and_link() {
    for start in [start_marquee, start_node_drag, start_link] {
        let harness = MinimalTestHarness::new();
        realize(&harness);
        start(&harness);
        harness
            .window
            .global::<NodeEditorInternalCallbacks>()
            .invoke_reset_graph();
        harness.pump_events();
        assert_idle(&harness);

        harness.mouse_move(500.0, 400.0);
        harness.mouse_up(500.0, 400.0);
        assert_idle(&harness);
        assert!(harness.tracker.node_drag_ended.borrow().is_empty());
        assert!(harness.tracker.link_requested.borrow().is_empty());
        assert_eq!(harness.node_data(1).unwrap().x, 100.0);
    }
}

#[test]
fn global_remove_pin_cancels_the_consumers_link_before_release() {
    let harness = MinimalTestHarness::new();
    realize(&harness);
    start_link(&harness);
    harness
        .window
        .global::<NodeEditorInternalCallbacks>()
        .invoke_remove_pin(3);
    harness.pump_events();
    assert_idle(&harness);
    let target = harness.pin_position(4).unwrap();
    harness.mouse_move(target.0, target.1);
    harness.mouse_up(target.0, target.1);
    assert_idle(&harness);
    assert!(harness.tracker.link_requested.borrow().is_empty());
    assert_eq!(*harness.tracker.link_cancelled.borrow(), 1);
}

#[test]
fn retiring_a_pin_or_its_node_cancels_a_forwarded_marquee() {
    for remove_node in [false, true] {
        let harness = MinimalTestHarness::new();
        realize(&harness);
        harness.window.invoke_clear_editor_focus();
        harness.key_press(Key::Control);
        let point = surface_point(&harness, Surface::Pin);
        harness.mouse_down(point.0, point.1);
        harness.mouse_move(650.0, 450.0);
        assert!(harness.window.get_is_selecting());

        let commits = std::rc::Rc::new(std::cell::Cell::new(0));
        harness.window.on_box_selection_committed({
            let commits = commits.clone();
            move |_, _, _, _, _| commits.set(commits.get() + 1)
        });
        let lifecycle = harness.window.global::<NodeEditorInternalCallbacks>();
        // Leave the delegate alive so the lifecycle hook must clear its grab state.
        if remove_node {
            lifecycle.invoke_remove_node(1);
        } else {
            lifecycle.invoke_remove_pin(3);
        }
        harness.pump_events();
        assert_idle(&harness);
        harness.mouse_move(660.0, 460.0);
        harness.mouse_up(660.0, 460.0);
        harness.key_release(Key::Control);
        assert_idle(&harness);
        assert_eq!(
            commits.get(),
            0,
            "a retired gesture must not commit on release"
        );
        assert!(harness.tracker.link_requested.borrow().is_empty());
    }
}

#[test]
fn node_stays_hovered_over_its_own_content() {
    let harness = MinimalTestHarness::new();
    realize(&harness);
    let hover = harness.window.global::<common::harness::HoverState>();
    for surface in [Surface::Node, Surface::EmbeddedControl, Surface::Pin] {
        let (x, y) = surface_point(&harness, surface);
        harness.mouse_move(x, y);
        harness.pump_events();
        assert_eq!(hover.get_hovered_node_id(), 1, "{surface:?}");
    }
}

#[test]
fn a_hovered_link_is_released_when_the_pointer_leaves_the_canvas() {
    let harness = MinimalTestHarness::new();
    realize(&harness);
    for leave in [Surface::Node, Surface::EmbeddedControl] {
        let (x, y) = surface_point(&harness, Surface::Link);
        harness.mouse_move(x, y);
        assert!(harness.window.get_hovered_link_id() >= 0, "the wire under the pointer");
        let (x, y) = surface_point(&harness, leave);
        harness.mouse_move(x, y);
        assert_eq!(harness.window.get_hovered_link_id(), -1, "{leave:?}");
    }
    let (x, y) = surface_point(&harness, Surface::Link);
    harness.mouse_move(x, y);
    harness.pointer_exit();
    assert_eq!(harness.window.get_hovered_link_id(), -1, "out of the window");
}

/// A committed box as `(x, y, width, height)`.
type BoxRect = (f32, f32, f32, f32);

/// Every box committed through the editor.
fn record_box_commits(harness: &MinimalTestHarness) -> Rc<RefCell<Vec<BoxRect>>> {
    let commits = Rc::new(RefCell::new(Vec::new()));
    harness.window.on_box_selection_committed({
        let commits = commits.clone();
        move |x, y, width, height, _| commits.borrow_mut().push((x, y, width, height))
    });
    commits
}

fn double_click_counter(harness: &MinimalTestHarness) -> Rc<Cell<u32>> {
    let count = Rc::new(Cell::new(0));
    harness
        .window
        .global::<NodeEditorEvents>()
        .on_node_double_clicked({
            let count = count.clone();
            move |_| count.set(count.get() + 1)
        });
    count
}

#[test]
fn a_drag_on_an_unmovable_node_acts_as_one_on_empty_canvas() {
    for box_selection_on_empty in [true, false] {
        let harness = MinimalTestHarness::new();
        harness.window.set_test_unmovable_node_id(1);
        harness
            .window
            .set_box_selection_on_empty(box_selection_on_empty);
        realize(&harness);
        let commits = record_box_commits(&harness);
        let hover = harness.window.global::<common::harness::HoverState>();

        let (x, y) = surface_point(&harness, Surface::Node);
        harness.mouse_down(x, y);
        assert_eq!(harness.selected_node_ids(), vec![1]);
        // Past the threshold but still inside the node, so its hover can be
        // checked mid-gesture.
        harness.mouse_move(x + 40.0, y - 30.0);
        assert!(!harness.window.global::<DragState>().get_is_dragging());
        assert_eq!(hover.get_hovered_node_id(), 1);
        harness.mouse_move(650.0, 450.0);
        assert_eq!(harness.window.get_is_selecting(), box_selection_on_empty);
        harness.mouse_up(650.0, 450.0);

        assert_idle(&harness);
        assert!(harness.tracker.node_drag_started.borrow().is_empty());
        assert!(harness.tracker.node_drag_ended.borrow().is_empty());
        let node = harness.node_data(1).unwrap();
        assert_eq!((node.x, node.y), (100.0, 100.0));
        assert_eq!(harness.ctrl.cache().borrow().node_rects[&1].x, 100.0);
        if box_selection_on_empty {
            // Anchored at the press, as a marquee on empty canvas is.
            assert_eq!(*commits.borrow(), vec![(x, y, 650.0 - x, 450.0 - y)]);
        } else {
            assert!(commits.borrow().is_empty());
            assert_eq!(harness.selected_node_ids(), vec![1]);
        }
    }
}

#[test]
fn sliding_off_a_selected_unmovable_node_is_not_a_click() {
    let harness = MinimalTestHarness::new();
    harness.window.set_test_unmovable_node_id(1);
    // Off, so no marquee commit replaces the selection under test.
    harness.window.set_box_selection_on_empty(false);
    realize(&harness);
    let node_b = harness.node_center(2).unwrap();
    harness.click(node_b.0, node_b.1);
    harness.shift_down();
    let (x, y) = surface_point(&harness, Surface::Node);
    harness.click(x, y);
    harness.shift_up();
    assert_eq!(harness.selected_node_ids(), vec![1, 2]);

    // A click on a selected node would narrow the selection to it on release.
    harness.drag(x, y, x + 40.0, y - 30.0);
    assert_eq!(harness.selected_node_ids(), vec![1, 2]);
    assert!(harness.tracker.node_drag_ended.borrow().is_empty());
}

#[test]
fn a_slide_on_an_unmovable_node_does_not_arm_a_double_click() {
    for box_selection_on_empty in [true, false] {
        let harness = MinimalTestHarness::new();
        harness.window.set_test_unmovable_node_id(1);
        harness
            .window
            .set_box_selection_on_empty(box_selection_on_empty);
        realize(&harness);
        let double_clicks = double_click_counter(&harness);
        let (x, y) = surface_point(&harness, Surface::Node);
        harness.drag(x, y, x + 40.0, y - 30.0);
        harness.click(x, y);
        assert_eq!(double_clicks.get(), 0, "{box_selection_on_empty}");
        // The same quick click pair does reach the host.
        harness.click(x, y);
        assert_eq!(double_clicks.get(), 1, "{box_selection_on_empty}");
    }
}

#[test]
fn dragging_a_node_carries_a_selected_unmovable_one() {
    let harness = MinimalTestHarness::new();
    harness.window.set_test_unmovable_node_id(2);
    realize(&harness);
    let node_b = harness.node_center(2).unwrap();
    harness.click(node_b.0, node_b.1);
    harness.shift_down();
    let (x, y) = surface_point(&harness, Surface::Node);
    harness.click(x, y);
    harness.shift_up();
    assert_eq!(harness.selected_node_ids(), vec![1, 2]);

    let drawn_b = || {
        (
            harness.window.get_observed_unmovable_screen_x(),
            harness.window.get_observed_unmovable_screen_y(),
        )
    };
    let before = drawn_b();
    harness.mouse_down(x, y);
    harness.mouse_move(x + 40.0, y + 30.0);
    assert_eq!(drawn_b(), (before.0 + 40.0, before.1 + 30.0));
    harness.mouse_up(x + 40.0, y + 30.0);

    assert_eq!(
        *harness.tracker.node_drag_ended.borrow(),
        vec![(40.0, 30.0)]
    );
    let node = harness.node_data(2).unwrap();
    assert_eq!((node.x, node.y), (440.0, 230.0));
}

#[test]
fn a_movable_node_beside_an_unmovable_one_still_drags() {
    let harness = MinimalTestHarness::new();
    harness.window.set_test_unmovable_node_id(2);
    realize(&harness);
    let (x, y) = surface_point(&harness, Surface::Node);
    harness.mouse_down(x, y);
    harness.mouse_move(x + 40.0, y + 30.0);
    assert!(harness.window.global::<DragState>().get_is_dragging());
    harness.mouse_up(x + 40.0, y + 30.0);
    assert_eq!(
        *harness.tracker.node_drag_ended.borrow(),
        vec![(40.0, 30.0)]
    );
    assert_eq!(harness.node_data(1).unwrap().x, 140.0);
}

#[test]
fn a_box_started_on_a_node_or_pin_is_editor_relative() {
    const EDITOR_X: f32 = 30.0;
    const EDITOR_Y: f32 = 20.0;
    let start_a_box = |harness: &MinimalTestHarness, from: Surface, modifier: bool| {
        let (x, y) = match from {
            Surface::Node => harness.world_to_screen((120.0, 180.0)),
            _ => harness.pin_position(3).expect("node 1 output pin"),
        };
        if modifier {
            harness.key_press(Key::Control);
        }
        harness.mouse_down(x, y);
        harness.mouse_move(x + 300.0, y + 200.0);
        harness.mouse_move(x + 400.0, y + 250.0);
        harness.mouse_up(x + 400.0, y + 250.0);
        if modifier {
            harness.key_release(Key::Control);
        }
        (x - EDITOR_X, y - EDITOR_Y)
    };
    // A slide on the unmovable node, then the marquee modifier on it and on its pin.
    for (from, modifier) in [
        (Surface::Node, false),
        (Surface::Node, true),
        (Surface::Pin, true),
    ] {
        let harness = MinimalTestHarness::new();
        harness.window.set_test_editor_x(EDITOR_X);
        harness.window.set_test_editor_y(EDITOR_Y);
        harness.window.set_test_unmovable_node_id(1);
        realize(&harness);
        // The modifier must reach the node, not the editor's own overlay.
        harness.window.invoke_clear_editor_focus();
        assert_eq!(
            (
                harness.window.get_editor_screen_x(),
                harness.window.get_editor_screen_y()
            ),
            (EDITOR_X, EDITOR_Y)
        );
        let commits = record_box_commits(&harness);
        let (x, y) = start_a_box(&harness, from, modifier);
        assert_eq!(
            *commits.borrow(),
            vec![(x, y, 400.0, 250.0)],
            "{from:?}, modifier {modifier}"
        );
    }
}
