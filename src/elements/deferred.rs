use crate::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};

/// Builds a `Deferred` element, which delays the layout and paint of its child.
pub fn deferred(child: impl IntoElement) -> Deferred {
    Deferred {
        child: Some(child.into_any_element()),
        priority: 0,
    }
}

/// An element which delays the painting of its child until after all of
/// its ancestors, while keeping its layout as part of the current element tree.
pub struct Deferred {
    child: Option<AnyElement>,
    priority: usize,
}

impl Deferred {
    /// Sets the `priority` value of the `deferred` element, which
    /// determines the drawing order relative to other deferred elements,
    /// with higher values being drawn on top.
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Element for Deferred {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let child = self.child.take().unwrap();
        let element_offset = window.element_offset();
        window.defer_draw(child, element_offset, self.priority, None)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for Deferred {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Deferred {
    /// Sets a priority for the element. A higher priority conceptually means painting the element
    /// on top of deferred draws with a lower priority (i.e. closer to the viewer).
    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AnyElement, App, Bounds, Context, Div, Element, ElementId, Entity, GlobalElementId,
        InspectorElementId, LayoutId, Modifiers, Pixels, Stateful, StyleRefinement, TestAppContext,
        Window, anchored, deferred, div, point, prelude::*, px, size,
    };
    use std::{cell::RefCell, rc::Rc};

    /// A stand-in for a dock panel hosting a popover (deferred draw) whose
    /// content opens another popover (a deferred draw created while
    /// prepainting the first one's content).
    struct PanelView;

    impl Render for PanelView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().key_context("Panel").size_full().child(
                deferred(
                    anchored().position(point(px(10.), px(10.))).child(
                        div().key_context("Popover").w(px(200.)).h(px(200.)).child(
                            deferred(
                                anchored().position(point(px(30.), px(30.))).child(
                                    div()
                                        .key_context("NestedMenu")
                                        .debug_selector(|| "NESTED_MENU".into())
                                        .w(px(50.))
                                        .h(px(50.)),
                                ),
                            )
                            .with_priority(2),
                        ),
                    ),
                )
                .with_priority(1),
            )
        }
    }

    struct RootView {
        panel: Entity<PanelView>,
    }

    impl Render for RootView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().key_context("Root").size_full().child(
                self.panel
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
        }
    }

    /// Regression test for a crash with nested deferred draws (e.g. a popover
    /// menu inside a popover hosted by a cached dock panel). Prepaint indices
    /// recorded during the deferred draw rounds must index the same
    /// `deferred_draws` vector that `reuse_prepaint` slices on the next frame;
    /// previously they were measured against a transient per-round vector, so
    /// reusing the panel's subtree grafted the wrong deferred draws and
    /// panicked in the dispatch tree.
    #[gpui::test]
    fn test_nested_deferred_draws_with_reused_views(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(800.), px(600.)), |_, cx| {
            let panel = cx.new(|_| PanelView);
            RootView { panel }
        });
        cx.run_until_parked();

        let menu_bounds = window
            .update(cx, |_, window, _| {
                window
                    .rendered_frame
                    .debug_bounds
                    .get("NESTED_MENU")
                    .copied()
            })
            .unwrap()
            .expect("NESTED_MENU debug bounds not found");
        assert_eq!(menu_bounds.size, size(px(50.), px(50.)));

        // Re-render only the root view; the panel is cached, so its subtree -
        // including both deferred draw records - is reused from the previous
        // frame.
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
        cx.run_until_parked();

        // Reuse the subtree a second time, exercising ranges that were
        // themselves recorded during a reused frame.
        window.update(cx, |_, _, cx| cx.notify()).unwrap();
        cx.run_until_parked();

        // Re-render the panel itself again to prove the popovers still draw.
        window
            .update(cx, |root, _, cx| {
                root.panel.update(cx, |_, cx| cx.notify());
            })
            .unwrap();
        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                assert_eq!(window.rendered_frame.deferred_draws.len(), 2);
                assert!(
                    window
                        .rendered_frame
                        .debug_bounds
                        .contains_key("NESTED_MENU")
                );
            })
            .unwrap();
    }

    type ClickLog = Rc<RefCell<Vec<&'static str>>>;

    /// An occluding box that logs its clicks under `name`.
    fn clicker(name: &'static str, log: &ClickLog) -> Stateful<Div> {
        let log = log.clone();
        div()
            .id(name)
            .absolute()
            .size(px(100.))
            .occlude()
            .on_click(move |_, _, _| log.borrow_mut().push(name))
    }

    /// Defers its child with the content mask it is prepainted under, as a host defers a view's
    /// overlay within the view's slot.
    struct MaskedDeferred(Option<AnyElement>);

    impl IntoElement for MaskedDeferred {
        type Element = Self;

        fn into_element(self) -> Self {
            self
        }
    }

    impl Element for MaskedDeferred {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            window: &mut Window,
            cx: &mut App,
        ) -> (LayoutId, ()) {
            (self.0.as_mut().unwrap().request_layout(window, cx), ())
        }

        fn prepaint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            window: &mut Window,
            _: &mut App,
        ) {
            let child = self.0.take().unwrap();
            let mask = window.content_mask();
            window.defer_draw(child, window.element_offset(), 0, Some(mask));
        }

        fn paint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            _: &mut (),
            _: &mut Window,
            _: &mut App,
        ) {
        }
    }

    struct SlotView(ClickLog);

    impl Render for SlotView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size(px(40.))
                .overflow_hidden()
                .child(MaskedDeferred(Some(
                    clicker("slot overlay", &self.0)
                        .left(px(30.))
                        .into_any_element(),
                )))
        }
    }

    /// A deferred draw given a content mask takes clicks only inside it, as it paints only
    /// inside it: an overlay that spills out of its 40px slot takes no click at x = 100.
    #[gpui::test]
    fn a_masked_deferred_draw_takes_no_click_outside_its_mask(cx: &mut TestAppContext) {
        let log = ClickLog::default();
        let (_, cx) = cx.add_window_view({
            let log = log.clone();
            move |_, _| SlotView(log)
        });
        cx.run_until_parked();

        cx.simulate_click(point(px(100.), px(10.)), Modifiers::none());
        assert!(
            log.borrow().is_empty(),
            "a click outside the mask reached the draw: {:?}",
            log.borrow()
        );

        cx.simulate_click(point(px(35.), px(10.)), Modifiers::none());
        assert_eq!(*log.borrow(), ["slot overlay"]);
    }

    struct RoundsView(ClickLog);

    impl Render for RoundsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(
                    deferred(div().child(deferred(clicker("round two at 0", &self.0))))
                        .with_priority(1),
                )
                .child(deferred(clicker("round one at 5", &self.0)).with_priority(5))
        }
    }

    /// Clicks reach deferred draws in paint order: by priority across every round. A draw
    /// registered while another deferred draw prepaints (round two) at priority 0 paints under
    /// a round-one draw at priority 5, so the click on the spot they share goes to the latter.
    #[gpui::test]
    fn a_later_rounds_lower_priority_draw_takes_no_click_from_above_it(cx: &mut TestAppContext) {
        let log = ClickLog::default();
        let (_, cx) = cx.add_window_view({
            let log = log.clone();
            move |_, _| RoundsView(log)
        });
        cx.run_until_parked();

        cx.simulate_click(point(px(50.), px(50.)), Modifiers::none());
        assert_eq!(*log.borrow(), ["round one at 5"]);
    }
}
