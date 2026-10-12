# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-linux/gpui-pre-linux-0.3.9.crate
Archive SHA-256: `e28213c3fde6f13e91ba0b74b90cbaadec9373e36a031eda98561d31059870f3`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

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

AccessKit 0.25 (`Cargo.toml` only): `accesskit` 0.24.0 → 0.25.1 and
`accesskit_unix` 0.22 → 0.24.0, which brings `accesskit_atspi_common` 0.21:
a disabled node no longer reads `enabled` and `sensitive` over AT-SPI. See
the fork README's "AccessKit 0.25"; drop these two lines when the published
crate asks for accesskit 0.25 or later.
