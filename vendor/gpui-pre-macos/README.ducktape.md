# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-macos/gpui-pre-macos-0.3.8.crate
Archive SHA-256: `0d756813ff1cd741a502af60fd18869e0677795936d7349642b41c49f43120c0`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `MacWindow` overrides it with `setFrame:display:`, converting the top-left bounds on the window's screen back to AppKit's bottom-left frame (the inverse of `bounds`).
Drop this vendored copy when the locked upstream release can move a window.
