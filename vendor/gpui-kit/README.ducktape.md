# gpui-kit: a `multithreaded` feature

Source: https://static.crates.io/crates/gpui-kit/gpui-kit-0.7.3.crate
Archive SHA-256: `0fac401f6b149bf33794da0942510240afd29b7b18556574e4cf17196b647066`, the
cksum the crates.io index lists. Published sources retained (longbridge/gpui-kit
67265a36, `crates/kit`); upstream tests unchanged. Consumers patch it in with the other
fork crates: `[patch.crates-io] gpui-kit = { git = fork, rev }`.

One change, in `Cargo.toml` only. `Cargo.toml.orig` is left as published.

gpui-kit 0.7.3 takes `gpui-pre-platform` and `gpui-pre-web` with their default features.
That default is `multithreaded`, which pulls `wasm_thread` into a wasm build through
`gpui-pre-scheduler/wasm-threads`. `wasm_thread` needs a nightly compiler, so an
application on gpui-kit could not build for `wasm32-unknown-unknown` on stable, and it
had no way to turn the feature off.

Now `gpui_platform` and `gpui_web` have `default-features = false`, and a kit feature
`multithreaded` turns their `multithreaded` back on. `multithreaded` is in the kit's
`default`, so every build that keeps the kit's default features gets the same packages
and the same code as before.

A wasm build that names `default-features = false, features = ["component", "assets"]`
leaves `wasm_thread` out and builds on stable. With the feature off, gpui-pre-web's
dispatcher starts no worker threads and runs background tasks on the browser's main thread.

The optional `gpui-fast-platform` and `gpui-fast-web` have the same default and are left as
published here: nothing in ducktape turns on the kit's `gpui-fast` feature, and a weak feature
on them would add a line to every consumer's `Cargo.lock`. A fix upstream should cover them
too.

Drop this vendored copy when a published gpui-kit has the feature.
