# Linux color emoji

Source: https://static.crates.io/crates/gpui-pre-wgpu/gpui-pre-wgpu-0.3.5.crate
Archive SHA-256: `640b666a16ddf9504e2eb3e7a997acb2b2adfaa7859925bdd36fc3acfd8f30a7`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One change, in `CosmicTextSystemState::load_family`: a known color emoji font
(`check_is_known_emoji_font`) is exempt from the check that removes a face with
no Latin `m`. Noto Color Emoji has no `m` by design, so the check dropped its
registered face before the fallback chain was built and emoji rendered as tofu.
Drop this vendored copy when the locked upstream release carries the exemption.
