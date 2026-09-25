//! Host-side group helpers.
//!
//! A group is an association of nodes rendered as a backing sheet behind its
//! members. It is not a node: it has no pins and cannot be linked. Like
//! selection, groups are model data and the editor holds none of it:
//! membership is the `group_id` on each node row ([`GroupMember`]), and each
//! group is a row with authoritative bounds, `collapsed` and `selected`
//! ([`GroupModel`], implemented for the Slint `GroupData` struct).
//!
//! [`GroupLogic`] holds the operations on those rows as plain functions over
//! models. One rule decides what a drag moves ([`GroupLogic::moves`]) whether
//! it started on a node or a sheet ([`Dragged`]); the editor asks the host the
//! same rule for its live preview. The movement and collapse contracts are
//! spelled out once, in `docs/component-reference.md` under BaseGroup.

use crate::graph::{GraphLogic, MovableNode};
use crate::hit_test::NodeGeometry;
use crate::selection::project_selection;
use crate::state::GeometryCache;
use slint::{Model, VecModel};
use std::collections::HashSet;

/// A node row that can belong to a group.
///
/// `group_id` is the node's membership: the id of its group, or 0 for none. It
/// sits on the row for the same reason `selected` does — a drag commits the
/// members of a moving group, reading them from the rows the editor renders
/// from. There are no defaults: a host that uses groups implements both, and
/// a host that does not never names this trait.
pub trait GroupMember: MovableNode {
    /// The group this node belongs to, or 0 for none.
    fn group_id(&self) -> i32;
    /// Change the node's membership.
    fn set_group_id(&mut self, group_id: i32);
}

/// A group row: authoritative bounds, collapse and selection.
///
/// Implemented for the Slint `GroupData` struct. Implement it for your own
/// row type to keep extra fields alongside.
pub trait GroupModel: Clone + 'static {
    fn id(&self) -> i32;
    /// The authoritative bounds in world coordinates.
    fn bounds(&self) -> Bounds;
    fn set_bounds(&mut self, bounds: Bounds);
    fn selected(&self) -> bool;
    fn collapsed(&self) -> bool;
    fn set_collapsed(&mut self, collapsed: bool);
}

impl GroupModel for crate::nodeeditor::GroupData {
    fn id(&self) -> i32 {
        self.id
    }
    fn bounds(&self) -> Bounds {
        Bounds {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
    fn set_bounds(&mut self, bounds: Bounds) {
        self.x = bounds.x;
        self.y = bounds.y;
        self.width = bounds.width;
        self.height = bounds.height;
    }
    fn selected(&self) -> bool {
        self.selected
    }
    fn collapsed(&self) -> bool {
        self.collapsed
    }
    fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }
}

/// What a drag gesture started on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dragged {
    /// A node drag; the value is the dragged node's id.
    Node(i32),
    /// A sheet drag; the value is the dragged group's id.
    Group(i32),
}

impl Dragged {
    /// Build from the ids the editor's drag state carries: a group drag sets
    /// the group id and leaves the node id 0.
    pub fn from_ids(dragged_node_id: i32, dragged_group_id: i32) -> Self {
        if dragged_group_id != 0 {
            Dragged::Group(dragged_group_id)
        } else {
            Dragged::Node(dragged_node_id)
        }
    }
}

/// A world-space rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Bounds {
    /// The rectangle enclosing every `(x, y, width, height)` given, grown by
    /// `padding` on every side. `None` for an empty iterator.
    ///
    /// [`GroupLogic::fit_bounds`] feeds it cached rectangles; a host that
    /// knows its node sizes can feed it model rows directly and fit a group
    /// synchronously in its drag commit, before the geometry reports arrive.
    pub fn enclosing(
        rects: impl IntoIterator<Item = (f32, f32, f32, f32)>,
        padding: f32,
    ) -> Option<Self> {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut any = false;
        for (x, y, w, h) in rects {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
            any = true;
        }
        any.then_some(Bounds {
            x: min_x - padding,
            y: min_y - padding,
            width: max_x - min_x + 2.0 * padding,
            height: max_y - min_y + 2.0 * padding,
        })
    }

    /// This rectangle shifted by a delta.
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Bounds {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

/// Operations on group rows and node membership.
pub struct GroupLogic;

impl GroupLogic {
    // ------------------------------------------------------------------
    // Movement
    // ------------------------------------------------------------------

    /// Does this group row move with a drag that started on `dragged`?
    ///
    /// Yes when its own sheet is being dragged, or when it is selected while a
    /// node or another sheet is. This is the one movement rule: the commit
    /// uses it for every group, and [`Self::group_moves`] answers the editor's
    /// live-preview question with it.
    pub fn moves<G: GroupModel>(group: &G, dragged: Dragged) -> bool {
        group.selected() || dragged == Dragged::Group(group.id())
    }

    /// [`Self::moves`] for a group looked up by id; unknown groups (and 0) do
    /// not move. This is the predicate behind
    /// `NodeEditorComputations.group-in-drag`; `wire_groups!` installs it.
    pub fn group_moves<G: GroupModel>(
        groups: &impl Model<Data = G>,
        group_id: i32,
        dragged: Dragged,
    ) -> bool {
        group_id != 0
            && Self::find_group(groups, group_id).is_some_and(|(_, g)| Self::moves(&g, dragged))
    }

    /// The ids of the groups a drag moves.
    pub fn moving_groups<G: GroupModel>(
        groups: &impl Model<Data = G>,
        dragged: Dragged,
    ) -> Vec<i32> {
        ids_where(groups, |g| Self::moves(g, dragged), G::id)
    }

    /// Commit a finished drag into one node model.
    ///
    /// A row moves if it is the dragged node, if it is selected, or if it is a
    /// member of a group that moves. Call once per node model when nodes are
    /// split across several; the dragged node lives in exactly one of them.
    pub fn commit_node_drag<T, G>(
        nodes: &VecModel<T>,
        groups: &impl Model<Data = G>,
        dragged: Dragged,
        delta_x: f32,
        delta_y: f32,
    ) where
        T: GroupMember,
        G: GroupModel,
    {
        let moving: HashSet<i32> = Self::moving_groups(groups, dragged).into_iter().collect();
        update_rows(nodes, |node| {
            let moves = dragged == Dragged::Node(node.id())
                || node.selected()
                || (node.group_id() != 0 && moving.contains(&node.group_id()));
            if moves {
                node.set_x(node.x() + delta_x);
                node.set_y(node.y() + delta_y);
            }
            moves
        });
    }

    /// Commit a finished drag into the group rows: every group that
    /// [`Self::moves`] shifts by the delta. Call once per drag.
    pub fn commit_group_drag<G: GroupModel>(
        groups: &VecModel<G>,
        dragged: Dragged,
        delta_x: f32,
        delta_y: f32,
    ) {
        update_rows(groups, |group| {
            let moves = Self::moves(group, dragged);
            if moves {
                group.set_bounds(group.bounds().offset(delta_x, delta_y));
            }
            moves
        });
    }

    /// Commit a finished drag into one node model and the group rows.
    ///
    /// The single-model convenience over [`Self::commit_node_drag`] and
    /// [`Self::commit_group_drag`]: install it with
    /// [`NodeEditorSetup::with_drag_commit`](crate::NodeEditorSetup::with_drag_commit)
    /// and both gestures share one rule.
    pub fn commit_drag<T, G>(
        nodes: &VecModel<T>,
        groups: &VecModel<G>,
        dragged: Dragged,
        delta_x: f32,
        delta_y: f32,
    ) where
        T: GroupMember,
        G: GroupModel,
    {
        Self::commit_node_drag(nodes, groups, dragged, delta_x, delta_y);
        Self::commit_group_drag(groups, dragged, delta_x, delta_y);
    }

    // ------------------------------------------------------------------
    // Collapse
    // ------------------------------------------------------------------

    /// Whether `group_id` is collapsed. Unknown groups are expanded. This is
    /// the predicate behind `NodeEditorComputations.group-collapsed`.
    pub fn is_collapsed<G: GroupModel>(groups: &impl Model<Data = G>, group_id: i32) -> bool {
        Self::find_group(groups, group_id).is_some_and(|(_, g)| g.collapsed())
    }

    /// Set a group's collapsed state. Returns whether the row changed.
    ///
    /// Collapsing deselects the members it hides, through the same row
    /// projection every other selection write uses (`selected` is the row's
    /// flag, handed back by `selected`), so no later gesture acts on invisible
    /// nodes. Expanding does not restore that selection. The sheet reacts to
    /// the row change: members re-evaluate their visibility, and an active
    /// drag is cancelled.
    pub fn set_collapsed<T, G>(
        groups: &VecModel<G>,
        nodes: &VecModel<T>,
        group_id: i32,
        collapsed: bool,
        selected: impl Fn(&mut T) -> &mut bool,
    ) -> bool
    where
        T: GroupMember,
        G: GroupModel,
    {
        let Some((index, mut group)) = Self::find_group(groups, group_id) else {
            return false;
        };
        if group.collapsed() == collapsed {
            return false;
        }
        group.set_collapsed(collapsed);
        groups.set_row_data(index, group);
        if collapsed {
            project_selection(
                nodes,
                |n| n.selected() && n.group_id() != group_id,
                selected,
            );
        }
        true
    }

    // ------------------------------------------------------------------
    // Membership
    // ------------------------------------------------------------------

    /// The ids of the nodes that belong to `group_id`, in row order.
    pub fn members<T: GroupMember>(nodes: &impl Model<Data = T>, group_id: i32) -> Vec<i32> {
        ids_where(nodes, |n| n.group_id() == group_id, T::id)
    }

    /// The selected nodes that belong to no group yet: the candidates for a
    /// new group. A node belongs to at most one group, so selected members of
    /// another group are left out rather than reassigned.
    pub fn selected_ungrouped<T: GroupMember>(nodes: &impl Model<Data = T>) -> Vec<i32> {
        ids_where(nodes, |n| n.selected() && n.group_id() == 0, T::id)
    }

    /// Make `node_ids` members of `group_id`, replacing any previous
    /// membership. Unknown ids are ignored.
    pub fn assign_members<T: GroupMember>(nodes: &VecModel<T>, node_ids: &[i32], group_id: i32) {
        update_rows(nodes, |node| {
            let assign = node_ids.contains(&node.id()) && node.group_id() != group_id;
            if assign {
                node.set_group_id(group_id);
            }
            assign
        });
    }

    /// Take `node_id` out of its group. Returns the group it left, or `None`
    /// if the node was unknown or ungrouped. The node stays where it is and no
    /// longer moves with the group. When the group is now empty the caller
    /// decides whether to keep or dissolve it ([`Self::members`]).
    pub fn remove_member<T: GroupMember>(nodes: &VecModel<T>, node_id: i32) -> Option<i32> {
        let (index, mut node) = GraphLogic::find_node_by_id(nodes, node_id, T::id)?;
        let former = node.group_id();
        if former == 0 {
            return None;
        }
        node.set_group_id(0);
        nodes.set_row_data(index, node);
        Some(former)
    }

    /// Dissolve a group: its members become ungrouped and stay where they are,
    /// and the row is removed. Returns whether a row was removed.
    ///
    /// The sheet's projected geometry is the editor's: call
    /// `NodeEditorInternalCallbacks.remove-group(id)` on the Slint side as
    /// well, as for a removed node.
    pub fn dissolve_group<T, G>(nodes: &VecModel<T>, groups: &VecModel<G>, group_id: i32) -> bool
    where
        T: GroupMember,
        G: GroupModel,
    {
        update_rows(nodes, |node| {
            let member = node.group_id() == group_id;
            if member {
                node.set_group_id(0);
            }
            member
        });
        match Self::find_group(groups, group_id) {
            Some((index, _)) => {
                groups.remove(index);
                true
            }
            None => false,
        }
    }

    // ------------------------------------------------------------------
    // Bounds
    // ------------------------------------------------------------------

    /// The rectangle enclosing the cached rectangles of `node_ids`, grown by
    /// `padding` on every side. `None` when none of them has geometry yet.
    pub fn fit_bounds<N: NodeGeometry + Copy>(
        cache: &GeometryCache<N>,
        node_ids: &[i32],
        padding: f32,
    ) -> Option<Bounds> {
        Bounds::enclosing(
            node_ids
                .iter()
                .filter_map(|id| cache.node_rects.get(id))
                .map(|node| node.rect()),
            padding,
        )
    }

    /// Write `bounds` into a group row. Returns whether the row changed.
    pub fn set_bounds<G: GroupModel>(groups: &VecModel<G>, group_id: i32, bounds: Bounds) -> bool {
        let Some((index, mut group)) = Self::find_group(groups, group_id) else {
            return false;
        };
        if group.bounds() == bounds {
            return false;
        }
        group.set_bounds(bounds);
        groups.set_row_data(index, group);
        true
    }

    /// Find a group row by id.
    pub fn find_group<G: GroupModel>(
        groups: &impl Model<Data = G>,
        group_id: i32,
    ) -> Option<(usize, G)> {
        GraphLogic::find_node_by_id(groups, group_id, G::id)
    }
}

/// The ids of the rows matching `pred`, in row order.
fn ids_where<T: Clone + 'static>(
    model: &impl Model<Data = T>,
    pred: impl Fn(&T) -> bool,
    id: impl Fn(&T) -> i32,
) -> Vec<i32> {
    (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .filter(|row| pred(row))
        .map(|row| id(&row))
        .collect()
}

/// Run `f` over every row and write back the rows it reports changed.
fn update_rows<T: Clone + 'static>(model: &VecModel<T>, mut f: impl FnMut(&mut T) -> bool) {
    for i in 0..model.row_count() {
        let Some(mut row) = model.row_data(i) else {
            continue;
        };
        if f(&mut row) {
            model.set_row_data(i, row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq)]
    struct Node {
        id: i32,
        x: f32,
        y: f32,
        selected: bool,
        group_id: i32,
    }

    impl MovableNode for Node {
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

    impl GroupMember for Node {
        fn group_id(&self) -> i32 {
            self.group_id
        }
        fn set_group_id(&mut self, group_id: i32) {
            self.group_id = group_id;
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Group {
        id: i32,
        bounds: Bounds,
        selected: bool,
        collapsed: bool,
    }

    impl GroupModel for Group {
        fn id(&self) -> i32 {
            self.id
        }
        fn bounds(&self) -> Bounds {
            self.bounds
        }
        fn set_bounds(&mut self, bounds: Bounds) {
            self.bounds = bounds;
        }
        fn selected(&self) -> bool {
            self.selected
        }
        fn collapsed(&self) -> bool {
            self.collapsed
        }
        fn set_collapsed(&mut self, collapsed: bool) {
            self.collapsed = collapsed;
        }
    }

    fn node(id: i32, group_id: i32, selected: bool) -> Node {
        Node {
            id,
            x: 10.0 * id as f32,
            y: 0.0,
            selected,
            group_id,
        }
    }

    fn group(id: i32, selected: bool) -> Group {
        Group {
            id,
            bounds: Bounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            selected,
            collapsed: false,
        }
    }

    fn positions(nodes: &VecModel<Node>) -> Vec<(i32, f32)> {
        (0..nodes.row_count())
            .filter_map(|i| nodes.row_data(i))
            .map(|n| (n.id, n.x))
            .collect()
    }

    fn collapse(
        groups: &VecModel<Group>,
        nodes: &VecModel<Node>,
        id: i32,
        collapsed: bool,
    ) -> bool {
        GroupLogic::set_collapsed(groups, nodes, id, collapsed, |n| &mut n.selected)
    }

    // ------------------------------------------------------------------
    // Movement set
    // ------------------------------------------------------------------

    #[test]
    fn dragging_a_sheet_moves_its_members_and_nothing_else() {
        let nodes = Rc::new(VecModel::from(vec![
            node(1, 7, false),
            node(2, 7, false),
            node(3, 0, false),
            node(4, 8, false),
        ]));
        let groups = Rc::new(VecModel::from(vec![group(7, false), group(8, false)]));

        GroupLogic::commit_drag(&nodes, &groups, Dragged::Group(7), 5.0, 0.0);

        assert_eq!(
            positions(&nodes),
            vec![(1, 15.0), (2, 25.0), (3, 30.0), (4, 40.0)]
        );
        assert_eq!(groups.row_data(0).unwrap().bounds.x, 5.0);
        assert_eq!(groups.row_data(1).unwrap().bounds.x, 0.0);
    }

    #[test]
    fn dragging_a_node_moves_selected_groups_with_their_members() {
        let nodes = Rc::new(VecModel::from(vec![
            node(1, 7, false),
            node(2, 0, false),
            node(3, 0, true),
        ]));
        let groups = Rc::new(VecModel::from(vec![group(7, true)]));

        // Node 2 is dragged: it moves, the selected node 3 moves, and the
        // selected group 7 moves with its unselected member 1.
        GroupLogic::commit_drag(&nodes, &groups, Dragged::Node(2), 0.0, 3.0);

        let ys: Vec<f32> = (0..3).map(|i| nodes.row_data(i).unwrap().y).collect();
        assert_eq!(ys, vec![3.0, 3.0, 3.0]);
        assert_eq!(groups.row_data(0).unwrap().bounds.y, 3.0);
    }

    #[test]
    fn a_selected_member_of_a_dragged_group_moves_once() {
        let nodes = Rc::new(VecModel::from(vec![node(1, 7, true)]));
        let groups = Rc::new(VecModel::from(vec![group(7, false)]));

        GroupLogic::commit_drag(&nodes, &groups, Dragged::Group(7), 5.0, 0.0);

        assert_eq!(nodes.row_data(0).unwrap().x, 15.0);
    }

    #[test]
    fn group_moves_answers_the_preview_the_same_way_as_the_commit() {
        let groups = Rc::new(VecModel::from(vec![group(7, true), group(8, false)]));

        assert!(GroupLogic::group_moves(&*groups, 7, Dragged::Node(1)));
        assert!(!GroupLogic::group_moves(&*groups, 8, Dragged::Node(1)));
        assert!(GroupLogic::group_moves(&*groups, 8, Dragged::Group(8)));
        assert!(!GroupLogic::group_moves(&*groups, 0, Dragged::Group(0)));
        assert!(!GroupLogic::group_moves(&*groups, 99, Dragged::Node(1)));
        assert_eq!(
            GroupLogic::moving_groups(&*groups, Dragged::Group(8)),
            vec![7, 8]
        );
        assert_eq!(Dragged::from_ids(0, 8), Dragged::Group(8));
        assert_eq!(Dragged::from_ids(3, 0), Dragged::Node(3));
    }

    #[test]
    fn node_drag_commit_across_two_models_moves_the_group_once() {
        let a = Rc::new(VecModel::from(vec![node(1, 7, false)]));
        let b = Rc::new(VecModel::from(vec![node(2, 7, false)]));
        let groups = Rc::new(VecModel::from(vec![group(7, false)]));

        GroupLogic::commit_node_drag(&a, &*groups, Dragged::Group(7), 1.0, 0.0);
        GroupLogic::commit_node_drag(&b, &*groups, Dragged::Group(7), 1.0, 0.0);
        GroupLogic::commit_group_drag(&groups, Dragged::Group(7), 1.0, 0.0);

        assert_eq!(a.row_data(0).unwrap().x, 11.0);
        assert_eq!(b.row_data(0).unwrap().x, 21.0);
        assert_eq!(groups.row_data(0).unwrap().bounds.x, 1.0);
    }

    // ------------------------------------------------------------------
    // Collapse
    // ------------------------------------------------------------------

    #[test]
    fn collapsing_deselects_members_and_is_idempotent() {
        let nodes = Rc::new(VecModel::from(vec![node(1, 7, true), node(2, 0, true)]));
        let groups = Rc::new(VecModel::from(vec![group(7, false)]));

        assert!(collapse(&groups, &nodes, 7, true));
        assert!(GroupLogic::is_collapsed(&*groups, 7));
        assert!(
            !nodes.row_data(0).unwrap().selected,
            "hidden member deselected"
        );
        assert!(nodes.row_data(1).unwrap().selected, "outsider untouched");

        assert!(!collapse(&groups, &nodes, 7, true));
        assert!(!collapse(&groups, &nodes, 99, true));
        assert!(!GroupLogic::is_collapsed(&*groups, 99));
    }

    #[test]
    fn expanding_does_not_restore_the_selection() {
        let nodes = Rc::new(VecModel::from(vec![node(1, 7, true)]));
        let groups = Rc::new(VecModel::from(vec![group(7, false)]));

        collapse(&groups, &nodes, 7, true);
        collapse(&groups, &nodes, 7, false);

        assert!(!groups.row_data(0).unwrap().collapsed);
        assert!(!nodes.row_data(0).unwrap().selected);
    }

    // ------------------------------------------------------------------
    // Membership
    // ------------------------------------------------------------------

    #[test]
    fn new_group_candidates_exclude_members_of_other_groups() {
        let nodes = Rc::new(VecModel::from(vec![
            node(1, 0, true),
            node(2, 5, true),
            node(3, 0, false),
        ]));

        assert_eq!(GroupLogic::selected_ungrouped(&*nodes), vec![1]);
        GroupLogic::assign_members(&nodes, &[1, 42], 9);
        assert_eq!(GroupLogic::members(&*nodes, 9), vec![1]);
        assert_eq!(nodes.row_data(1).unwrap().group_id, 5);
    }

    #[test]
    fn removing_a_member_leaves_it_in_place_and_reports_its_group() {
        let nodes = Rc::new(VecModel::from(vec![node(1, 7, false), node(2, 7, false)]));

        assert_eq!(GroupLogic::remove_member(&nodes, 1), Some(7));
        assert_eq!(GroupLogic::remove_member(&nodes, 1), None);
        assert_eq!(GroupLogic::remove_member(&nodes, 99), None);
        assert_eq!(nodes.row_data(0).unwrap().x, 10.0);
        assert_eq!(GroupLogic::members(&*nodes, 7), vec![2]);
    }

    #[test]
    fn dissolving_ungroups_members_and_removes_the_row() {
        let nodes = Rc::new(VecModel::from(vec![node(1, 7, false), node(2, 8, false)]));
        let groups = Rc::new(VecModel::from(vec![group(7, false), group(8, false)]));

        assert!(GroupLogic::dissolve_group(&nodes, &groups, 7));
        assert!(!GroupLogic::dissolve_group(&nodes, &groups, 7));

        assert_eq!(nodes.row_data(0).unwrap().group_id, 0);
        assert_eq!(nodes.row_data(1).unwrap().group_id, 8);
        assert_eq!(groups.row_count(), 1);
        assert_eq!(groups.row_data(0).unwrap().id, 8);
    }

    // ------------------------------------------------------------------
    // Bounds
    // ------------------------------------------------------------------

    #[test]
    fn fit_bounds_is_the_padded_union_of_cached_member_rects() {
        let mut cache = GeometryCache::new();
        cache.update_node_rect(1, 100.0, 100.0, 50.0, 20.0);
        cache.update_node_rect(2, 200.0, 50.0, 10.0, 100.0);

        assert_eq!(
            GroupLogic::fit_bounds(&cache, &[1, 2, 3], 10.0),
            Some(Bounds {
                x: 90.0,
                y: 40.0,
                width: 130.0,
                height: 120.0
            })
        );
        assert_eq!(GroupLogic::fit_bounds(&cache, &[3], 10.0), None);
        assert_eq!(GroupLogic::fit_bounds(&cache, &[], 10.0), None);
    }

    #[test]
    fn set_bounds_writes_the_row_only_when_it_changes() {
        let groups = Rc::new(VecModel::from(vec![group(7, false)]));
        let fitted = Bounds {
            x: 5.0,
            y: -5.0,
            width: 40.0,
            height: 40.0,
        };

        assert!(GroupLogic::set_bounds(&groups, 7, fitted));
        assert_eq!(groups.row_data(0).unwrap().bounds, fitted);
        assert!(!GroupLogic::set_bounds(&groups, 7, fitted));
        assert!(!GroupLogic::set_bounds(&groups, 8, fitted));
        assert_eq!(
            fitted.offset(1.0, 2.0),
            Bounds {
                x: 6.0,
                y: -3.0,
                ..fitted
            }
        );
    }
}
