# Linux color emoji

Source: https://static.crates.io/crates/gpui-pre-wgpu/gpui-pre-wgpu-0.3.8.crate
Archive SHA-256: `439c5421cafe1a7cf68d0c0ae5cc82b26fad6b0e6345ac98d20acaf1f2463914`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One change, in `CosmicTextSystemState::load_family`: a known color emoji font
(`check_is_known_emoji_font`) is exempt from the check that removes a face with
no Latin `m`. Noto Color Emoji has no `m` by design, so the check dropped its
registered face before the fallback chain was built and emoji rendered as tofu.
Drop this vendored copy when the locked upstream release carries the exemption.
