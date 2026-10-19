# gpui-pre-mobile

Source: longbridge/gpui-mobile at `c24a1308e1dcb47ce94d9ef8d8d05ca4d77d2aa7` (main on
2026-10-15), the whole tree as `git archive` gives it. Not published on crates.io.

Three things differ from upstream. First, `Cargo.toml` line 134:

```
accesskit_android = "0.7.5"   ->   accesskit_android = "0.9"
```

Why: this fork asks for accesskit 0.25 (fork README "AccessKit 0.25": a disabled control
reads as disabled over AT-SPI). `accesskit_android` 0.7.5 is built on accesskit 0.24, so
gm's `src/android/accesskit_adapter.rs` fails with five `E0277`/`E0271` errors against the
fork ("multiple different versions of crate `accesskit`"); 0.9.0 is built on accesskit
0.25.1 and the same adapter compiles unchanged (S0-android §B, 2026-10-15). Published
gpui-pre 0.3.9 asks for accesskit 0.24, so upstream gm cannot take the line until a
gpui-pre release asks for 0.25; it is offered to longbridge as a PR to land then. Drop this
vendored copy the day gm's own manifest says `"0.9"` (or later).

Second, upstream's own copy of `gpui-pre-wgpu` (`vendor/gpui-pre-wgpu`, the published crate
plus an `offscreen` feature) is not here: `[patch.crates-io]` points at the fork's
`vendor/gpui-pre-wgpu` (`path = "../gpui-pre-wgpu"`), which carries that `offscreen` set
and the fork's Linux color emoji change. A git source holds one package per name: with both
copies, a consumer patching `gpui-pre-wgpu` from the fork got `skipping duplicate package`
and cargo took this nested copy, without the emoji change (desktop Linux emoji drew as tofu).
When dropping this vendored tree, drop nothing from the fork's `gpui-pre-wgpu`.

Third, `android-activity` is optional, behind a `native-activity` feature that is on by
default (`Cargo.toml` and `src/android/{jni,host,platform,accessibility}.rs`). Its glue exports
`ANativeActivity_onCreate` and calls the application's `android_main`, so any library linking
it must define that symbol or fail to load (`dlopen`: `cannot locate symbol "android_main"`),
even when no `NativeActivity` is declared and the app's own Activity drives GPUI through
`android::host`. With the feature off, `jni.rs` keeps the JNI helpers `host` uses
(`java_vm`, `activity_as_ptr`, the night mode and the asset manager read the Activity passed
to `host::set_activity`) and leaves out `init_platform`, `run_event_loop`, `poll_events` and
`android_app`; `path_provider`, whose Android half reads the app's directories from
`android-activity`, turns it on. Default builds are unchanged. It needs nothing else from
this fork and is offered to longbridge as is; drop it when upstream has it.

`diff -rq <git archive c24a130> vendor/gpui-pre-mobile` lists `Cargo.toml`, the four
`src/android` files above, this file, and `Only in <archive>: vendor`.

Consumers depend on it with `default-features = false` (so without `native-activity`) and build Android with
`cargo ndk --platform 26` (below 26 the link fails on `-lnativewindow`). Its iOS half is not
used: iOS is `vendor/gpui-pre-ios`.

Check (Linux, NDK): a consumer crate depending on this path with the fork's crates
patched in, `cargo ndk -t arm64-v8a -t x86_64 --platform 26 check`.
