# gpui-pre-ios

Source: zed-industries/zed `crates/gpui_ios` at `a47f721832e5b6ecf977891c1b7c6aac1406723e`
(zed #63068 "gpui_ios: Add iOS platform backend", merged 2026-10-14). No `gpui-pre-ios`
crate is published; this directory is that crate renamed onto the gpui-pre 0.3.9 names,
the way `gpui-pre-macos` is to `gpui_macos`. Replace it with the published crate if one
appears.

`diff -rq <zed>/crates/gpui_ios vendor/gpui-pre-ios` lists only:

- `Cargo.toml`: a standalone manifest (the fork is not a workspace). `gpui` is
  `gpui-pre =0.3.9`, `gpui_apple` is `gpui-pre-apple =0.3.9` (published, file-for-file
  zed's `gpui_apple` at the snapshot rev); the other dependency lines are zed's workspace
  lines at `a47f721832` inlined, and zed's `[workspace.lints]` inlined as `[lints]`.
  `crate-type = ["staticlib", "rlib"]`, every dependency under `cfg(target_os = "ios")`,
  as upstream.
- `examples/ios/Cargo.toml`: a standalone package patching `gpui-pre` and
  `gpui-pre-scheduler` to this fork by path. `gpui-pre-platform` 0.3.9 has no iOS arm, so
  `src/gpui_ios_example.rs` runs `gpui::Application::with_platform(gpui_ios::current_platform(false))`
  instead of `gpui_platform::application()` (one line); `build-simulator.sh` builds from
  this directory (three lines: `repository_root` from the script's own path, not `git`;
  `example_directory`; `cargo build` without `-p`).
- `src/ios/display.rs`: zed's `unsafe impl Send for IosDisplay {}` / `unsafe impl Sync for IosDisplay {}`
  are deleted. `IosDisplay` wraps a `Retained<UIScreen>`, a `MainThreadOnly` class, and nothing needs
  the bounds (`PlatformDisplay: Debug` only; every use is `Rc<dyn PlatformDisplay>` on the main
  thread), so the two lines only let safe code move a `UIScreen` off the main thread. Two lines
  fewer, no other change.
- this file.

The rest of `src/` and `tests/` is verbatim.

## `unsafe` in this crate: an explicit exemption from the fork's SAFETY-comment rule

The crate has 29 `unsafe {}` blocks and (after the deletion above) no `unsafe impl`, none with a
`// SAFETY:` comment, and zed's `[lints]` neither deny `unsafe_code` nor ask for
`clippy::undocumented_unsafe_blocks`. They are not annotated here on purpose: the code is verbatim
zed `a47f721832` and is refreshed from zed on every bump, so a comment on each block would be a
source delta to carry forever. Instead the blocks were reviewed one by one on 2026-10-16
(`wt/briefs/wasm-mobile/lanes/reviews/fork-29-31-opus.md`, section "#30 ... B. unsafe / FFI"), all
sound as used: the `alloc().set_ivars()` + `msg_send![super(..), init|initWithFrame:]` pattern; the
`resignFirstResponder` super call; `performSelector_withObject_afterDelay(sel!(reconcileTextInput), None, 0.0)`
(a defined, argument-less selector; a late fire finds no window and does nothing); `borrow_raw` on a
view `self` retains, tied to `&self`; observers removed in both `Drop`s;
`rangeOfComposedCharacterSequenceAtIndex` bounds-checked before the call; `SFSafariViewController`
only for `http(s)://`; every class `MainThreadOnly`, so an off-main entry panics at
`MainThreadMarker::new().expect(..)` instead of causing UB. A refresh that changes any `unsafe`
site re-does that review before the bump commit.

**The phone profile must build with `panic = "abort"`.** objc2 0.6's `define_class!` methods are
`extern "C-unwind"`, and no ObjC entry point here (`touchesBegan:`, `insertText:`, `renderFrame:`,
...) has an unwind guard, so a Rust panic reached from one would unwind into UIKit frames. With
`panic = "abort"` the app dies with its logged reason instead (the SPEC's §13.3 rule for a phone).

Known, left as zed has them (same review): `window.rs` keyboard observers use `queue: None` and rely
on UIKit posting the notifications on main (they do); `text_input.rs` keeps a non-atomic `Rc<()>`
session token in ObjC ivars whose `dealloc` runs on whichever thread releases last; the manifest
loosens zed's `=0.3.2` pins on `objc2-{core-foundation,foundation,quartz-core}` to `0.3.2`;
`disallowed_methods = "deny"` has no list (zed's `clippy.toml` did not come along); zed's
`crates/gpui/examples/input.rs` update from #63068 (its reference text field for this contract) is
not in the fork's `examples/`.

## What it needs from gpui core (the #63068 backport, in the fork's `src/`)

The crate compiles only against a gpui core that carries #63068's core half (the fork
commit "ios: the #63068 core half"): `App::{on_app_lifecycle, on_memory_warning}`
observers and the platform hooks wired to them, `AppLifecyclePhase::Disconnected`,
`Window::insets()`, `Window::{notify_text_input_changed, notify_text_selection_changed}`,
`PlatformInputHandler::{focus_id, query_accepts_focused_text_input,
selection_bounds_for_range}`, `InputHandler::selection_bounds_for_range` and the
`EntityInputHandler` forward, a public `Window::last_input_was_touch`, and the
`focused_text_input: Option<FocusId>` bookkeeping in `Window`. Zed's tests for those
came with it (`app/test_context.rs`, `elements/div.rs`, `input.rs`, `key_dispatch.rs`,
`window.rs`).

## Build (Mac only; 1.96.1)

`cargo check --target aarch64-apple-ios` needs Apple's toolchain, so it runs on a Mac.
From this directory:

```
cargo update -p libc --precise 0.2.189
IPHONEOS_DEPLOYMENT_TARGET=15.0 cargo build --target aarch64-apple-ios-sim \
  --config 'patch.crates-io.gpui-pre.path="../.."' \
  --config 'patch.crates-io.gpui-pre-scheduler.path="../gpui-pre-scheduler"'
```

The libc line: `backtrace` 0.3.76 (what gpui-pre 0.3.9 pulls) calls `_dyld_image_count`
and three siblings on every Apple target, and libc 0.2.190 (2026-10-02) restricts those
to macOS, so an iOS build with libc 0.2.190 in the lock fails inside `backtrace`. The app's
lock holds 0.2.189, which still declares them. A consumer with 0.2.190 in its lock cannot
build this crate for iOS until `backtrace` follows libc (or `backtrace` leaves the graph).

The example app: `examples/ios/build-simulator.sh` (as zed's), then
`xcrun simctl install <udid> <app>` and `xcrun simctl launch <udid> dev.zed.gpui-ios-example`.

## Bump

Refresh from zed's `crates/gpui_ios` at the new snapshot's zed rev, re-apply the three
file edits above, and re-run the two builds. If the new gpui-pre release itself carries
#63068's core half, drop the fork's core commit and keep only this vendor.
