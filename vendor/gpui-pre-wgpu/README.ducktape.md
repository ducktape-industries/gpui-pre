# Linux color emoji

Source: https://static.crates.io/crates/gpui-pre-wgpu/gpui-pre-wgpu-0.3.9.crate
Archive SHA-256: `5b1cdb26c1503bf9662ee8fbc5ab0b44b2deeee4b5c12dc67491ba6fb5fc544b`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

Two changes. The first, in `CosmicTextSystemState::load_family`: a known color emoji font
(`check_is_known_emoji_font`) is exempt from the check that skips a face with
no Latin `m`. Noto Color Emoji has no `m` by design. Through 0.3.8 the check
also removed such a face from the font database, so emoji rendered as tofu.
0.3.9 (zed d6358b2d, #65269) keeps the face in the database, where
cosmic-text's own fallback can find it, but still does not load it:
`load_family` answers no font for the family, so `font_id` for it fails and a
caller's fallback chain (`FontFallbacks`) drops the entry. The exemption keeps
a color emoji family loadable by name and in the chain a caller declares.

The second is longbridge/gpui-mobile's (`c24a130`, its own `vendor/gpui-pre-wgpu`,
which `vendor/gpui-pre-mobile` used to carry): an `offscreen` feature (`dep:image`,
off by default) that builds `WgpuHeadlessRenderer` outside `test-support` /
`bench-support`, with `with_instance` (a wgpu instance of the caller's backend; iOS
needs Metal), `render_to_image`, `sprite_atlas`, `max_texture_size`,
`release_render_target` and `clear_sprite_atlas`, and one Linux test of the last two.
Its files (`Cargo.toml`, `gpui_wgpu.rs`, `wgpu_context.rs`, `wgpu_renderer.rs`) are
taken whole from that copy. The fork holds one `gpui-pre-wgpu`: a second package of
the name in the same git source made cargo pick one of the two for consumers.
Drop this vendored copy when the locked upstream release loads a color emoji
family by name and has an equivalent of `offscreen` (or `gpui-pre-mobile` no
longer asks for it).
