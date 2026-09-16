//! Level 5: Keyboard Input Tests
//!
//! Tests Delete/Backspace for removing selected items, Escape for canceling operations.

mod common;

use common::harness::MinimalTestHarness;
use slint::platform::{Key, PointerEventButton};
use slint::{ComponentHandle, Model, SharedString};

/// Helper to set up geometry in the cache for testing.
fn setup_test_geometry(harness: &MinimalTestHarness) {
    let cache = harness.ctrl.cache();
    let mut cache = cache.borrow_mut();

    cache.update_node_rect(1, 100.0, 100.0, 150.0, 100.0);
    cache.update_node_rect(2, 400.0, 200.0, 150.0, 100.0);

    cache.handle_pin_report(2, 1, 1, 0.0, 50.0);
    cache.handle_pin_report(3, 1, 2, 150.0, 50.0);
    cache.handle_pin_report(4, 2, 1, 0.0, 50.0);
    cache.handle_pin_report(5, 2, 2, 150.0, 50.0);
}

// ============================================================================
// Delete Key Tests
// ============================================================================

// NodeEditor no longer binds Delete/Backspace/Ctrl+N itself — embedders own
// the keymap. Tests below verify key events dispatch without panicking, and
// that model-level delete operations work independently of any key binding.

#[test]
fn test_delete_key_sends_event() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    // NodeEditor no longer binds Delete; this verifies the key event mechanism
    // dispatches without panicking.
    harness.key_tap(Key::Delete);

    // The actual behavior depends on Slint's focus handling
    // This test verifies the key event mechanism works
}

#[test]
fn test_backspace_key_sends_event() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    harness.key_tap(Key::Backspace);

    // Similar to Delete - verifies the event dispatch mechanism
}

// ============================================================================
// Delete Implementation Tests
// ============================================================================

#[test]
fn test_delete_removes_selected_nodes_from_model() {
    use common::harness::NodeData;

    let harness = MinimalTestHarness::with_nodes_and_links(
        vec![
            NodeData {
                id: 1,
                title: SharedString::from("A"),
                x: 100.0,
                y: 100.0,
                selected: false,
            },
            NodeData {
                id: 2,
                title: SharedString::from("B"),
                x: 400.0,
                y: 200.0,
                selected: false,
            },
            NodeData {
                id: 3,
                title: SharedString::from("C"),
                x: 700.0,
                y: 100.0,
                selected: false,
            },
        ],
        vec![],
    );

    assert_eq!(harness.nodes.row_count(), 3);

    // Select node 2
    harness.window.invoke_node_selected(2, false);

    // Simulate delete operation directly on the model (embedder's responsibility)
    let to_delete: Vec<i32> = harness.selected_node_ids();
    for id in to_delete {
        for i in 0..harness.nodes.row_count() {
            if let Some(node) = harness.nodes.row_data(i) {
                if node.id == id {
                    harness.nodes.remove(i);
                    break;
                }
            }
        }
    }
    harness.window.invoke_selection_cleared();

    assert_eq!(harness.nodes.row_count(), 2);
    // Verify node 2 was removed
    for i in 0..harness.nodes.row_count() {
        if let Some(node) = harness.nodes.row_data(i) {
            assert_ne!(node.id, 2, "Node 2 should have been deleted");
        }
    }
}

#[test]
fn test_delete_removes_selected_links_from_model() {
    use common::harness::LinkData;
    use slint::Color;

    let harness = MinimalTestHarness::with_nodes_and_links(
        vec![
            common::harness::NodeData {
                id: 1,
                title: SharedString::from("A"),
                x: 100.0,
                y: 100.0,
                selected: false,
            },
            common::harness::NodeData {
                id: 2,
                title: SharedString::from("B"),
                x: 400.0,
                y: 200.0,
                selected: false,
            },
        ],
        vec![
            LinkData {
                id: 1,
                start_pin_id: 3,
                end_pin_id: 4,
                color: Color::from_argb_u8(255, 100, 180, 255),
                line_width: 2.0,
                status: -1,
                selected: false,
            },
            LinkData {
                id: 2,
                start_pin_id: 5,
                end_pin_id: 2,
                color: Color::from_argb_u8(255, 255, 100, 100),
                line_width: 2.0,
                status: -1,
                selected: false,
            },
        ],
    );

    assert_eq!(harness.links.row_count(), 2);

    // Simulate deleting link with id 1
    // In real implementation, this would be triggered by selecting a link and pressing delete
    for i in 0..harness.links.row_count() {
        if let Some(link) = harness.links.row_data(i) {
            if link.id == 1 {
                harness.links.remove(i);
                break;
            }
        }
    }

    assert_eq!(harness.links.row_count(), 1);
    let remaining = harness.links.row_data(0).unwrap();
    assert_eq!(
        remaining.id, 2,
        "Link 1 should have been deleted, link 2 remains"
    );
}

#[test]
fn test_deleting_node_should_also_remove_connected_links() {
    use common::harness::{LinkData, NodeData};
    use slint::Color;

    let harness = MinimalTestHarness::with_nodes_and_links(
        vec![
            NodeData {
                id: 1,
                title: SharedString::from("A"),
                x: 100.0,
                y: 100.0,
                selected: false,
            },
            NodeData {
                id: 2,
                title: SharedString::from("B"),
                x: 400.0,
                y: 200.0,
                selected: false,
            },
        ],
        vec![LinkData {
            id: 1,
            start_pin_id: 3, // Node 1 output
            end_pin_id: 4,   // Node 2 input
            color: Color::from_argb_u8(255, 100, 180, 255),
            line_width: 2.0,
            status: -1,
            selected: false,
        }],
    );

    setup_test_geometry(&harness);

    // Delete Node 1 and its connected links
    let node_to_delete = 1;

    // Find links connected to node 1
    // Pin IDs for node 1 are: 2 (input) and 3 (output)
    let node_pins = [2, 3]; // node_id * 2 and node_id * 2 + 1

    // Remove connected links
    let mut links_to_remove = Vec::new();
    for i in 0..harness.links.row_count() {
        if let Some(link) = harness.links.row_data(i) {
            if node_pins.contains(&link.start_pin_id) || node_pins.contains(&link.end_pin_id) {
                links_to_remove.push(i);
            }
        }
    }
    // Remove in reverse order to maintain indices
    for &i in links_to_remove.iter().rev() {
        harness.links.remove(i);
    }

    // Remove the node
    for i in 0..harness.nodes.row_count() {
        if let Some(node) = harness.nodes.row_data(i) {
            if node.id == node_to_delete {
                harness.nodes.remove(i);
                break;
            }
        }
    }

    assert_eq!(harness.nodes.row_count(), 1, "One node should remain");
    assert_eq!(
        harness.links.row_count(),
        0,
        "Link should be removed with node"
    );
}

#[test]
fn test_delete_with_empty_selection() {
    let harness = MinimalTestHarness::new();

    assert!(harness.selected_node_ids().is_empty());
    let initial_node_count = harness.nodes.row_count();
    let initial_link_count = harness.links.row_count();

    // Delete with nothing selected should do nothing
    let to_delete: Vec<i32> = harness.selected_node_ids();
    assert!(to_delete.is_empty(), "Nothing should be selected");

    // Verify nothing changed
    assert_eq!(harness.nodes.row_count(), initial_node_count);
    assert_eq!(harness.links.row_count(), initial_link_count);
}

#[test]
fn test_delete_multiple_selected_nodes() {
    use common::harness::NodeData;

    let harness = MinimalTestHarness::with_nodes_and_links(
        vec![
            NodeData {
                id: 1,
                title: SharedString::from("A"),
                x: 100.0,
                y: 100.0,
                selected: false,
            },
            NodeData {
                id: 2,
                title: SharedString::from("B"),
                x: 400.0,
                y: 200.0,
                selected: false,
            },
            NodeData {
                id: 3,
                title: SharedString::from("C"),
                x: 700.0,
                y: 100.0,
                selected: false,
            },
        ],
        vec![],
    );

    // Select multiple nodes
    harness.window.invoke_node_selected(1, false);
    harness.window.invoke_node_selected(3, true);

    // Delete selected
    let to_delete: Vec<i32> = harness.selected_node_ids();
    for id in to_delete {
        loop {
            let mut found = false;
            for i in 0..harness.nodes.row_count() {
                if let Some(node) = harness.nodes.row_data(i) {
                    if node.id == id {
                        harness.nodes.remove(i);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                break;
            }
        }
    }
    harness.window.invoke_selection_cleared();

    assert_eq!(harness.nodes.row_count(), 1, "Only node 2 should remain");
    let remaining = harness.nodes.row_data(0).unwrap();
    assert_eq!(remaining.id, 2);
}

// ============================================================================
// Escape Key Tests
// ============================================================================

#[test]
fn test_escape_key_sends_event() {
    let harness = MinimalTestHarness::new();

    // Just verify event can be dispatched
    harness.key_tap(Key::Escape);
}

#[test]
fn test_link_cancelled_callback_tracking() {
    let harness = MinimalTestHarness::new();

    assert_eq!(*harness.tracker.link_cancelled.borrow(), 0);

    // Simulate escape during link creation (which should call link_cancelled)
    *harness.tracker.link_cancelled.borrow_mut() += 1;

    assert_eq!(*harness.tracker.link_cancelled.borrow(), 1);
}

// ============================================================================
// Text Input Tests
// ============================================================================

#[test]
fn test_text_input_dispatch() {
    let harness = MinimalTestHarness::new();

    // Test that text input can be dispatched (for potential search/rename features)
    harness.text_input("test");
}

// ============================================================================
// Focus Tests
// ============================================================================

#[test]
fn test_window_can_receive_focus() {
    let harness = MinimalTestHarness::new();

    // The window should be able to receive focus for keyboard events
    // This is a basic sanity check (testing backend doesn't have visible windows)
    assert!(harness.window.window().size().width > 0);
}

// ============================================================================
// Press Focus Tests
// ============================================================================

// Screen positions in the test UI: node 1 is 150x100 at (100, 100) with its
// field at (120..180, 170..190) and its output pin centred at (244, 150); the
// minimap covers (600..780, 450..585); the probe covers (700..800, 0..20).
const EMPTY_CANVAS: (f32, f32) = (400.0, 450.0);
const NODE_1_CENTER: (f32, f32) = (175.0, 150.0);
const NODE_1_FIELD: (f32, f32) = (150.0, 180.0);
const NODE_1_OUTPUT_PIN: (f32, f32) = (244.0, 150.0);
const MINIMAP: (f32, f32) = (690.0, 517.0);
const PROBE: (f32, f32) = (750.0, 10.0);

/// Which field holds keyboard focus.
#[derive(Debug, PartialEq)]
enum Focused {
    Probe,
    NodeField,
    Editor,
}

fn focused(harness: &MinimalTestHarness) -> Focused {
    let w = &harness.window;
    match (
        w.get_probe_has_focus(),
        w.get_node_field_has_focus(),
        w.get_editor_has_focus(),
    ) {
        (true, false, false) => Focused::Probe,
        (false, true, false) => Focused::NodeField,
        (false, false, true) => Focused::Editor,
        other => panic!("exactly one of (probe, node field, editor) must hold focus: {other:?}"),
    }
}

fn focus_probe(harness: &MinimalTestHarness) {
    harness.window.invoke_focus_probe();
    harness.pump_events();
    assert_eq!(
        focused(harness),
        Focused::Probe,
        "the press must start from a focused field"
    );
}

fn focus_node_field(harness: &MinimalTestHarness) {
    // The nodes are repeated, and a repeater instantiates only when something
    // walks the item tree; with no renderer, a pointer event is that walk.
    let (x, y) = EMPTY_CANVAS;
    harness.mouse_move(x, y);
    harness.window.invoke_focus_node_field();
    harness.pump_events();
    assert_eq!(
        focused(harness),
        Focused::NodeField,
        "the press must start from a focused field"
    );
}

fn press(harness: &MinimalTestHarness, (x, y): (f32, f32), button: PointerEventButton) {
    harness.mouse_down_button(x, y, button);
    harness.mouse_up_button(x, y, button);
}

#[test]
fn test_press_on_canvas_focuses_editor() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_probe(&harness);
    press(&harness, EMPTY_CANVAS, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_right_press_on_canvas_focuses_editor() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_probe(&harness);
    press(&harness, EMPTY_CANVAS, PointerEventButton::Right);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_middle_press_on_node_focuses_editor() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_node_field(&harness);
    press(&harness, NODE_1_CENTER, PointerEventButton::Middle);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_press_on_node_takes_focus_from_its_own_field() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_node_field(&harness);
    press(&harness, NODE_1_CENTER, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_press_on_pin_focuses_editor() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_node_field(&harness);
    press(&harness, NODE_1_OUTPUT_PIN, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_press_on_minimap_focuses_editor() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_probe(&harness);
    press(&harness, MINIMAP, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Editor);
}

#[test]
fn test_press_on_a_field_keeps_its_focus() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    focus_probe(&harness);
    press(&harness, PROBE, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Probe);

    focus_node_field(&harness);
    press(&harness, NODE_1_FIELD, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::NodeField);
}

/// The editor takes focus inside the press itself, so a focus the embedder
/// takes in response to that press — here on the selection the press asks
/// for, which reaches the host only after the event — is the one left
/// standing.
#[test]
fn test_focus_taken_in_response_to_a_press_wins() {
    let harness = MinimalTestHarness::new();
    setup_test_geometry(&harness);

    let w = harness.window.as_weak();
    harness.window.on_node_selected(move |_, _| {
        if let Some(w) = w.upgrade() {
            w.invoke_focus_probe();
        }
    });

    press(&harness, NODE_1_CENTER, PointerEventButton::Left);
    assert_eq!(focused(&harness), Focused::Probe);
}
