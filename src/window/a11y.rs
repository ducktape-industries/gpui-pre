//! Accessibility support, provided by [AccessKit][accesskit].
//!
//! There are user-facing guide-level docs [here](crate::_accessibility).
//!
//! ## Architecture
//!
//! ```text
//!                              ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           ┌─▶│ AccessKit Adapter (MacOS)      │◀─▶│ MacOS System APIs   │
//!                           │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//! ┌──────┐   ┌───────────┐  │  ┌────────────────────────────────┐   ┌─────────────────────┐
//! │ GPUI │◀─▶│ AccessKit │◀─┼─▶│ AccessKit Adapter (Windows)    │◀─▶│ Windows System APIs │
//! └──────┘   └───────────┘  │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//!                           │  ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           └─▶│ AccessKit Adapter (Linux)      │◀─▶│ dbus                │
//!                              └────────────────────────────────┘   └─────────────────────┘
//! ```
//!
//! In order for GPUI apps to be usable for people using assistive technology,
//! we must do a few things:
//! - Inform the system when the UI changes meaningfully. This includes:
//!   - Reporting new/removed/changed UI elements
//!   - *Not* reporting irrelevant UI changes, e.g. an invisible `div()` being
//!     added.
//!   - Reporting the appearance and capabilities of each UI element. For example:
//!     - What does this piece of text say?
//!     - How far along is this progress bar?
//!     - Can this node be focused?
//!     - Can this node have a value directly assigned? (e.g. a slider)
//! - Allowing the system to interact with the UI by dispatching actions to
//!   nodes. Note that AccessKit has its own [`Action`] type, which is not the
//!   [`crate::Action`] trait.
//! - Activate and deactivate accessibility features when requested by the
//!   system.
//!
//! Activating and deactivating at the right time is trivial, so I won't go into
//! detail here. The other two are almost orthogonal in implementation.
//!
//! The state for both lives in the [`A11y`] struct in this module.
//!
//! ### Reporting UI changes
//!
//! Every frame, we build a [`TreeUpdate`] and send it to the platform-specific
//! adapter. A [`TreeUpdate`] is a representation of a subset of the UI tree.
//! When the adapter receives the update, it diffs it against the previous
//! update, and calls platform-specific APIs to inform screen readers about the
//! changes. Nodes may have been created, destroyed, or updated.
//!
//! Each node has an ID, and this ID *should* be stable across frames. If a
//! node's ID changes, then, from AccessKit's point of view, it is a different
//! node.
//!
//! We derive the node ID from the [`GlobalElementId`] in
//! [`GlobalElementId::accesskit_node_id`]. Nodes without [`GlobalElementId`]s
//! cannot produce an AccessKit [`NodeId`], and so are not included in the
//! accessibility tree. We try to warn when using accessibility APIs on
//! [`div()`] without setting an ID.
//!
//! This all happens in [`Drawable::prepaint`]. The [`A11y`] struct maintains a
//! stack of nodes during prepainting, which we can use to calculate the
//! [`NodeId`]s, and record parent-child relationships. Once all [`Element`]s in
//! a frame have been prepainted, we send the resulting [`TreeUpdate`] object to
//! the adapter and the screen reader can announce the changes.
//!
//! #### Synthetic children
//!
//! Additionally, some nodes can register "synthetic children" using
//! [`Element::a11y_synthetic_children`]. Normally, one accesskit node is pushed
//! for every [`Element`] with a role and id. However, sometimes a single
//! element may want to produce many accesskit nodes. These extra nodes are
//! referred to as "synthetic children" of the element providing a non-default
//! [`Element::a11y_synthetic_children`] implementation.
//!
//! The user is provided a builder-style API using [`A11ySubtreeBuilder`], which
//! allows them to create push nodes that are children of the current node, as
//! well as modify the current node itself.
//!
//! GPUI calls this callback *after* prepainting (and just before popping the
//! corresponding element), since this step may need prepaint information to be
//! available. In the future, we may want to add prepaint information more
//! generally to [`Element::write_a11y_info`], but for now that's not necessary.
//!
//! ### Responding to actions
//!
//! On adapter creation, we provide a callback to the adapter, which can be used
//! to dispatch actions. This callback forwards to [`A11y::action_listeners`], a
//! mapping from [`NodeId`]s to action handlers (basically just `Box<dyn
//! Fn()>`).
//!
//! This is populated in:
//! - [`Window::on_a11y_action`], which is called by:
//! - [`Interactivity::paint`], which is called by:
//! - [`StatefulInteractiveElement::on_a11y_action`], which is a public-facing API
//!
//! These are cleared at the start of a frame, and re-populated during painting.
//!
//! [`NodeId`]: accesskit::NodeId

use crate::*;

pub(crate) mod debug;

use crate::{App, Bounds, FocusId, Pixels, SharedString, Window};
use accesskit::{Action, NodeId, TreeUpdate};
use collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// The fixed AccessKit node ID used for the root of every window's a11y tree.
pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

/// A listener for an accessibility action on a specific node.
pub(crate) type A11yActionListener =
    Box<dyn FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static>;

/// What the last drawn frame built, kept for cache reuse.
///
/// [`A11y::begin_frame`] moves the live per-frame maps here at the start of
/// the next frame (not at its end: the live maps are read between frames by
/// [`crate::Window::a11y_element_id`] and action dispatch). A cached view
/// whose prepaint is reused copies its nodes and side entries back out of
/// here by log range, see [`A11y::reuse_range`].
#[derive(Default)]
pub(crate) struct RenderedA11y {
    /// `(parent, id, node)` in pop order, as [`A11yNodeBuilder::log`].
    log: Vec<(NodeId, NodeId, accesskit::Node)>,
    focus_ids: FxHashMap<NodeId, FocusId>,
    node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    element_ids: FxHashMap<NodeId, GlobalElementId>,
    action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
    focus: Option<NodeId>,
    /// Every active-descendant claim made, as [`A11y::claims`].
    claims: Vec<(NodeId, Option<NodeId>)>,
    /// The previous draw ran `begin_frame` and `end_frame`, so `log` is the
    /// tree the prepaint ranges of that frame index into.
    pub(crate) built: bool,
}

/// Per-window accessibility state.
///
/// Manages the AccessKit tree that is built each frame and the mappings
/// needed to dispatch incoming action requests back to the right elements.
pub(crate) struct A11y {
    /// Whether accessibility has been [forcibly disabled] for this window.
    ///
    /// [forcibly disabled]: crate::Application::new_inaccessible
    force_disabled: bool,
    /// Whether a11y features have been requested by the system.
    ///
    /// Updated by AccessKit using callbacks provided to the adapter. Can change
    /// halfway through a frame.
    active_flag: Arc<AtomicBool>,
    /// Whether a11y features are active for *this specific frame*.
    ///
    /// At the start of each frame, we load [`Self::active_flag`] (using
    /// [`Self::sync_active_flag`]) and use this to determine whether we
    /// should construct a [`TreeUpdate`] for this frame. It's important that
    /// this value is stable within a frame, because the builder API exposed by
    /// this type maintains a stack of nodes and each must be pushed and popped
    /// exactly once.
    ///
    /// At the end of the frame, we re-call [`Self::sync_active_flag`] to
    /// determine whether we should actually send the finished [`TreeUpdate`].
    active_this_frame: bool,
    pub(crate) nodes: A11yNodeBuilder,
    pub(crate) focus_ids: FxHashMap<NodeId, FocusId>,
    pub(crate) node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    /// The element each node of this frame was built from, so a tool can
    /// name a node by its element path rather than its hashed [`NodeId`].
    pub(crate) element_ids: FxHashMap<NodeId, GlobalElementId>,
    /// The elements whose node this frame refused because an earlier node
    /// already had the same id, in paint order.
    refused: Vec<(NodeId, GlobalElementId)>,
    pub(crate) action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
    /// Every active-descendant claim made this frame, honored or not, with
    /// its gate: the claimant's nearest focusable ancestor, whose focus
    /// decides the claim. A reused range judges its claims again by the
    /// current frame's focus (see [`Self::reuse_range`]).
    claims: Vec<(NodeId, Option<NodeId>)>,
    pub(crate) rendered: RenderedA11y,
    /// The window's title, used to label the root node so assistive
    /// technology can tell windows apart.
    window_title: Option<SharedString>,
    /// The focus id we most recently reported as having no accessibility node,
    /// used to log at most once per focus change rather than every frame.
    last_focus_without_node: Option<FocusId>,
    /// Retains the last tree update (and, in debug builds, per-node provenance)
    /// so it can be dumped via [`crate::Window::debug_a11y_tree_json`].
    debug: debug::A11yDebug,
    /// Maps a view's [`EntityId`] to its `Render` type name
    #[cfg(debug_assertions)]
    pub(crate) view_type_names: FxHashMap<EntityId, &'static str>,
}

impl A11y {
    pub(crate) fn new(
        active_flag: Arc<AtomicBool>,
        force_disabled: bool,
        window_title: Option<SharedString>,
    ) -> Self {
        Self {
            force_disabled,
            active_flag,
            active_this_frame: false,
            nodes: A11yNodeBuilder::new(),
            focus_ids: FxHashMap::default(),
            node_bounds: FxHashMap::default(),
            element_ids: FxHashMap::default(),
            refused: Vec::new(),
            action_listeners: FxHashMap::default(),
            claims: Vec::new(),
            rendered: RenderedA11y::default(),
            window_title,
            last_focus_without_node: None,
            debug: debug::A11yDebug::default(),
            #[cfg(debug_assertions)]
            view_type_names: FxHashMap::default(),
        }
    }

    /// Logs (once per focus change) that the focused element is not exposed to
    /// assistive technology because it has no accessibility node. When this
    /// happens, screen readers fall back to announcing the whole window instead
    /// of the focused element. The fix is to give the element both an
    /// `.id(...)` and a `.role(...)`.
    pub(crate) fn note_focus_without_node(&mut self, focus_id: FocusId, reason: &str) {
        if self.last_focus_without_node != Some(focus_id) {
            self.last_focus_without_node = Some(focus_id);
            log::info!(
                "a11y: focused element ({focus_id:?}) has no accessibility node \
                 ({reason}); assistive technology will announce the whole window \
                 instead. Give it both an `.id(...)` and a `.role(...)` to expose it."
            );
        }
    }

    pub(crate) fn set_window_title(&mut self, title: impl Into<SharedString>) {
        self.window_title = Some(title.into());
    }

    /// Ensures that [`Self::is_active`] returns up to date information.
    ///
    /// See the docs for [`Self::active_flag`] and [`Self::active_this_frame`]
    /// for more commentary.
    pub(crate) fn sync_active_flag(&mut self) {
        self.active_this_frame = self.is_enabled() && self.active_flag.load(Ordering::SeqCst);
    }

    pub(crate) fn is_enabled(&self) -> bool {
        !self.force_disabled
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_this_frame
    }

    /// Marks accessibility as requested, as an adapter's activation does.
    pub(crate) fn activate(&self) {
        self.active_flag.store(true, Ordering::SeqCst);
    }

    pub(crate) fn set_focusable(&mut self, node_id: NodeId, focus_id: FocusId) {
        self.focus_ids.insert(node_id, focus_id);
    }

    /// Report `node_id` as the currently-focused node, if it is present in the
    /// tree.
    ///
    /// Must only be called once per frame.
    pub(crate) fn set_focus(&mut self, node_id: NodeId) {
        // A focused node must have been registered as focusable this frame.
        if !self.focus_ids.contains_key(&node_id) {
            if cfg!(debug_assertions) {
                panic!("set_focus called for a node that was not registered with set_focusable");
            } else {
                log::warn!(
                    "a11y: set_focus called for a node that was not registered with \
                     set_focusable ({node_id:?})"
                );
            }
        }
        if self.nodes.has_node(node_id) {
            // The focused element is properly exposed; reset the dedup so a
            // later focus on a node-less element logs again.
            self.last_focus_without_node = None;
            self.nodes.set_focus(node_id);
        } else {
            // The element registered a focus handle and an id, but never got a
            // node because it has no role.
            if let Some(focus_id) = self.focus_ids.get(&node_id).copied() {
                self.note_focus_without_node(focus_id, "it has an id but no role");
            }
        }
    }

    pub(crate) fn set_active_descendant(&mut self, node_id: NodeId) {
        // The active descendant must be a descendant of the focused container,
        // not the focused node itself.
        if self.nodes.node_is_focused(node_id) {
            if cfg!(debug_assertions) {
                panic!("set_active_descendant called on the focused node");
            } else {
                log::warn!("a11y: set_active_descendant called on the focused node ({node_id:?})");
            }
            return;
        }
        if self.nodes.has_node(node_id) {
            let gate = self.nodes.nearest_focusable_ancestor(&self.focus_ids);
            self.claims.push((node_id, gate));
            if gate.is_some() && gate == self.nodes.focus {
                self.nodes.set_active_descendant(node_id);
            }
        }
    }

    /// Push the node of `element` onto the tree, remembering the element for
    /// [`Window::a11y_element_id`] or, when the id is taken, for
    /// [`Window::a11y_refused_elements`].
    ///
    /// Returns `true` if the node was pushed.
    pub(crate) fn push_element(
        &mut self,
        node_id: NodeId,
        node: accesskit::Node,
        element: &GlobalElementId,
    ) -> bool {
        let pushed = self.nodes.push(node_id, node);
        if pushed {
            self.element_ids.insert(node_id, element.clone());
        } else {
            self.refused.push((node_id, element.clone()));
        }
        pushed
    }

    pub(crate) fn refused_elements(&self) -> &[(NodeId, GlobalElementId)] {
        &self.refused
    }

    /// Hand the last frame's state to [`Self::rendered`], clear the rest and
    /// push the root node to start a new frame.
    pub(crate) fn begin_frame(&mut self) {
        self.rendered.focus_ids = mem::take(&mut self.focus_ids);
        self.rendered.node_bounds = mem::take(&mut self.node_bounds);
        self.rendered.element_ids = mem::take(&mut self.element_ids);
        self.rendered.action_listeners = mem::take(&mut self.action_listeners);
        self.rendered.log = mem::take(&mut self.nodes.log);
        self.rendered.focus = self.nodes.focus;
        self.rendered.claims = mem::take(&mut self.claims);
        self.refused.clear();
        self.nodes.begin_frame(self.window_title.as_ref());
    }

    /// Replay the nodes a reused prepaint built last frame: `range` indexes
    /// [`RenderedA11y::log`]. Nodes whose parent lies outside the range become
    /// children of the current top-of-stack node; the side maps are copied and
    /// the action listeners moved for every node replayed. `contains_focus`
    /// (the reused dispatch subtree's) carries the focused node over.
    ///
    /// The log is in pop order (children first), so it is walked in reverse
    /// and a node is replayed only when its parent was: a duplicate id (an
    /// element that moved between a cached and an uncached parent this frame)
    /// is left out with its subtree and recorded as refused, as a refused push
    /// is, never grafted as an orphan, and its replayed parent no longer lists
    /// it. Bounds need no re-mapping: the cache key demands equal bounds.
    pub(crate) fn reuse_range(&mut self, range: Range<usize>, contains_focus: bool) {
        let Some(entries) = self.rendered.log.get(range) else {
            return;
        };
        let in_range: FxHashSet<NodeId> = entries.iter().map(|(_, id, _)| *id).collect();
        let mut pushed = FxHashSet::default();
        // Decided parents first (reverse), appended in pop order (children
        // first) so the next frame's range reads the same way.
        let mut kept = Vec::with_capacity(entries.len());
        for (parent, id, node) in entries.iter().rev() {
            let parent_in_range = in_range.contains(parent);
            if parent_in_range && !pushed.contains(parent) {
                continue;
            }
            if !self.nodes.seen_ids.insert(*id) {
                if let Some(element) = self.rendered.element_ids.get(id) {
                    self.refused.push((*id, element.clone()));
                }
                continue;
            }
            let parent = match (parent_in_range, self.nodes.ids_stack.last()) {
                (true, _) => *parent,
                (false, Some(head)) => *head,
                (false, None) => continue,
            };
            pushed.insert(*id);
            kept.push((parent, *id, node.clone(), !parent_in_range));
            if let Some(focus_id) = self.rendered.focus_ids.get(id) {
                self.focus_ids.insert(*id, *focus_id);
            }
            if let Some(bounds) = self.rendered.node_bounds.get(id) {
                self.node_bounds.insert(*id, *bounds);
            }
            if let Some(element) = self.rendered.element_ids.get(id) {
                self.element_ids.insert(*id, element.clone());
            }
            if let Some(listeners) = self.rendered.action_listeners.remove(id) {
                self.action_listeners
                    .entry(*id)
                    .or_default()
                    .extend(listeners);
            }
        }
        for (parent, id, mut node, top) in kept.into_iter().rev() {
            // Pop order, so top-level siblings join the head as they were.
            if top && let Some(head) = self.nodes.nodes_stack.last_mut() {
                head.push_child(id);
            }
            // A child left out above must not stay listed under its parent.
            if node.children().iter().any(|c| !pushed.contains(c)) {
                let children: Vec<NodeId> = node
                    .children()
                    .iter()
                    .copied()
                    .filter(|c| pushed.contains(c))
                    .collect();
                node.set_children(children);
            }
            self.nodes.all_nodes.push((id, node.clone()));
            self.nodes.log.push((parent, id, node));
        }
        if contains_focus && let Some(focus) = self.rendered.focus.filter(|f| pushed.contains(f)) {
            self.nodes.focus = Some(focus);
        }
        // A replayed claim is judged as a live build would judge it, by this
        // frame's focus: it counts when its gate is the focused node (set in
        // the range above, or by an ancestor already pushed live). The gate
        // can have gained or lost the focus since the range was built.
        for (claim, gate) in &self.rendered.claims {
            if pushed.contains(claim) {
                self.claims.push((*claim, *gate));
                if gate.is_some() && *gate == self.nodes.focus {
                    self.nodes.set_active_descendant(*claim);
                }
            }
        }
    }

    /// Finalize the tree and produce a [`TreeUpdate`] for the platform adapter.
    pub(crate) fn end_frame(&mut self, frame: debug::FrameDebugInfo) -> TreeUpdate {
        let update = self.nodes.finalize();
        self.debug.capture(
            &update,
            self.nodes.focus,
            self.nodes.active_descendant,
            self.window_title.as_ref(),
            frame,
        );
        #[cfg(debug_assertions)]
        self.debug.capture_node_info(&self.nodes.node_info);
        update
    }

    pub(crate) fn last_tree_update(&self) -> Option<&TreeUpdate> {
        self.debug.last_tree_update()
    }

    pub(crate) fn debug_tree_json(&self) -> Option<String> {
        self.debug.to_json()
    }
}

/// Builder API for synthetic children. See the docs for
/// [`Element::a11y_synthetic_children`].
pub struct A11ySubtreeBuilder<'a> {
    parent_id: NodeId,
    nodes: &'a mut A11yNodeBuilder,
    /// Provenance of the real element whose `a11y_synthetic_children` is
    /// running.
    #[cfg(debug_assertions)]
    creator: debug::NodeCreator,
}

impl<'a> A11ySubtreeBuilder<'a> {
    pub(crate) fn new(parent_id: NodeId, nodes: &'a mut A11yNodeBuilder) -> Self {
        Self {
            parent_id,
            nodes,
            #[cfg(debug_assertions)]
            creator: debug::NodeCreator::default(),
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn with_creator(mut self, creator: debug::NodeCreator) -> Self {
        self.creator = creator;
        self
    }

    /// Derive a [`NodeId`] for a synthetic child.
    ///
    /// The generated ID is based on the hash of `key`, as well as the parent's
    /// ID. This means that `key`s must be unique within the same
    /// [`Element::a11y_synthetic_children`] call, but may be duplicated across
    /// different calls.
    pub fn synthetic_node_id(&self, key: impl Hash) -> NodeId {
        let mut hasher = std::hash::DefaultHasher::default();
        self.parent_id.0.hash(&mut hasher);
        key.hash(&mut hasher);
        NodeId(hasher.finish())
    }

    /// Append a synthetic leaf node as a child of this element's node.
    ///
    /// Returns `false` if a node with this id is already present in the tree,
    /// in which case the node is discarded.
    pub fn push_child(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        let pushed = self.nodes.push_leaf(id, node);
        #[cfg(debug_assertions)]
        if pushed {
            self.nodes.record_node_info(
                id,
                debug::NodeDebugInfo {
                    synthetic: true,
                    view: self.creator.view,
                    element_id: self.creator.element_id.clone(),
                    source_location: self.creator.source_location,
                },
            );
        }
        pushed
    }

    /// A mutable reference to the parent node.
    pub fn parent_node(&mut self) -> &mut accesskit::Node {
        self.nodes
            .current_node_mut()
            .expect("A11ySubtreeBuilder exists only while its element's node is on the stack")
    }
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    /// This is the exact type required by accesskit, so we can't just make it a
    /// `HashMap<NodeId, Node>` to remove the need for `seen_ids`
    all_nodes: Vec<(NodeId, accesskit::Node)>,
    /// `(parent, id, node)` for every `all_nodes` push this frame, in the
    /// same order, so a reused prepaint range can replay its nodes (see
    /// [`A11y::reuse_range`]); [`crate::Window::prepaint_index`] records its
    /// length. `finalize` leaves it alone.
    log: Vec<(NodeId, NodeId, accesskit::Node)>,
    seen_ids: FxHashSet<NodeId>,
    /// The node that GPUI considers focused. Note that this may be different to
    /// what is reported to accesskit - see [`Self::active_descendant`]
    focus: Option<NodeId>,
    /// If a node calls `.aria_active_descendant()`, AND its nearest focusable
    /// ancestor is focused, override it as the focused node. This supports the
    /// "active descendant" pattern, which allows a focused container to act as
    /// if a descendant is focused.
    active_descendant: Option<NodeId>,
    #[cfg(debug_assertions)]
    node_info: FxHashMap<NodeId, debug::NodeDebugInfo>,
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            log: Vec::new(),
            seen_ids: FxHashSet::default(),
            focus: None,
            active_descendant: None,
            #[cfg(debug_assertions)]
            node_info: FxHashMap::default(),
        }
    }

    /// Records provenance for a node already pushed this frame. Debug builds only.
    #[cfg(debug_assertions)]
    pub(crate) fn record_node_info(&mut self, id: NodeId, info: debug::NodeDebugInfo) {
        self.node_info.insert(id, info);
    }

    #[must_use]
    fn can_push(&mut self, id: NodeId) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "node pushed before push_root");

        if !self.seen_ids.insert(id) {
            debug_assert!(
                false,
                "Duplicate a11y node id: {id:?}. In a release build, this node would be silently discarded from the a11y tree."
            );
            return false;
        }

        true
    }

    /// Push a new node onto the stack. It becomes a child of the current
    /// top-of-stack node.
    ///
    /// Returns `true` if the node was successfully pushed.
    pub(crate) fn push(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        if !self.can_push(id) {
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        true
    }

    /// Add a leaf node as a child of the current top-of-stack node, without
    /// pushing it onto the stack. Semantically equivalent to a [`Self::push`]
    /// followed by a [`Self::pop`].
    ///
    /// Returns `true` if the node was successfully pushed.
    pub(crate) fn push_leaf(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        if !self.can_push(id) {
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.log_node(id, &node);
        self.all_nodes.push((id, node));
        true
    }

    /// Records `node` in [`Self::log`] under the current top-of-stack parent.
    fn log_node(&mut self, id: NodeId, node: &accesskit::Node) {
        if let Some(parent) = self.ids_stack.last() {
            self.log.push((*parent, id, node.clone()));
        }
    }

    pub(crate) fn log_len(&self) -> usize {
        self.log.len()
    }

    pub(crate) fn current_node_mut(&mut self) -> Option<&mut accesskit::Node> {
        self.nodes_stack.last_mut()
    }

    /// Pop the current node off the stack and finalize it into the all_nodes
    /// list.
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
            self.log_node(id, &node);
            self.all_nodes.push((id, node));
        }
    }

    /// Push the root node to start a new frame.
    fn begin_frame(&mut self, window_title: Option<&SharedString>) {
        self.all_nodes.clear();
        self.log.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.seen_ids.clear();
        #[cfg(debug_assertions)]
        self.node_info.clear();
        let mut root_node = accesskit::Node::new(accesskit::Role::Window);
        if let Some(title) = window_title {
            root_node.set_label(title.to_string());
        }

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.focus = None;
        self.active_descendant = None;
    }

    /// Returns whether a node with the given ID has been pushed in this frame.
    pub(crate) fn has_node(&self, id: NodeId) -> bool {
        id == ROOT_NODE_ID || self.seen_ids.contains(&id)
    }

    /// Returns whether `id` is the node currently reported as focused.
    pub(crate) fn node_is_focused(&self, id: NodeId) -> bool {
        self.focus == Some(id)
    }

    /// The current node's nearest focusable ancestor: the first node below it
    /// on the stack that is in `focus_ids`. It gates the node's
    /// active-descendant claim, which counts only while it is the focused node.
    ///
    /// A focusable element with no role pushes no node, so the walk passes
    /// over it; it can never be the focused node either.
    pub(crate) fn nearest_focusable_ancestor(
        &self,
        focus_ids: &FxHashMap<NodeId, FocusId>,
    ) -> Option<NodeId> {
        // The current node is on top of the stack; everything below it is an
        // ancestor.
        let ancestor_count = self.ids_stack.len().saturating_sub(1);
        self.ids_stack[..ancestor_count]
            .iter()
            .rev()
            .find(|id| focus_ids.contains_key(id))
            .copied()
    }

    pub(crate) fn set_active_descendant(&mut self, id: NodeId) {
        if self
            .active_descendant
            .is_some_and(|existing| existing != id)
        {
            if cfg!(debug_assertions) {
                panic!("active descendant claimed by multiple nodes in one frame");
            } else {
                log::warn!(
                    "a11y: multiple nodes claimed the active descendant this frame; \
                     using last-wins ({id:?})"
                );
            }
        }
        self.active_descendant = Some(id);
    }

    pub(crate) fn set_focus(&mut self, id: NodeId) {
        if self.focus.is_some() {
            if cfg!(debug_assertions) {
                panic!("set_focus called more than once in a single frame");
            } else {
                log::warn!(
                    "a11y: set_focus called more than once in a single frame; \
                     using last-wins ({id:?})"
                );
            }
        }
        self.focus = Some(id);
    }

    fn finalize(&mut self) -> TreeUpdate {
        // Stack should contain only the root node
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);

        if self.ids_stack.len() != 1 {
            log::error!(
                "a11y: Stack imbalance at end of frame: expected 1 (root), got {}. \
                 Some elements may have pushed without popping.",
                self.ids_stack.len()
            );
        }

        // Pop remaining nodes (should just be the root).
        while !self.ids_stack.is_empty() {
            if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
                self.all_nodes.push((id, node));
            }
        }

        let focus = match self.active_descendant {
            Some(id) if self.has_node(id) => id,
            Some(id) => {
                if cfg!(debug_assertions) {
                    panic!("active_descendant set to {id:?}, which is not in the tree");
                } else {
                    log::warn!("active_descendant set to {id:?}, which is not in the tree");
                    self.focus.unwrap_or(ROOT_NODE_ID)
                }
            }

            _ => self.focus.unwrap_or(ROOT_NODE_ID),
        };

        let nodes = std::mem::take(&mut self.all_nodes);
        let update = TreeUpdate {
            nodes,
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus,
        };

        Self::repair_tree_update(update)
    }

    /// Accesskit panics on invalid [`TreeUpdate`]s. This function defensively
    /// checks invariants that accesskit panics on, and tries to fix them.
    fn repair_tree_update(mut update: TreeUpdate) -> TreeUpdate {
        let node_ids: FxHashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();

        // Focus must point to a node in the tree.
        if !node_ids.contains(&update.focus) {
            log::error!(
                "a11y: Focused node {:?} is not in the tree ({} nodes). \
                 Falling back to root. This is a bug in the a11y tree builder.",
                update.focus,
                update.nodes.len()
            );
            update.focus = ROOT_NODE_ID;
        }

        // Every child reference must point to a node in the update.
        for (id, node) in &mut update.nodes {
            let has_invalid_child = node
                .children()
                .iter()
                .any(|child_id| !node_ids.contains(child_id));
            if has_invalid_child {
                let children = node.children();
                let invalid_count = children
                    .iter()
                    .filter(|child_id| !node_ids.contains(child_id))
                    .count();
                log::error!(
                    "a11y: Node {:?} references {} children not present in the tree. \
                     Stripping invalid child references.",
                    id,
                    invalid_count
                );
                let valid: Vec<NodeId> = children
                    .iter()
                    .copied()
                    .filter(|child_id| node_ids.contains(child_id))
                    .collect();
                node.set_children(valid);
            }
        }

        update
    }
}

#[cfg(test)]
mod tests {
    // Import specific items rather than glob-importing `super`, which would pull
    // in gpui's own `test` attribute macro and shadow the standard one.
    use super::{A11y, A11yNodeBuilder, ROOT_NODE_ID};
    use crate::{Bounds, ElementId, FocusId, GlobalElementId, point, px, size};
    use accesskit::{NodeId, Role};
    use std::sync::{Arc, atomic::AtomicBool};

    fn test_node() -> accesskit::Node {
        accesskit::Node::new(Role::GenericContainer)
    }

    fn new_builder() -> A11yNodeBuilder {
        let mut builder = A11yNodeBuilder::new();
        builder.begin_frame(None);
        builder
    }

    fn new_a11y() -> A11y {
        let mut a11y = A11y::new(Arc::new(AtomicBool::new(true)), false, None);
        a11y.begin_frame();
        a11y
    }

    fn element(name: &'static str) -> GlobalElementId {
        GlobalElementId(Arc::from([ElementId::Name(name.into())]))
    }

    // A second element with an id already used this frame: panic in debug; in
    // release its node is left out and the element is recorded as refused.
    #[test]
    #[cfg_attr(debug_assertions, should_panic(expected = "Duplicate a11y node id"))]
    fn a_second_element_with_one_id_is_refused() {
        let mut a11y = new_a11y();
        let id = NodeId(1);
        let (first, second) = (element("first"), element("second"));

        assert!(a11y.push_element(id, test_node(), &first));
        a11y.nodes.pop();
        assert!(!a11y.push_element(id, test_node(), &second));

        assert_eq!(a11y.refused_elements(), [(id, second)]);
        assert_eq!(a11y.element_ids.get(&id), Some(&first));

        a11y.begin_frame();
        assert!(a11y.refused_elements().is_empty());
    }

    // A frame's tree, then the next frame replaying it whole: the focused
    // container's active-descendant claim and the item's bounds come back
    // with the nodes, and the replayed nodes hang under the new root.
    #[test]
    fn a_reused_range_carries_the_claim_and_the_bounds() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let item = NodeId(2);
        let bounds = Bounds::new(point(px(1.), px(2.)), size(px(3.), px(4.)));

        push_focusable(&mut a11y, container);
        a11y.set_focus(container);
        assert!(a11y.nodes.push(item, test_node()));
        a11y.node_bounds.insert(item, bounds);
        a11y.set_active_descendant(item);
        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // container
        assert_eq!(a11y.end_frame(Default::default()).focus, item);

        a11y.begin_frame();
        assert_eq!(a11y.rendered.log.len(), 2);
        a11y.reuse_range(0..2, true);

        assert_eq!(a11y.nodes.focus, Some(container));
        assert_eq!(a11y.nodes.active_descendant, Some(item));
        assert_eq!(a11y.node_bounds.get(&item), Some(&bounds));
        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, item);
        let root = update.nodes.iter().find(|(id, _)| *id == ROOT_NODE_ID);
        assert_eq!(root.map(|(_, n)| n.children()), Some(&[container][..]));
    }

    // A claim refused last frame (its container unfocused) counts when the
    // range replays under the container focused this frame; a claim honored
    // last frame is dropped when the focus has left the container.
    #[test]
    fn a_replayed_claim_is_judged_by_this_frames_focus() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let item = NodeId(2);

        push_focusable(&mut a11y, container);
        assert!(a11y.nodes.push(item, test_node()));
        a11y.set_active_descendant(item);
        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // container
        assert_eq!(a11y.end_frame(Default::default()).focus, ROOT_NODE_ID);

        a11y.begin_frame();
        a11y.reuse_range(0..2, false);
        // The container replayed, focus arrives on it live this frame.
        a11y.set_focusable(container, FocusId::default());
        a11y.set_focus(container);
        assert_eq!(a11y.nodes.active_descendant, None, "judged at the replay");

        a11y.begin_frame();
        a11y.reuse_range(0..2, true);
        assert_eq!(a11y.nodes.focus, Some(container));
        assert_eq!(a11y.nodes.active_descendant, Some(item));
        assert_eq!(a11y.end_frame(Default::default()).focus, item);

        a11y.begin_frame();
        a11y.reuse_range(0..2, false);
        assert_eq!(a11y.nodes.focus, None);
        assert_eq!(a11y.nodes.active_descendant, None);
    }

    // A replayed node whose id this frame already pushed is left out, and so
    // is its subtree: no node is grafted under a parent that is not there.
    #[test]
    fn a_reused_duplicate_is_left_out_with_its_subtree() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let item = NodeId(2);
        let element = element("container");

        assert!(a11y.push_element(container, test_node(), &element));
        assert!(a11y.nodes.push(item, test_node()));
        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // container
        a11y.end_frame(Default::default());

        a11y.begin_frame();
        assert!(a11y.nodes.seen_ids.insert(container));
        a11y.reuse_range(0..2, false);
        assert_eq!(a11y.refused_elements(), [(container, element)]);

        let update = a11y.end_frame(Default::default());
        let ids: Vec<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, [ROOT_NODE_ID]);
    }

    // Top-level siblings of a replayed range keep their order under the head.
    #[test]
    fn a_reused_range_keeps_sibling_order() {
        let mut a11y = new_a11y();
        assert!(a11y.nodes.push_leaf(NodeId(1), test_node()));
        assert!(a11y.nodes.push_leaf(NodeId(2), test_node()));
        a11y.end_frame(Default::default());

        a11y.begin_frame();
        a11y.reuse_range(0..2, false);
        let update = a11y.end_frame(Default::default());
        let root = update.nodes.iter().find(|(id, _)| *id == ROOT_NODE_ID);
        assert_eq!(
            root.map(|(_, n)| n.children()),
            Some(&[NodeId(1), NodeId(2)][..])
        );
    }

    // A replayed child whose id this frame already pushed elsewhere is not
    // left listed under its replayed parent (accesskit panics on a child
    // listed twice).
    #[test]
    fn a_reused_child_taken_live_is_listed_once() {
        let mut a11y = new_a11y();
        let (container, item) = (NodeId(1), NodeId(2));
        assert!(a11y.nodes.push(container, test_node()));
        assert!(a11y.nodes.push_leaf(item, test_node()));
        a11y.nodes.pop(); // container
        a11y.end_frame(Default::default());

        a11y.begin_frame();
        assert!(a11y.nodes.push_leaf(item, test_node()));
        a11y.reuse_range(0..2, false);
        let update = a11y.end_frame(Default::default());
        let parents = update
            .nodes
            .iter()
            .filter(|(_, n)| n.children().contains(&item))
            .count();
        assert_eq!(parents, 1);
    }

    #[test]
    fn distinct_ids_refuse_nothing() {
        let mut a11y = new_a11y();

        assert!(a11y.push_element(NodeId(1), test_node(), &element("first")));
        a11y.nodes.pop();
        assert!(a11y.push_element(NodeId(2), test_node(), &element("second")));
        a11y.nodes.pop();

        assert!(a11y.refused_elements().is_empty());
    }

    #[test]
    fn accessibility_enabled_is_independent_of_activation() {
        for force_disabled in [false, true] {
            for active in [false, true] {
                let mut a11y = A11y::new(Arc::new(AtomicBool::new(active)), force_disabled, None);
                a11y.sync_active_flag();

                assert_eq!(a11y.is_enabled(), !force_disabled);
                assert_eq!(a11y.is_active(), !force_disabled && active);
            }
        }
    }

    // Registers `id` as focusable this frame, as a div with a focus handle does
    // in its prepaint before its children.
    fn push_focusable(a11y: &mut A11y, id: NodeId) {
        assert!(a11y.nodes.push(id, test_node()));
        a11y.set_focusable(id, FocusId::default());
    }

    #[test]
    fn active_descendant_honored_when_container_focused() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let item = NodeId(2);

        push_focusable(&mut a11y, container);
        a11y.set_focus(container);
        assert!(a11y.nodes.push(item, test_node()));

        // The item's nearest focusable ancestor is the focused container, so
        // the claim is honored.
        a11y.set_active_descendant(item);

        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // container
        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, item);
    }

    #[test]
    fn active_descendant_honored_for_deep_descendant() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let group = NodeId(2);
        let item = NodeId(3);

        push_focusable(&mut a11y, container);
        a11y.set_focus(container);
        assert!(a11y.nodes.push(group, test_node()));
        assert!(a11y.nodes.push(item, test_node()));

        // The item is a grandchild of the focused container; depth doesn't
        // matter while no focusable node sits in between.
        a11y.set_active_descendant(item);

        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // group
        a11y.nodes.pop(); // container
        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, item);
    }

    // A focused outer node holds a focusable inner node that is not focused;
    // the inner node's item claims. The inner node is the item's nearest
    // focusable ancestor and does not have the keys, so the claim is ignored
    // and the outer node stays focused.
    #[test]
    fn active_descendant_ignored_under_unfocused_focusable_ancestor() {
        let mut a11y = new_a11y();
        let outer = NodeId(1);
        let inner = NodeId(2);
        let item = NodeId(3);

        push_focusable(&mut a11y, outer);
        a11y.set_focus(outer);
        push_focusable(&mut a11y, inner);
        assert!(a11y.nodes.push(item, test_node()));
        a11y.set_active_descendant(item);
        a11y.nodes.pop(); // item
        a11y.nodes.pop(); // inner
        a11y.nodes.pop(); // outer

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, outer);
    }

    // Two composites each claim their active item: the focused one's item and,
    // inside it, an unfocused focusable one's. Only the focused composite's
    // claim counts, so there is one claim and no double-claim panic.
    #[test]
    fn focused_composite_claim_wins_over_unfocused_one() {
        let mut a11y = new_a11y();
        let focused = NodeId(1);
        let focused_item = NodeId(2);
        let unfocused = NodeId(3);
        let unfocused_item = NodeId(4);

        push_focusable(&mut a11y, focused);
        a11y.set_focus(focused);

        assert!(a11y.nodes.push(focused_item, test_node()));
        a11y.set_active_descendant(focused_item);
        a11y.nodes.pop(); // focused_item

        push_focusable(&mut a11y, unfocused);
        assert!(a11y.nodes.push(unfocused_item, test_node()));
        a11y.set_active_descendant(unfocused_item);
        a11y.nodes.pop(); // unfocused_item
        a11y.nodes.pop(); // unfocused

        a11y.nodes.pop(); // focused
        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, focused_item);
    }

    // A focused box (a held pane) holds two unfocused composites that each
    // claim their active item. Neither claim counts: the box is reported
    // focused, with no double-claim panic.
    #[test]
    fn focused_box_over_two_unfocused_composites_stays_focused() {
        let mut a11y = new_a11y();
        let pane = NodeId(1);
        let composites = [(NodeId(2), NodeId(3)), (NodeId(4), NodeId(5))];

        push_focusable(&mut a11y, pane);
        a11y.set_focus(pane);
        for (composite, item) in composites {
            push_focusable(&mut a11y, composite);
            assert!(a11y.nodes.push(item, test_node()));
            a11y.set_active_descendant(item);
            a11y.nodes.pop(); // item
            a11y.nodes.pop(); // composite
        }
        a11y.nodes.pop(); // pane

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, pane);
    }

    #[test]
    fn active_descendant_ignored_when_focus_in_other_subtree() {
        let mut a11y = new_a11y();
        let focused_container = NodeId(1);
        let focused_leaf = NodeId(2);
        let other_container = NodeId(3);
        let other_item = NodeId(4);

        // First subtree holds real focus.
        assert!(a11y.nodes.push(focused_container, test_node()));
        push_focusable(&mut a11y, focused_leaf);
        a11y.set_focus(focused_leaf);
        a11y.nodes.pop(); // focused_leaf
        a11y.nodes.pop(); // focused_container

        // Second subtree: its item claims the active descendant, but the
        // focus is not on any of its ancestors, so the gate rejects it.
        push_focusable(&mut a11y, other_container);
        assert!(a11y.nodes.push(other_item, test_node()));
        a11y.set_active_descendant(other_item);
        a11y.nodes.pop(); // other_item
        a11y.nodes.pop(); // other_container

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, focused_leaf);
    }

    #[test]
    fn active_descendant_ignored_when_nothing_focused() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let item = NodeId(2);

        push_focusable(&mut a11y, container);
        assert!(a11y.nodes.push(item, test_node()));

        // Nothing is focused (focus defaults to the root window node), so the
        // gate rejects the claim.
        a11y.set_active_descendant(item);
        a11y.nodes.pop();
        a11y.nodes.pop();

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, ROOT_NODE_ID);
    }

    #[test]
    fn regular_focus_used_when_no_active_descendant() {
        let mut builder = new_builder();
        let focused = NodeId(1);

        assert!(builder.push(focused, test_node()));
        builder.set_focus(focused);
        builder.pop();

        let update = builder.finalize();
        assert_eq!(update.focus, focused);
    }

    #[test]
    fn nearest_focusable_ancestor_excludes_self() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let inner = NodeId(2);

        push_focusable(&mut a11y, container);

        // With the container itself on top, it is not its own (strict)
        // ancestor, so it has no gate.
        assert_eq!(a11y.nodes.nearest_focusable_ancestor(&a11y.focus_ids), None);

        // A focusable inner node on top: its own registration does not count,
        // the container is its nearest focusable ancestor.
        push_focusable(&mut a11y, inner);
        assert_eq!(
            a11y.nodes.nearest_focusable_ancestor(&a11y.focus_ids),
            Some(container)
        );

        a11y.nodes.pop();
        a11y.nodes.pop();
    }

    // The double-claim guard panics only in debug builds; in release it falls
    // back to last-wins with a warning.
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "active descendant claimed by multiple nodes")
    )]
    fn multiple_active_descendant_claims_panic_in_debug() {
        let mut builder = new_builder();
        builder.set_active_descendant(NodeId(1));
        builder.set_active_descendant(NodeId(2));
    }

    // Setting focus twice in one frame means two elements both claimed window
    // focus; that panics in debug and falls back to last-wins in release.
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "set_focus called more than once")
    )]
    fn setting_focus_twice_panics_in_debug() {
        let mut builder = new_builder();
        builder.set_focus(NodeId(1));
        builder.set_focus(NodeId(2));
    }

    // Focusing a node that was never registered as focusable is a bug: panic in
    // debug, warn in release.
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "was not registered with set_focusable")
    )]
    fn set_focus_without_set_focusable() {
        let mut a11y = new_a11y();
        let node = NodeId(1);
        assert!(a11y.nodes.push(node, test_node()));
        // set_focusable was never called for `node`.
        a11y.set_focus(node);
    }

    // The focused node cannot also be its own active descendant: panic in
    // debug, warn in release.
    #[test]
    #[cfg_attr(debug_assertions, should_panic(expected = "on the focused node"))]
    fn set_active_descendant_on_focused_node() {
        let mut a11y = new_a11y();
        let node = NodeId(1);
        assert!(a11y.nodes.push(node, test_node()));
        a11y.set_focusable(node, FocusId::default());
        a11y.set_focus(node);
        a11y.set_active_descendant(node);
    }

    // Two sibling children of a focused container both claim the active
    // descendant (both pass the focus gate). The second claim is a bug: panic
    // in debug, last-wins + warn in release.
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "active descendant claimed by multiple nodes")
    )]
    fn two_siblings_claiming_active_descendant() {
        let mut a11y = new_a11y();
        let container = NodeId(1);
        let first = NodeId(2);
        let second = NodeId(3);

        assert!(a11y.nodes.push(container, test_node()));
        a11y.set_focusable(container, FocusId::default());
        a11y.set_focus(container);

        assert!(a11y.nodes.push(first, test_node()));
        a11y.set_active_descendant(first);
        a11y.nodes.pop(); // first

        assert!(a11y.nodes.push(second, test_node()));
        a11y.set_active_descendant(second);
        a11y.nodes.pop(); // second

        a11y.nodes.pop(); // container
    }

    // Node A is focused; node C (a child of the unfocused node B) claims the
    // active descendant. The final tree must still report A as focused.
    #[test]
    fn active_descendant_in_unfocused_subtree_keeps_real_focus() {
        let mut a11y = new_a11y();
        let a = NodeId(1);
        let b = NodeId(2);
        let c = NodeId(3);

        assert!(a11y.nodes.push(a, test_node()));
        a11y.set_focusable(a, FocusId::default());
        a11y.set_focus(a);
        a11y.nodes.pop(); // a

        assert!(a11y.nodes.push(b, test_node()));
        assert!(a11y.nodes.push(c, test_node()));
        a11y.set_active_descendant(c);
        a11y.nodes.pop(); // c
        a11y.nodes.pop(); // b

        let update = a11y.end_frame(Default::default());
        assert_eq!(update.focus, a);
    }
}
