//! A single-line gpui-base `Input` on iOS, logging every change the field
//! sees, so the system keyboard's behaviour (Korean composition, Return) can
//! be read from the app's stderr by a simulator gate.

#[cfg(target_os = "ios")]
use gpui::{Context, Entity, Window, WindowOptions, div, prelude::*, px, rgb};
#[cfg(target_os = "ios")]
use gpui_base::input::{Input, InputEvent, InputState};

#[cfg(target_os = "ios")]
struct Probe {
    input: Entity<InputState>,
    enters: usize,
}

#[cfg(target_os = "ios")]
impl Probe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("type here"));
        cx.subscribe(&input, |this, input, event, cx| {
            match event {
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
            }
        })
        .detach();
        Self { input, enters: 0 }
    }
}

#[cfg(target_os = "ios")]
impl Render for Probe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.input.read(cx).text().to_string();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .pt(px(80.))
            .px(px(16.))
            .bg(rgb(0xffffff))
            .text_color(rgb(0x101622))
            .child(div().h(px(44.)).child(Input::new(&self.input)))
            .child(div().text_sm().child(format!("value: {content}")))
            .child(div().text_sm().child(format!("enters: {}", self.enters)))
            .child(div().flex_1())
    }
}

#[cfg(target_os = "ios")]
fn main() {
    gpui::Application::with_platform(gpui_ios::current_platform(false)).run(|cx| {
        gpui_base::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| cx.new(|cx| Probe::new(window, cx)))
            .unwrap();
        cx.activate(true);
        eprintln!("PROBE started");
    });
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("Build this example for an iOS target and package it with build-simulator.sh.");
}
