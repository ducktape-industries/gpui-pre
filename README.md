# gpui-pre (ducktape-industries fork)

**What:** [`gpui-pre` 0.3.5](https://crates.io/crates/gpui-pre/0.3.5) exactly as published on crates.io (first commit), plus four small commits:

1. `test-support: the test window keeps the last accessibility tree update` — `TestWindow` retains the last AccessKit `TreeUpdate` it is sent; `TestWindow::last_a11y_tree_update` and `Window::last_a11y_tree_update` (test-support only) read it.
2. `window: a public switch activates accessibility for one window` — `Window::activate_a11y()` builds that window's tree with no assistive technology attached.
3. `div: an aria_disabled setter on the element accessibility API` — `aria_disabled(bool)` beside `aria_selected`/`aria_expanded`/`aria_toggled`.
4. `window: read the accessibility tree, its element ids, and dispatch an action at runtime` — `Window::a11y_tree()` (the last `TreeUpdate`, in every build), `Window::a11y_element_id(node)` (the `GlobalElementId` that built a node), `Window::dispatch_a11y_action(request)` (the adapter's own action path). Read-only accessors plus one public entry to the existing handler; nothing changes unless called.

**Why:** ducktape-app #114 gates merges on an accessibility contract (`ax_contract`) that reads every screen's AccessKit tree headless. Without (1) and (2) no headless test can see the tree: the test window dropped it and a window only built it once an adapter activated. (4) is for the app's opt-in loopback test door, which serves that same tree to a QA runner in a real (release) build and acts through the same path a screen reader does.

**Not upstreamed, by owner decision (2026-09-19).** No PR to zed-industries/zed and no gpui-kit issue; this private fork stays. The app consumes it through `[patch.crates-io]`, pinned by full rev.

### Re-applying on a gpui-pre bump

1. Start a branch from the new `gpui-pre` release exactly as published on crates.io (one commit: unpack the `.crate`, nothing else).
2. `git cherry-pick` the four commits above in order (2bd3361, a28ce2b, 8705c2b, 021b436); conflicts are confined to `src/window.rs`, `src/platform/test/window.rs` and `src/elements/div.rs`.
3. In ducktape-app, set the new full rev in `[patch.crates-io]`, then run `cargo test ax_contract` and the door tests: they fail if any of the four is missing.

### What the patch does (kept for a future reader)

## wasm guest without JS

The default-on `web` feature retains browser-backed clocks and randomness.
Wire-only guests use `default-features = false`; they retain GPUI's real
`StyleRefinement`, `Styled`, geometry, colors, and serialization types.
No JavaScript import is replaced with a shim.

- GPUI, scheduler, and zlog select chrono's `serde`, `std`, and `clock`
  explicitly instead of its defaults. `web` restores `chrono/wasmbind`.
  Native chrono clocks keep their existing implementations.
- `web-time` is optional in GPUI and the vendored scheduler. Scheduler exports
  `std::time::Instant` without `web`; visual tests use this shared export.
  A bare wasm guest must not call the OS-clock or platform APIs: its host
  provides timing through its guest protocol. Removing the JS dependency
  also removes js-sys's JSPI spawn-poll and wasm-bindgen registration code.
- GPUI's wasm getrandom backend and UUID `js`, `v4`, and `v7` generation
  features require `web`. Native UUID generation remains enabled by target
  dependencies. UUID values, serde, and deterministic v5 remain available.
- GPUI and scheduler use seeded rand generators without OS/thread RNG on a
  bare wasm target. Native target dependencies retain rand's defaults;
  `web` restores OS/thread RNG for browser builds.
- Scheduler disables flume defaults for a guest, retaining its async channel
  support. Its unused select/eventual-fairness features otherwise enable
  `fastrand/js`, which imports getrandom's browser backend. Native targets
  and `web` retain flume defaults. Browser worker threads imply `web`.

Consumers must patch `gpui-pre`, `gpui-pre-scheduler`, and `gpui-pre-zlog`
from the same fork revision. The two sibling packages live in `vendor/`;
`README.ducktape.md` records each original crates.io archive SHA-256. Cargo
ignores dependency manifests' patch tables, so patches belong in each
consumer workspace's `[patch.crates-io]` table, not this dependency.

On a gpui-pre bump, refresh these two sibling sources and archive hashes,
reapply the dependency feature/target gates and scheduler Instant cfg,
and preserve the existing accessibility changes. Inspect the full wasm
normal dependency tree for wasm-bindgen, js-sys, and web-sys; transitive
feature unification can re-enable a browser path. Rebuild all four release
views and require the exact guest ABI (only `ducktape_view.panicked`, only
`alloc/init/tick/snapshot/restore` function exports), then run native app and
SDK gates. Compare sizes and retained functions before calling the bump
complete; importing a browser crate is an ABI change even if Rust APIs
remain compatible.

### Upstream-ready description

> **gpui: read the accessibility tree in tests, and switch it on without an adapter**
>
> Today a window builds its AccessKit tree only after the platform adapter's activation callback fires, i.e. only with a screen reader attached, and `TestWindow` ignores `a11y_tree_update`. So no test (and no in-process tool) can inspect the tree GPUI builds.
>
> - `Window::activate_a11y()` sets the same per-window flag the adapter's activation sets and refreshes, so the next frame builds and sends the tree. Nothing calls it on its own; every existing path is unchanged, and `Application::new_inaccessible` still wins. An adapter's deactivation turns it off again.
> - `TestWindow` keeps the last `TreeUpdate` it is sent (`TestWindow::last_a11y_tree_update`, and `Window::last_a11y_tree_update` under `test-support`). Each update GPUI sends carries the whole tree, so the last one is the current tree.
> - `StatefulInteractiveElement::aria_disabled(bool)`: the Disabled flag had no setter; the only route was `a11y_synthetic_children(|b| b.parent_node().set_disabled())`. Defaults to false, so no element changes.
>
> Test plan: a test opens a window on the test platform, calls `activate_a11y`, draws, and reads `last_a11y_tree_update()`; with this, ducktape-app checks every screen's tree (names, roles, states, tab reachability) in CI.

---

# Welcome to GPUI!

GPUI is a hybrid immediate and retained mode, GPU accelerated, UI framework
for Rust, designed to support a wide variety of applications.

## Getting Started

GPUI is still in active development as we work on the Zed code editor, and is still pre-1.0. There will often be breaking changes between versions. You'll also need to use the latest version of stable Rust. Add `gpui`, and optionally `gpui_platform`, to your `Cargo.toml`:

```toml
gpui = { version = "*" }
gpui_platform = { version = "*", features = ["font-kit", "wayland", "x11"] }
```

Everything in a standalone GPUI app starts with an `Application`. You can create one with `gpui_platform::application()`, which picks the windowing and text backends for the host OS, and kick off your application by passing a callback to `Application::run()`. Inside this callback, you can create a new window with `App::open_window()` and register your first root view.

```rust,no_run
use gpui::*;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        // ..
    });
}
```

### `gpui_platform`

The features on `gpui_platform` are platform-specific, so the list above is a safe cross-platform default. If you build for a single platform, you can trim it:

- **macOS** — Rendering uses Metal and is always available, but glyph rasterization needs `font-kit`. Without it, GPUI falls back to a placeholder text system that lays text out but renders no glyphs.

    ```toml
    gpui_platform = { version = "*", features = ["font-kit"] }
    ```

- **Linux / FreeBSD** — enable at least one windowing backend for desktop windows: `wayland`, `x11`, or both. These features also compile the renderer and text system, so no separate text feature is needed.

    ```toml
    gpui_platform = { version = "*", features = ["wayland", "x11"] }
    ```

- **Windows** — no features are required. Windowing uses Win32 and text uses DirectWrite. `font-kit` has no effect here.

### Additional Topics

- [Ownership and data flow](_ownership_and_data_flow)
- [Accessibility](_accessibility)

### Dependencies

GPUI has various system dependencies that it needs in order to work.

#### macOS

On macOS, GPUI uses Metal for rendering. In order to use Metal, you need to do the following:

- Install [Xcode](https://apps.apple.com/us/app/xcode/id497799835?mt=12) from the macOS App Store, or from the [Apple Developer](https://developer.apple.com/download/all/) website. Note this requires a developer account.

> Ensure you launch Xcode after installing, and install the macOS components, which is the default option.

- Install [Xcode command line tools](https://developer.apple.com/xcode/resources/)

  ```sh
  xcode-select --install
  ```

- Ensure that the Xcode command line tools are using your newly installed copy of Xcode:

  ```sh
  sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
  ```

## The Big Picture

GPUI offers three different [registers](<https://en.wikipedia.org/wiki/Register_(sociolinguistics)>) depending on your needs:

- State management and communication with `Entity`'s. Whenever you need to store application state that communicates between different parts of your application, you'll want to use GPUI's entities. Entities are owned by GPUI and are only accessible through an owned smart pointer similar to an `Rc`. See the `app::context` module for more information.

- High level, declarative UI with views. All UI in GPUI starts with a view. A view is simply an `Entity` that can be rendered, by implementing the `Render` trait. At the start of each frame, GPUI will call this render method on the root view of a given window. Views build a tree of `elements`, lay them out and style them with a tailwind-style API, and then give them to GPUI to turn into pixels. See the `div` element for an all purpose swiss-army knife of rendering.

- Low level, imperative UI with Elements. Elements are the building blocks of UI in GPUI, and they provide a nice wrapper around an imperative API that provides as much flexibility and control as you need. Elements have total control over how they and their child elements are rendered and can be used for making efficient views into large lists, implement custom layouting for a code editor, and anything else you can think of. See the `element` module for more information.

Each of these registers has one or more corresponding contexts that can be accessed from all GPUI services. This context is your main interface to GPUI, and is used extensively throughout the framework.

## Other Resources

In addition to the systems above, GPUI provides a range of smaller services that are useful for building complex applications:

- Actions are user-defined structs that are used for converting keystrokes into logical operations in your UI. Use this for implementing keyboard shortcuts, such as cmd-q. See the `action` module for more information.

- Platform services, such as `quit the app` or `open a URL` are available as methods on the `app::App`.

- An async executor that is integrated with the platform's event loop. See the `executor` module for more information.,

- The `[gpui::test]` macro provides a convenient way to write tests for your GPUI applications. Tests also have their own kind of context, a `TestAppContext` which provides ways of simulating common platform input. See `app::test_context` and `test` modules for more details.

Currently, the best way to learn about these APIs is to read the Zed source code or drop a question in the [Zed Discord](https://zed.dev/community-links). We're working on improving the documentation, creating more examples, and will be publishing more guides to GPUI on our [blog](https://zed.dev/blog).

### Guest wire size

The fork also carries `vendor/rmp-serde` 1.3.1 with shared parser operations and a
default-off `typed` decoding feature. It is a consumer patch, not a GPUI dependency.
The view wire enables typed decoding only on wasm; native MessagePack remains
unrestricted. See its `README.ducktape.md` for the archive checksum, exact accepted
shapes, unchanged encoding and bump checks. Consumers patch `rmp-serde` to the
same fork revision alongside the scheduler and logger. Guest builds additionally
share serialization writers and use a pinned Binaryen optimizer; GPUI itself was
not the main remaining size contributor after the JS gates removed its exports.
