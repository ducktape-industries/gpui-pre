# Linux color emoji

Source: https://static.crates.io/crates/gpui-pre-wgpu/gpui-pre-wgpu-0.3.7.crate
Archive SHA-256: `f0b02657b56b09140ce542f5e4434fb96d9fdaba0fd0035ba7dba13c171ddb9b`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One change, in `CosmicTextSystemState::load_family`: a known color emoji font
(`check_is_known_emoji_font`) is exempt from the check that removes a face with
no Latin `m`. Noto Color Emoji has no `m` by design, so the check dropped its
registered face before the fallback chain was built and emoji rendered as tofu.
Drop this vendored copy when the locked upstream release carries the exemption.
