# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-linux/gpui-pre-linux-0.3.8.crate
Archive SHA-256: `b1131746fad5c87b5de74dc9c73105655b0c820a2dd4bb247586573e7cf5720e`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `X11Window` overrides it with one `ConfigureWindow` carrying x, y, width and height. Wayland keeps the trait default (size only): its clients can't position their windows.
Drop this vendored copy when the locked upstream release can move a window.
