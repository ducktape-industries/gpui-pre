# gpui-pre (ducktape-industries fork)

**What:** [`gpui-pre` 0.3.7](https://crates.io/crates/gpui-pre/0.3.7) exactly as published on crates.io (the base commit), plus our patches. The first four are accessibility:

1. `test-support: the test window keeps the last accessibility tree update` — `TestWindow` retains the last AccessKit `TreeUpdate` it is sent; `TestWindow::last_a11y_tree_update` and `Window::last_a11y_tree_update` (test-support only) read it.
2. `window: a public switch activates accessibility for one window` — `Window::activate_a11y()` builds that window's tree with no assistive technology attached.
3. `div: an aria_disabled setter on the element accessibility API` — `aria_disabled(bool)` beside `aria_selected`/`aria_expanded`/`aria_toggled`.
4. `window: read the accessibility tree, its element ids, and dispatch an action at runtime` — `Window::a11y_tree()` (the last `TreeUpdate`, in every build), `Window::a11y_element_id(node)` (the `GlobalElementId` that built a node), `Window::dispatch_a11y_action(request)` (the adapter's own action path). Read-only accessors plus one public entry to the existing handler; nothing changes unless called.

The later ones each have a section below: link wrapping, `Window::set_bounds`, bounds observers on full-screen/maximize, one tab stop per focus handle, refused accessibility nodes, the active-descendant claim gate, accessibility nodes through cache reuse, the JavaScript-free wasm guest gates, and the vendored siblings (`vendor/`) those need.

**Why:** ducktape-app #114 gates merges on an accessibility contract (`ax_contract`) that reads every screen's AccessKit tree headless. Without (1) and (2) no headless test can see the tree: the test window dropped it and a window only built it once an adapter activated. (4) is for the app's opt-in loopback test door, which serves that same tree to a QA runner in a real (release) build and acts through the same path a screen reader does.

**Not upstreamed, by owner decision (2026-09-19).** No PR to zed-industries/zed and no gpui-kit issue; this private fork stays. The app consumes it through `[patch.crates-io]`, pinned by full rev.

### Re-applying on a gpui-pre bump

1. Start a branch from `main` whose first commit is the new `gpui-pre` release exactly as published on crates.io (one commit: the tree is the unpacked `.crate`, nothing else; `diff -rq` against the unpacked archive prints nothing).
2. `git cherry-pick -x` every patch commit of the previous bump in `git log` order (the accessibility ones: the four above, the tab-stop fix, refused nodes, the claim gate, nodes through cache reuse with `src/view/probes.rs`). A commit that carries a `vendor/` crate (the wasm-guest gates: scheduler and zlog; `set_bounds`: linux and macos; the wgpu emoji fix) is a refresh, not a plain pick: `cherry-pick -n`, replace the vendor directory with the new published archive, reapply our delta (diff of the old vendor against the old published archive), record the new archive SHA-256 in its `README.ducktape.md`, then commit with the original message plus a note. `diff -rq <published> vendor/<crate>` must list only `README.ducktape.md` and the patched files.
3. Run this crate's own tests with the vendored siblings patched in and `test-support` on: `cargo test --lib --features test-support --config 'patch.crates-io.gpui-pre-scheduler.path="vendor/gpui-pre-scheduler"' --config 'patch.crates-io.gpui-pre-zlog.path="vendor/gpui-pre-zlog"'`, and the a11y/tab_stop tests again with `--release` (`a_second_element_with_one_id_is_refused` only records in release). Two tests `include_bytes!` zed's `assets/fonts/{ibm-plex-sans,lilex}` three directories above `src/`; fetch them from zed at the snapshot rev. Then `cargo tree --target wasm32-unknown-unknown --no-default-features -e normal` must show no `wasm-bindgen`, `js-sys`, `web-sys`, `web-time` or `getrandom`. Root `Cargo.lock` is the published one and is not committed with resolver changes.
4. In ducktape-app and modules, set the new full rev in every `[patch.crates-io]` entry (all fork crates at one rev), then run `cargo test ax_contract` and the door tests: they fail if any of the accessibility commits is missing.

### Link wrapping

`LineWrapper::wrap_line` and `LineLayout::compute_wrap_boundaries` (what laid-out text such as `StyledText` wraps with) break links sensibly: never inside or right after `scheme://`, not in the authority, and after `/`, `-` or `.` in the path; in prose a `/` breaks after itself rather than before (`and/` | `or`). Upstream broke before every `/`, so `duck://a-b/c` wrapped as `duck:/` | `/a-b/c`. `test_wrap_link` and `test_wrap_boundaries_keep_links_whole_until_their_path` pin it.

### One tab stop per focus handle

`TabStopMap::insert` keeps one entry per focus handle per frame: the first, which is the outermost element's (a div inserts its own handle before its children paint). Upstream kept every insert in the order and the last in its lookup, so a field whose labelled wrapper tracks the input's own handle held two stops, and `focus_prev` from the field landed on its other entry: Shift+Tab never left it. The skipped insert is still recorded, so a cached element's replayed range keeps the stop when nothing outside it tracks the handle. `test_one_tab_stop_per_focus_handle` pins it.

### Refused accessibility nodes

`Window::a11y_refused_elements()` lists the elements whose accessibility node the last frame with accessibility active left out because an earlier node had the same id, each with that shared `NodeId`, so `a11y_element_id(id)` names the element that kept it. In a debug build a duplicate pushed live panics first, so only duplicates skipped while replaying a cached view are listed there; a release build dropped such a node without a trace, and this lets the app's test door report it. `a_second_element_with_one_id_is_refused` pins it (run it with `--release`).

### Active-descendant claim gate

An `aria_active_descendant` claim counts only when the claimant's nearest focusable ancestor (the first node below it on the accessibility stack registered with `set_focusable`) is the focused node. Upstream honored a claim under a focused ancestor at any depth, so a focused outer box (a held pane, a focusable frame) let every composite inside it claim: two claims panicked in debug ("active descendant claimed by multiple nodes") and the last won in release, telling assistive technology that a row of an unfocused composite had the focus. A focusable element with no role pushes no node and is passed over; it can never be the accessibility tree's focused node either. `active_descendant_ignored_under_unfocused_focusable_ancestor`, `focused_composite_claim_wins_over_unfocused_one` and `focused_box_over_two_unfocused_composites_stays_focused` pin it.

### Accessibility nodes survive cache reuse

A cached view (`.cached()`) whose prepaint is reused keeps its accessibility nodes. Upstream `reuse_prepaint` replayed hitboxes, tooltips, element states, layouts, the dispatch subtree and deferred draws but built no nodes, so under a screen reader a cached view vanished from the tree (and its action listeners, focus and bounds with it) on every frame that hit the cache; the only remedy was to switch caching off while accessibility was active. Now `A11yNodeBuilder` logs `(parent, id, node)` beside every node it finishes, `PrepaintStateIndex` records the log position, and `A11y::begin_frame` moves the previous frame's log and side maps (`focus_ids`, `node_bounds`, `element_ids`, `action_listeners`, focus, active descendant) into `A11y::rendered` instead of clearing them. `reuse_prepaint` then replays the range's nodes onto the current tree (`A11y::reuse_range`): a node whose parent lies outside the range becomes a child of the node on top of the stack, side entries are copied, listeners moved, the focused node carried when the reused dispatch subtree holds the focus, and an active-descendant claim carried when its focused node is set again this frame (in the range, or an ancestor already pushed live), which is what a live build accepts. Deferred draws reuse through the same call, so a menu inside a cached view keeps its nodes too.

The handoff happens at the start of the next frame, not at `end_frame`: the live maps are read between frames by `a11y_element_id` and action dispatch. Reuse only runs when the previous draw built a tree (`rendered.built`); otherwise the cached view's nodes are absent that frame, as before, until the refresh that follows activation renders it. A node whose id was already pushed this frame (an element that moved between a cached and an uncached parent) is left out with its subtree and listed by `a11y_refused_elements()`, as a refused push is, and its replayed parent no longer lists it (accesskit rejects a child under two parents); the cache key demands equal bounds, so none are re-mapped. The refused list itself is not replayed: a refusal inside a cached view is listed only on frames that render the view live, though its node stays left out.

`src/view/probes.rs` pins the cache semantics as numbers (P1-P6: sibling isolation, nested notify, a mid-draw write, an unobserved read, nested caches, an uncached layer) and P7/P7b prove the reuse: with accessibility active, ten sibling notifies leave a cached card un-rendered while every frame's tree holds its nodes under the right parents, `a11y_element_id` and a Click action answer after a reused frame, and a Focus into it survives the next reuse, directly and through a `deferred(anchored())` child.

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

Consumers must patch `gpui-pre` and every package in `vendor/` from the same
fork revision. Each vendored package's `README.ducktape.md` records its
original crates.io archive SHA-256. Cargo
ignores dependency manifests' patch tables, so patches belong in each
consumer workspace's `[patch.crates-io]` table, not this dependency.

On a gpui-pre bump, refresh the scheduler and zlog sources and archive hashes,
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

### Moving a window

`Window::set_bounds` moves and sizes a window's outer frame in the
coordinates `Window::bounds` reports; `resize` keeps the origin. The backends
that implement it are vendored: `vendor/gpui-pre-linux` (X11) and
`vendor/gpui-pre-macos`, each 0.3.7 as published plus that one method (see
their `README.ducktape.md`). Wayland and Windows keep the trait default, which
only resizes. Consumers patch `gpui-pre-linux` and `gpui-pre-macos` from the
same revision as `gpui-pre`.

### Linux color emoji

`vendor/gpui-pre-wgpu` is 0.3.7 as published plus one condition in
`load_family`: a known color emoji face is not removed for lacking a Latin
`m`. See its `README.ducktape.md`. The app repo's `font_fallback` tests pin
the behavior.

### Guest wire size

The fork also carries `vendor/rmp-serde` 1.3.1 with shared parser operations and a
default-off `typed` decoding feature. It is a consumer patch, not a GPUI dependency.
The view wire enables typed decoding only on wasm; native MessagePack remains
unrestricted. See its `README.ducktape.md` for the archive checksum, exact accepted
shapes, unchanged encoding and bump checks. Consumers patch `rmp-serde` to the
same fork revision alongside the scheduler and logger. Guest builds additionally
share serialization writers and use a pinned Binaryen optimizer; GPUI itself was
not the main remaining size contributor after the JS gates removed its exports.
