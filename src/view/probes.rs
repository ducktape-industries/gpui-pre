//! The fork's view-cache semantics, pinned as numbers: which views render
//! when a sibling, a child or a read model notifies, and (P7) that a cached
//! view keeps its accessibility nodes, side entries, action listeners and
//! focus while its prepaint is reused, (P8) that a focus move re-renders
//! the two views that own the old and the new focus, not the window, and
//! (P9) the cached `in_focus` readers under either focus with them.
//!
//! P1-P6 came from the shell-rewrite design probes; a per-view render counter
//! is the only instrument.

use crate::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, MouseButton, Render,
    StyleRefinement, Subscription, TestAppContext, VisualTestContext, Window, anchored, deferred,
    div, prelude::*, px, size,
};
use crate::{ElementId, window::a11y::ROOT_NODE_ID};
use accesskit::{Action, ActionRequest, NodeId, Role, TreeId, TreeUpdate};
use std::{cell::Cell, rc::Rc};

type Count = Rc<Cell<u32>>;

fn counts<const N: usize>(counters: &[Count; N]) -> [u32; N] {
    std::array::from_fn(|i| counters[i].get())
}

struct Leaf {
    renders: Count,
    value: u32,
    model: Option<Entity<Model>>,
    child: Option<Entity<Leaf>>,
    cache_child: bool,
    _observe: Option<Subscription>,
}

struct Model {
    v: u32,
}

impl Leaf {
    fn new(renders: &Count) -> Self {
        Self {
            renders: renders.clone(),
            value: 0,
            model: None,
            child: None,
            cache_child: false,
            _observe: None,
        }
    }
}

impl Render for Leaf {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let read = self.model.as_ref().map_or(0, |m| m.read(cx).v);
        div()
            .size(px(20.))
            .child(format!("{} {}", self.value, read))
            .children(self.child.clone().map(|child| {
                let child = AnyView::from(child);
                match self.cache_child {
                    true => child
                        .cached(StyleRefinement::default().size(px(10.)))
                        .into_any_element(),
                    false => child.into_any_element(),
                }
            }))
    }
}

struct Root {
    renders: Count,
    desk: Entity<Leaf>,
    dot: Option<Entity<Leaf>>,
    /// Write into the cached desk during this render (a render-time write).
    push: bool,
    plain_desk: bool,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        if self.push {
            let n = self.renders.get();
            self.desk.update(cx, |desk, cx| {
                desk.value = n;
                cx.notify();
            });
        }
        let desk = match self.plain_desk {
            true => AnyView::from(self.desk.clone()).into_any_element(),
            false => AnyView::from(self.desk.clone())
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
        };
        div().size_full().child(desk).children(self.dot.clone())
    }
}

fn open<V: Render + 'static>(
    cx: &mut TestAppContext,
    make: impl FnOnce(&mut App) -> V,
) -> (Entity<V>, VisualTestContext) {
    let window = cx.open_window(size(px(400.), px(400.)), move |_, cx| make(cx));
    let root = window.root(cx).unwrap();
    let native = VisualTestContext::from_window(window.into(), cx);
    native.run_until_parked();
    (root, native)
}

/// P1: a sibling of a cached view notifying itself (as the pulse's animation
/// does) re-renders the window root and itself, never the cached sibling.
#[gpui::test]
fn p1_a_sibling_notify_leaves_the_cached_view_alone(cx: &mut TestAppContext) {
    let c: [Count; 3] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| Root {
        renders: k[0].clone(),
        desk: cx.new(|_| Leaf::new(&k[1])),
        dot: Some(cx.new(|_| Leaf::new(&k[2]))),
        push: false,
        plain_desk: false,
    });
    let before = counts(&c);
    let dot = root.read_with(&native, |root, _| root.dot.clone().unwrap());
    for _ in 0..10 {
        dot.update(&mut native, |_, cx| cx.notify());
        native.run_until_parked();
    }
    let after = counts(&c);
    eprintln!("P1 [root, cached desk, dot]: {before:?} -> {after:?}");
    assert_eq!(before, [1, 1, 1]);
    assert_eq!(after, [11, 1, 11]);
}

/// P2: a child inside the cached view notifying itself dirties every
/// ancestor: the cached view renders again each time.
#[gpui::test]
fn p2_a_nested_notify_re_renders_the_cached_ancestor(cx: &mut TestAppContext) {
    let c: [Count; 3] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| {
        let dot = cx.new(|_| Leaf::new(&k[2]));
        let desk = cx.new(|_| Leaf {
            child: Some(dot),
            ..Leaf::new(&k[1])
        });
        Root {
            renders: k[0].clone(),
            desk,
            dot: None,
            push: false,
            plain_desk: false,
        }
    });
    let before = counts(&c);
    let dot = root.read_with(&native, |root, cx| {
        root.desk.read(cx).child.clone().unwrap()
    });
    for _ in 0..10 {
        dot.update(&mut native, |_, cx| cx.notify());
        native.run_until_parked();
    }
    let after = counts(&c);
    eprintln!("P2 [root, cached desk, nested dot]: {before:?} -> {after:?}");
    assert_eq!(before, [1, 1, 1]);
    assert_eq!(after, [11, 11, 11]);
}

/// P3: a notify issued during the window's own draw (a render-time write into
/// a cached child) is not seen by that draw and leaves the window clean: the
/// child shows the old value until some other frame comes.
#[gpui::test]
fn p3_a_mid_draw_notify_is_lost_until_another_frame(cx: &mut TestAppContext) {
    let c: [Count; 3] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| Root {
        renders: k[0].clone(),
        desk: cx.new(|_| Leaf::new(&k[1])),
        dot: Some(cx.new(|_| Leaf::new(&k[2]))),
        push: false,
        plain_desk: false,
    });
    let before = counts(&c);
    root.update(&mut native, |root, cx| {
        root.push = true;
        cx.notify();
    });
    native.run_until_parked();
    let after_push = counts(&c);
    // nothing else asked for a frame: does one come?
    for _ in 0..5 {
        native.run_until_parked();
    }
    let idle = counts(&c);
    // an unrelated frame (the dot) comes: the pending write finally shows
    let dot = root.read_with(&native, |root, _| root.dot.clone().unwrap());
    root.update(&mut native, |root, _| root.push = false);
    dot.update(&mut native, |_, cx| cx.notify());
    native.run_until_parked();
    let after_unrelated = counts(&c);
    eprintln!(
        "P3 [root, cached desk, dot]: {before:?} -> push {after_push:?} -> idle {idle:?} -> unrelated frame {after_unrelated:?}"
    );
    assert_eq!(
        after_push,
        [2, 1, 2],
        "the mid-draw write showed in its own draw"
    );
    assert_eq!(idle, after_push, "the window drew again by itself");
    assert_eq!(after_unrelated, [3, 2, 3]);
}

/// P4: a cached view that reads a model in render but does not observe it is
/// not re-rendered when the model notifies; observing it fixes that. (The
/// window still draws: a render that read the entity tracked it, so its
/// notify dirties the window, but not the cached view.)
#[gpui::test]
fn p4_a_cached_view_must_observe_what_it_reads(cx: &mut TestAppContext) {
    for observe in [false, true] {
        let c: [Count; 2] = Default::default();
        let k = c.clone();
        let model = cx.new(|_| Model { v: 0 });
        let m = model.clone();
        let (_root, mut native) = open(cx, move |cx| Root {
            renders: k[0].clone(),
            desk: cx.new(|cx| Leaf {
                model: Some(m.clone()),
                _observe: observe.then(|| cx.observe(&m, |_, _, cx| cx.notify())),
                ..Leaf::new(&k[1])
            }),
            dot: None,
            push: false,
            plain_desk: false,
        });
        let before = counts(&c);
        model.update(&mut native, |model, cx| {
            model.v += 1;
            cx.notify();
        });
        native.run_until_parked();
        let after = counts(&c);
        eprintln!("P4 observe={observe} [root, cached desk]: {before:?} -> {after:?}");
        assert_eq!(before, [1, 1]);
        assert_eq!(after, if observe { [2, 2] } else { [2, 1] });
    }
}

/// P5: a cached view that misses re-renders every cached view nested in it,
/// dirty or not (the miss path sets `window.refreshing`); under an uncached
/// parent the same nested cached view hits.
#[gpui::test]
fn p5_a_cached_miss_throws_away_the_caches_inside_it(cx: &mut TestAppContext) {
    for plain_desk in [true, false] {
        let c: [Count; 3] = Default::default();
        let k = c.clone();
        let (root, mut native) = open(cx, move |cx| {
            let inner = cx.new(|_| Leaf::new(&k[2]));
            let desk = cx.new(|_| Leaf {
                child: Some(inner),
                cache_child: true,
                ..Leaf::new(&k[1])
            });
            Root {
                renders: k[0].clone(),
                desk,
                dot: None,
                push: false,
                plain_desk,
            }
        });
        let before = counts(&c);
        let desk = root.read_with(&native, |root, _| root.desk.clone());
        for _ in 0..5 {
            desk.update(&mut native, |_, cx| cx.notify());
            native.run_until_parked();
        }
        let after = counts(&c);
        eprintln!(
            "P5 desk {} [root, desk, cached inner]: {before:?} -> {after:?}",
            if plain_desk { "uncached" } else { "cached" }
        );
        assert_eq!(before, [1, 1, 1]);
        assert_eq!(after, if plain_desk { [6, 6, 1] } else { [6, 6, 6] });
    }
}

struct Layered {
    renders: Count,
    chrome: Entity<Leaf>,
    layer: Entity<Leaf>,
    dot: Entity<Leaf>,
}

impl Render for Layered {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .size_full()
            .child(
                AnyView::from(self.chrome.clone()).cached(StyleRefinement::default().size(px(50.))),
            )
            .child(self.layer.clone())
            .child(self.dot.clone())
    }
}

/// P6: an uncached layer view under the root renders on every frame the root
/// renders, even when only a sibling notified. Its cached child still hits.
#[gpui::test]
fn p6_an_uncached_layer_renders_on_every_sibling_notify(cx: &mut TestAppContext) {
    let c: [Count; 5] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| {
        let inner = cx.new(|_| Leaf::new(&k[3]));
        Layered {
            renders: k[0].clone(),
            chrome: cx.new(|_| Leaf::new(&k[1])),
            layer: cx.new(|_| Leaf {
                child: Some(inner),
                cache_child: true,
                ..Leaf::new(&k[2])
            }),
            dot: cx.new(|_| Leaf::new(&k[4])),
        }
    });
    let before = counts(&c);
    let dot = root.read_with(&native, |r, _| r.dot.clone());
    for _ in 0..10 {
        dot.update(&mut native, |_, cx| cx.notify());
        native.run_until_parked();
    }
    let after = counts(&c);
    eprintln!(
        "P6 [root, cached chrome, uncached layer, cached inner, dot]: {before:?} -> {after:?}"
    );
    assert_eq!(before, [1, 1, 1, 1, 1]);
    assert_eq!(after, [11, 1, 11, 1, 11]);
}

/// A cached view with an accessible button inside: `card` (Group) holding
/// `btn` (Button, focusable, with a Click listener), directly or through a
/// `deferred(anchored())`.
struct Card {
    renders: Count,
    btn: FocusHandle,
    clicks: Count,
    /// Mouse presses released on the card itself (no click or a11y listener:
    /// a Click action on its node falls back to a synthesized press at its
    /// bounds' centre).
    card_clicks: Count,
    deferred: bool,
}

impl Render for Card {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let clicks = self.clicks.clone();
        let btn = div()
            .id("btn")
            .role(Role::Button)
            .track_focus(&self.btn)
            .size(px(10.))
            .on_a11y_action(Action::Click, move |_, _, _| clicks.set(clicks.get() + 1));
        let card_clicks = self.card_clicks.clone();
        let card = div()
            .id("card")
            .role(Role::Group)
            .size(px(40.))
            .on_mouse_up(MouseButton::Left, move |_, _, _| {
                card_clicks.set(card_clicks.get() + 1)
            });
        match self.deferred {
            true => card.child(deferred(anchored().child(btn))),
            false => card.child(btn),
        }
    }
}

struct Shell {
    renders: Count,
    card: Entity<Card>,
    dot: Entity<Leaf>,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .id("shell")
            .role(Role::Group)
            .size_full()
            .child(
                AnyView::from(self.card.clone()).cached(StyleRefinement::default().size(px(40.))),
            )
            .child(self.dot.clone())
    }
}

fn tree(native: &mut VisualTestContext) -> TreeUpdate {
    native.update(|window, _| window.a11y_tree().cloned().expect("a tree was built"))
}

fn node_with_role(tree: &TreeUpdate, role: Role) -> NodeId {
    let mut ids = tree.nodes.iter().filter(|(_, n)| n.role() == role);
    let (id, _) = ids.next().unwrap_or_else(|| panic!("no {role:?} node"));
    assert!(ids.next().is_none(), "more than one {role:?} node");
    *id
}

fn parent_of(tree: &TreeUpdate, child: NodeId) -> NodeId {
    let mut parents = tree
        .nodes
        .iter()
        .filter(|(_, n)| n.children().contains(&child))
        .map(|(id, _)| *id);
    let parent = parents.next().unwrap_or_else(|| {
        let dump: Vec<_> = tree
            .nodes
            .iter()
            .map(|(id, n)| (*id, n.role(), n.children().to_vec()))
            .collect();
        panic!("{child:?} has no parent in {dump:#?}")
    });
    assert!(parents.next().is_none(), "{child:?} has two parents");
    parent
}

fn request(action: Action, target_node: NodeId) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node,
        data: None,
    }
}

/// P7 / P7b: with accessibility active, ten sibling notifies leave the cached
/// card un-rendered while every frame's tree still holds its nodes under the
/// right parents; between frames the door's reads (`a11y_element_id`, a Click
/// action) still answer, and a Focus into the card survives the next reuse.
fn a11y_nodes_survive_cache_reuse(cx: &mut TestAppContext, deferred: bool) {
    let c: [Count; 3] = Default::default();
    let (clicks, card_clicks) = (Count::default(), Count::default());
    let (k, kc, kcc) = (c.clone(), clicks.clone(), card_clicks.clone());
    let (root, mut native) = open(cx, move |cx| Shell {
        renders: k[0].clone(),
        card: cx.new(|cx| Card {
            renders: k[1].clone(),
            btn: cx.focus_handle(),
            clicks: kc,
            card_clicks: kcc,
            deferred,
        }),
        dot: cx.new(|_| Leaf::new(&k[2])),
    });
    native.update(|window, _| window.activate_a11y());
    native.run_until_parked();

    let first = tree(&mut native);
    let btn = node_with_role(&first, Role::Button);
    // Two Group nodes: the shell (a root child) and the card (under the shell).
    let groups: Vec<NodeId> = first
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == Role::Group)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(groups.len(), 2, "shell and card");
    let (shell, card) = if parent_of(&first, groups[0]) == ROOT_NODE_ID {
        (groups[0], groups[1])
    } else {
        (groups[1], groups[0])
    };
    let check = |tree: &TreeUpdate, frame: &str| {
        assert_eq!(
            parent_of(tree, shell),
            ROOT_NODE_ID,
            "{frame}: shell parent"
        );
        assert_eq!(parent_of(tree, card), shell, "{frame}: card parent");
        // a deferred button is the card's, as one drawn in place is
        assert_eq!(parent_of(tree, btn), card, "{frame}: btn parent");
        assert_eq!(node_with_role(tree, Role::Button), btn, "{frame}: btn id");
    };
    check(&first, "fresh");

    let before = counts(&c);
    let dot = root.read_with(&native, |r, _| r.dot.clone());
    for i in 0..10 {
        dot.update(&mut native, |_, cx| cx.notify());
        native.run_until_parked();
        check(&tree(&mut native), &format!("reused frame {i}"));
    }
    let after = counts(&c);
    eprintln!(
        "P7{} [root, cached card, dot]: {before:?} -> {after:?}",
        if deferred { "b" } else { "" }
    );
    assert_eq!(before, [2, 2, 2], "the a11y activation refreshed once");
    assert_eq!(after, [12, 2, 12]);

    // Between frames, after a reused frame: the door's reads.
    native.update(|window, cx| {
        let element = window
            .a11y_element_id(btn)
            .expect("btn still has an element id");
        assert_eq!(element.0.last(), Some(&ElementId::from("btn")));
        window.dispatch_a11y_action(request(Action::Click, btn), cx);
    });
    assert_eq!(
        clicks.get(),
        1,
        "the Click listener was reached after a reused frame"
    );

    // Focus into the card: the focus frame re-renders everything (M1); the
    // reuse that follows carries the focused node.
    native.update(|window, cx| window.dispatch_a11y_action(request(Action::Focus, btn), cx));
    native.run_until_parked();
    assert_eq!(tree(&mut native).focus, btn, "focused on the fresh frame");
    dot.update(&mut native, |_, cx| cx.notify());
    native.run_until_parked();
    let focused = tree(&mut native);
    check(&focused, "reused after focus");
    assert_eq!(focused.focus, btn, "focus survived the reuse");
    let handle = root.read_with(&native, |r, cx| r.card.read(cx).btn.clone());
    assert!(native.update(|window, _| handle.is_focused(window)));
    assert_eq!(
        counts(&c),
        [14, 3, 14],
        "one miss for the focus frame, then a hit"
    );

    // A Click on a node without a listener falls back to a synthesized mouse
    // click at the node's replayed bounds, through the replayed hitboxes.
    native.update(|window, cx| window.dispatch_a11y_action(request(Action::Click, card), cx));
    assert_eq!(
        card_clicks.get(),
        1,
        "the fallback click reached the card after a reused frame"
    );
    assert_eq!(clicks.get(), 1, "the card's centre is off the button");
}

#[gpui::test]
fn p7_a11y_nodes_survive_cache_reuse(cx: &mut TestAppContext) {
    a11y_nodes_survive_cache_reuse(cx, false);
}

#[gpui::test]
fn p7b_a11y_nodes_survive_cache_reuse_through_a_deferred_child(cx: &mut TestAppContext) {
    a11y_nodes_survive_cache_reuse(cx, true);
}

struct Row {
    renders: Count,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .id("row")
            .role(Role::ListBoxOption)
            .size(px(10.))
            .aria_active_descendant()
    }
}

struct ListShell {
    list: FocusHandle,
    row: Entity<Row>,
    dot: Entity<Leaf>,
}

impl Render for ListShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("list")
                    .role(Role::ListBox)
                    .track_focus(&self.list)
                    .size(px(40.))
                    .child(
                        AnyView::from(self.row.clone())
                            .cached(StyleRefinement::default().size(px(10.))),
                    ),
            )
            .child(self.dot.clone())
    }
}

#[gpui::test]
fn p7c_a_claim_under_an_outer_focus_survives_reuse(cx: &mut TestAppContext) {
    let rows = Count::default();
    let r = rows.clone();
    let (root, mut native) = open(cx, move |cx| ListShell {
        list: cx.focus_handle(),
        row: cx.new(|_| Row { renders: r }),
        dot: cx.new(|_| Leaf::new(&Count::default())),
    });
    native.update(|window, _| window.activate_a11y());
    native.run_until_parked();
    let list = root.read_with(&native, |r, _| r.list.clone());
    let pre = rows.get();
    native.update(|window, cx| window.focus(&list, cx));
    native.run_until_parked();
    assert_eq!(rows.get(), pre, "the focus frame reused the row");
    let fresh = tree(&mut native);
    let row = node_with_role(&fresh, Role::ListBoxOption);
    assert_eq!(
        fresh.focus, row,
        "focus frame: the replayed claim is judged by this frame's focus"
    );
    let before = rows.get();
    let dot = root.read_with(&native, |r, _| r.dot.clone());
    dot.update(&mut native, |_, cx| cx.notify());
    native.run_until_parked();
    assert_eq!(rows.get(), before, "the row was reused");
    let reused = tree(&mut native);
    assert_eq!(reused.focus, row, "reused frame: the claim wins");
}

/// A cached view that owns a focus handle, or reads another view's.
struct Stop {
    renders: Count,
    handle: FocusHandle,
    /// The `is_focused` read of a handle this view did not render (P8's stale reader).
    reads: Option<FocusHandle>,
}

impl Render for Stop {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let read = self.reads.as_ref().map(|h| h.is_focused(window));
        div()
            .size(px(20.))
            .track_focus(&self.handle)
            .child(format!("{:?} {read:?}", self.handle.is_focused(window)))
    }
}

struct Stops {
    renders: Count,
    stops: Vec<Entity<Stop>>,
}

impl Render for Stops {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().children(self.stops.iter().map(|stop| {
            AnyView::from(stop.clone()).cached(StyleRefinement::default().size(px(20.)))
        }))
    }
}

/// P8 (F3): a focus move re-renders the views that rendered the old and the
/// new focus ids, and the root over them; a cached sibling holding neither
/// hits. A fourth cached view that reads C's handle without observing focus
/// keeps its stale text (the s18 audit's class).
#[gpui::test]
fn p8_a_focus_move_re_renders_the_two_views_that_own_it(cx: &mut TestAppContext) {
    let c: [Count; 5] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| {
        let stop = |i: usize, reads: Option<FocusHandle>, cx: &mut App| {
            let handle = cx.focus_handle();
            cx.new(|_| Stop {
                renders: k[i].clone(),
                handle,
                reads,
            })
        };
        let a = stop(1, None, cx);
        let b = stop(2, None, cx);
        let c = stop(3, None, cx);
        let c_handle = c.read(cx).handle.clone();
        let d = stop(4, Some(c_handle), cx);
        Stops {
            renders: k[0].clone(),
            stops: vec![a, b, c, d],
        }
    });
    let stops = root.read_with(&native, |root, _| root.stops.clone());
    let handle =
        |i: usize, native: &VisualTestContext| stops[i].read_with(native, |s, _| s.handle.clone());
    let (b_handle, c_handle) = (handle(1, &native), handle(2, &native));
    let before = counts(&c);
    native.update(|window, cx| window.focus(&b_handle, cx));
    native.run_until_parked();
    let after_b = counts(&c);
    native.update(|window, cx| window.focus(&c_handle, cx));
    native.run_until_parked();
    let after_c = counts(&c);
    eprintln!(
        "P8 [root, A, B, C, D reads C]: {before:?} -> focus(B) {after_b:?} -> focus(C) {after_c:?}"
    );
    assert_eq!(before, [1, 1, 1, 1, 1]);
    assert_eq!(after_b, [2, 1, 2, 1, 1]);
    assert_eq!(after_c, [3, 1, 3, 2, 1]);
    assert!(native.update(|window, _| c_handle.is_focused(window)));
}

// P8b: `blur` and a focus move made by a focus listener during a draw take the
// same two-view path. B's listener forwards focus to C; then the window blurs.
#[gpui::test]
fn p8b_blur_and_a_listener_move_re_render_two_views(cx: &mut TestAppContext) {
    let c: [Count; 5] = Default::default();
    let k = c.clone();
    let (root, mut native) = open(cx, move |cx| {
        let stop = |i: usize, cx: &mut App| {
            let handle = cx.focus_handle();
            cx.new(|_| Stop {
                renders: k[i].clone(),
                handle,
                reads: None,
            })
        };
        Stops {
            renders: k[0].clone(),
            stops: vec![stop(1, cx), stop(2, cx), stop(3, cx), stop(4, cx)],
        }
    });
    native.update(|window, _| window.activate_window());
    native.run_until_parked();
    let base = counts(&c);
    let stops = root.read_with(&native, |root, _| root.stops.clone());
    let handle =
        |i: usize, native: &VisualTestContext| stops[i].read_with(native, |s, _| s.handle.clone());
    let (b_handle, c_handle) = (handle(1, &native), handle(2, &native));
    let b = stops[1].clone();
    let fwd = c_handle.clone();
    let _sub = native.update(|window, cx| {
        b.update(cx, |_, cx| {
            cx.on_focus(&b_handle, window, move |_, window, cx| {
                window.focus(&fwd, cx)
            })
        })
    });
    native.update(|window, cx| window.focus(&b_handle, cx));
    native.run_until_parked();
    // The test platform draws on a flush; the draw-end move left the window dirty.
    native.update(|_, _| ());
    let after_fwd = counts(&c);
    assert!(native.update(|window, _| c_handle.is_focused(window)));
    native.update(|window, cx| window.blur(cx));
    native.run_until_parked();
    let after_blur = counts(&c);
    eprintln!("P8b {base:?} -> fwd {after_fwd:?} -> blur {after_blur:?}");
    let d = |a: [u32; 5]| std::array::from_fn::<u32, 5, _>(|i| a[i] - base[i]);
    assert_eq!(
        d(after_fwd),
        [2, 0, 2, 1, 0],
        "listener move: B and C, not A or D"
    );
    let blur = std::array::from_fn::<u32, 5, _>(|i| after_blur[i] - after_fwd[i]);
    assert_eq!(blur, [1, 0, 0, 1, 0], "blur: root and C only");
    assert_eq!(d(after_blur), [3, 0, 2, 2, 0], "totals since the start");
}

/// A cached view whose div styles itself by `in_focus`. Its render records
/// what `within_focused` read, which is what that style follows.
struct Within {
    renders: Count,
    handle: FocusHandle,
    within: Rc<Cell<bool>>,
}

impl Render for Within {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        self.within.set(self.handle.within_focused(window, cx));
        div()
            .size(px(20.))
            .track_focus(&self.handle)
            .in_focus(|s| s.opacity(0.5))
    }
}

/// The window root: renders its own focusable live over two cached views.
struct Over {
    renders: Count,
    own: FocusHandle,
    stop: Entity<Stop>,
    reader: Entity<Within>,
}

impl Render for Over {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let cached = |view: AnyView| view.cached(StyleRefinement::default().size(px(20.)));
        div()
            .size_full()
            .track_focus(&self.own)
            .child(cached(AnyView::from(self.stop.clone())))
            .child(cached(AnyView::from(self.reader.clone())))
    }
}

/// P9: a cached view whose div styles itself by `in_focus` under a focusable
/// that an outer view renders live (the window root here; a cached owner
/// re-renders its whole subtree, P5) redraws when that focusable gains or
/// loses the focus, and its render sees the move. The first move does not
/// touch the reader, so its flag must survive one reuse. The cached sibling
/// with no such reader keeps its cache, so P8's two-view saving stands.
#[gpui::test]
fn p9_a_focus_move_re_renders_the_in_focus_readers_under_it(cx: &mut TestAppContext) {
    let c: [Count; 3] = Default::default();
    let k = c.clone();
    let within = Rc::new(Cell::new(false));
    let seen = within.clone();
    let (root, mut native) = open(cx, move |cx| {
        let stop_handle = cx.focus_handle();
        let stop = cx.new(|_| Stop {
            renders: k[1].clone(),
            handle: stop_handle,
            reads: None,
        });
        let reader_handle = cx.focus_handle();
        let reader = cx.new(|_| Within {
            renders: k[2].clone(),
            handle: reader_handle,
            within: seen,
        });
        Over {
            renders: k[0].clone(),
            own: cx.focus_handle(),
            stop,
            reader,
        }
    });
    let (own, stop_handle) = root.read_with(&native, |root, cx| {
        (root.own.clone(), root.stop.read(cx).handle.clone())
    });
    let start = counts(&c);
    native.update(|window, cx| window.focus(&stop_handle, cx));
    native.run_until_parked();
    let after_stop = (counts(&c), within.get());
    native.update(|window, cx| window.focus(&own, cx));
    native.run_until_parked();
    let after_own = (counts(&c), within.get());
    native.update(|window, cx| window.focus(&stop_handle, cx));
    native.run_until_parked();
    let after_leave = (counts(&c), within.get());
    eprintln!(
        "P9 [root, stop, reader] (within): {start:?} -> focus(stop) {after_stop:?} -> focus(own) {after_own:?} -> focus(stop) {after_leave:?}"
    );
    assert_eq!(start, [1, 1, 1]);
    assert_eq!(
        after_stop,
        ([2, 2, 1], false),
        "focus(stop): the stop only; the reader is reused"
    );
    assert_eq!(
        after_own,
        ([3, 3, 2], true),
        "focus(own): the stop it left, the root and the reader under it"
    );
    assert_eq!(
        after_leave,
        ([4, 4, 3], false),
        "focus(stop): the stop, and the reader that left the focused subtree"
    );
}

/// A list clipped to 40 px whose row lies 100 px down, out of view, and a
/// button under an occluding cover, and a link that answers Click itself;
/// every element counts its clicks (the link its answers too) and the window
/// root counts every pointer press it is hit by.
struct Pressables {
    row: Count,
    button: Count,
    cover: Count,
    presses: Count,
    link: Count,
    link_answers: Count,
}

/// A listener that counts its calls on `count`.
fn counted<E>(count: &Count) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let count = count.clone();
    move |_, _, _| count.set(count.get() + 1)
}

impl Render for Pressables {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, counted(&self.presses))
            .child(
                div()
                    .id("list")
                    .size(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(div().h(px(100.)).flex_shrink_0())
                    .child(
                        div()
                            .id("row")
                            .role(Role::ListBoxOption)
                            .size(px(20.))
                            .flex_shrink_0()
                            .on_click(counted(&self.row)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .size(px(40.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("button")
                            .role(Role::Button)
                            .size(px(40.))
                            .on_click(counted(&self.button)),
                    )
                    .child(
                        div()
                            .id("cover")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(px(40.))
                            .occlude()
                            .on_click(counted(&self.cover)),
                    ),
            )
            .child({
                let answers = self.link_answers.clone();
                div()
                    .id("link")
                    .role(Role::Link)
                    .size(px(20.))
                    .flex_shrink_0()
                    .on_click(counted(&self.link))
                    .on_a11y_action(Action::Click, move |_, _, _| answers.set(answers.get() + 1))
            })
    }
}

/// A Click from assistive technology on an element with a click listener runs
/// that listener, whether the element is clipped out of its list or covered by
/// another, and no pointer press is synthesized at its centre: the row's and
/// the button's own clicks fire, the cover's and the root's presses do not.
/// An element that answers Click itself keeps its answer, and only it runs.
#[gpui::test]
fn a11y_click_runs_the_click_listener_of_a_clipped_or_covered_element(cx: &mut TestAppContext) {
    let c: [Count; 6] = Default::default();
    let k = c.clone();
    let (_, mut native) = open(cx, move |_| Pressables {
        row: k[0].clone(),
        button: k[1].clone(),
        cover: k[2].clone(),
        presses: k[3].clone(),
        link: k[4].clone(),
        link_answers: k[5].clone(),
    });
    native.update(|window, _| window.activate_a11y());
    native.run_until_parked();
    let first = tree(&mut native);
    let row = node_with_role(&first, Role::ListBoxOption);
    let button = node_with_role(&first, Role::Button);
    let link = node_with_role(&first, Role::Link);
    assert!(
        first
            .nodes
            .iter()
            .any(|(id, n)| *id == row && n.supports_action(Action::Click))
    );

    native.update(|window, cx| window.dispatch_a11y_action(request(Action::Click, row), cx));
    // [row, button, cover, presses, link, link answers]
    assert_eq!(
        counts(&c),
        [1, 0, 0, 0, 0, 0],
        "the clipped row's own click"
    );
    native.update(|window, cx| window.dispatch_a11y_action(request(Action::Click, button), cx));
    assert_eq!(
        counts(&c),
        [1, 1, 0, 0, 0, 0],
        "the covered button's own click"
    );
    native.update(|window, cx| window.dispatch_a11y_action(request(Action::Click, link), cx));
    assert_eq!(
        counts(&c),
        [1, 1, 0, 0, 0, 1],
        "the link's own answer, alone"
    );
}
