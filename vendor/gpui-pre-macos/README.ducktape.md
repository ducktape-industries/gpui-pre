# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-macos/gpui-pre-macos-0.3.7.crate
Archive SHA-256: `5a43af845b260b09393e923c4f1e1a67a10fcfc847b4815192bbfd02ed9fe725`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `MacWindow` overrides it with `setFrame:display:`, converting the top-left bounds on the window's screen back to AppKit's bottom-left frame (the inverse of `bounds`).
Drop this vendored copy when the locked upstream release can move a window.
