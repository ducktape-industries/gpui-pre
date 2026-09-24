# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-macos/gpui-pre-macos-0.3.5.crate
Archive SHA-256: `806eac0cb5c0eebc0b032c6450c11d283264f9ac05754dda83fc80d0b86b4ee8`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `MacWindow` overrides it with `setFrame:display:`, converting the top-left bounds on the window's screen back to AppKit's bottom-left frame (the inverse of `bounds`).
Drop this vendored copy when the locked upstream release can move a window.
