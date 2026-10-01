# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-linux/gpui-pre-linux-0.3.7.crate
Archive SHA-256: `0aac7022e347409454777701082201742710052813964a1e25160f7e22c965ad`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `X11Window` overrides it with one `ConfigureWindow` carrying x, y, width and height. Wayland keeps the trait default (size only): its clients can't position their windows.
Drop this vendored copy when the locked upstream release can move a window.

A held key's repeat: X11 sends it as one more `KeyPress`, which upstream
reported with `is_held: false`. The client now asks XKB for detectable
auto-repeat at connect, so the server sends no release before each repeat
(upstream dropped that release in `process_x11_events` only when the next
press came in the same read within 20 ms; across two reads it passed as a
real release and press). `X11ClientState::keys_down` holds the keys pressed
and not released; a press of one of them is reported `is_held`, and the set
is cleared when the window loses the keyboard (`FocusOut`), since a key
released then sends no release here.
