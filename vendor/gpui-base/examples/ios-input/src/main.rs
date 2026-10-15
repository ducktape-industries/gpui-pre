//! A gpui-base field on iOS, logging every change the field sees, so the
//! system keyboard's behaviour (Korean composition, Japanese marked text,
//! Return) can be read from the app's stderr by a simulator gate. The
//! single-line `Input` by default; the multi-line `Textarea` when the
//! environment has `PROBE_TEXTAREA` (`SIMCTL_CHILD_PROBE_TEXTAREA=1 xcrun simctl launch ...`).

#[cfg(target_os = "ios")]
use gpui::{Context, Entity, Window, WindowOptions, div, prelude::*, px, rgb};
#[cfg(target_os = "ios")]
use gpui_base::input::{InputBaseState, InputEvent, InputModeKind, InputState, TextareaState};

#[cfg(target_os = "ios")]
struct Probe<M: InputModeKind> {
    input: Entity<InputBaseState<M>>,
    enters: usize,
}

#[cfg(target_os = "ios")]
impl<M: InputModeKind> Probe<M> {
    fn new(input: Entity<InputBaseState<M>>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&input, |this, input, event, cx| match event {
            InputEvent::Change => {
                let state = input.read(cx);
                eprintln!(
                    "PROBE change content={:?} sel={:?}",
                    state.text().to_string(),
                    state.selected_range()
                );
            }
            InputEvent::PressEnter { .. } => {
                this.enters += 1;
                eprintln!("PROBE enter {}", this.enters);
                cx.notify();
            }
            InputEvent::Focus => eprintln!("PROBE focus"),
            InputEvent::Blur => eprintln!("PROBE blur"),
        })
        .detach();
        Self { input, enters: 0 }
    }
}

#[cfg(target_os = "ios")]
impl<M: InputModeKind> Render for Probe<M> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.input.read(cx);
        let content = state.text().to_string();
        let height = if state.is_multi_line() { 120. } else { 44. };
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .pt(px(80.))
            .px(px(16.))
            .bg(rgb(0xffffff))
            .text_color(rgb(0x101622))
            // `Input` and `Textarea` render the state entity itself.
            .child(div().h(px(height)).child(self.input.clone()))
            .child(div().text_sm().child(format!("value: {content:?}")))
            .child(div().text_sm().child(format!("enters: {}", self.enters)))
            .child(div().flex_1())
    }
}

#[cfg(target_os = "ios")]
fn main() {
    gpui::Application::with_platform(gpui_ios::current_platform(false)).run(|cx| {
        gpui_base::init(cx);
        if std::env::var_os("PROBE_TEXTAREA").is_some() {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let input = cx.new(|cx| TextareaState::new(window, cx).placeholder("type here"));
                cx.new(|cx| Probe::new(input, cx))
            })
            .unwrap();
            eprintln!("PROBE started textarea");
        } else {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("type here"));
                cx.new(|cx| Probe::new(input, cx))
            })
            .unwrap();
            eprintln!("PROBE started");
        }
        cx.activate(true);
    });
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("Build this example for an iOS target and package it with build-simulator.sh.");
}
