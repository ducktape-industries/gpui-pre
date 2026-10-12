# Window::set_bounds

Source: https://static.crates.io/crates/gpui-pre-macos/gpui-pre-macos-0.3.9.crate
Archive SHA-256: `c7c988b7e49775cc8542dbe48d5a10e073d8e898a023ef3ff3d24f33363e9863`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One addition: `PlatformWindow::set_bounds`, which gpui-pre declares with a
resize-only default. `MacWindow` overrides it with `setFrame:display:`, converting the top-left bounds on the window's screen back to AppKit's bottom-left frame (the inverse of `bounds`).
Drop this vendored copy when the locked upstream release can move a window.

AccessKit 0.25 (`Cargo.toml` only): `accesskit` 0.24.0 → 0.25.1 and
`accesskit_macos` 0.26.0 → 0.27.1 (a dependency move to the same
`accesskit` as gpui-pre). See the fork README's "AccessKit 0.25"; drop these
two lines when the published crate asks for accesskit 0.25 or later.
