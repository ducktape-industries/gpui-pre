# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-linux/gpui-pre-linux-0.3.7.crate
Archive SHA-256: `0aac7022e347409454777701082201742710052813964a1e25160f7e22c965ad`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `X11Window` overrides it with one `ConfigureWindow` carrying x, y, width and height. Wayland keeps the trait default (size only): its clients can't position their windows.
Drop this vendored copy when the locked upstream release can move a window.
