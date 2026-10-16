# Linux color emoji

Source: https://static.crates.io/crates/gpui-pre-wgpu/gpui-pre-wgpu-0.3.9.crate
Archive SHA-256: `5b1cdb26c1503bf9662ee8fbc5ab0b44b2deeee4b5c12dc67491ba6fb5fc544b`. Published sources and the Apache-2.0 license retained; upstream tests unchanged.

One change, in `CosmicTextSystemState::load_family`: a known color emoji font
(`check_is_known_emoji_font`) is exempt from the check that skips a face with
no Latin `m`. Noto Color Emoji has no `m` by design. Through 0.3.8 the check
also removed such a face from the font database, so emoji rendered as tofu.
0.3.9 (zed d6358b2d, #65269) keeps the face in the database, where
cosmic-text's own fallback can find it, but still does not load it:
`load_family` answers no font for the family, so `font_id` for it fails and a
caller's fallback chain (`FontFallbacks`) drops the entry. The exemption keeps
a color emoji family loadable by name and in the chain a caller declares.
Drop this vendored copy when the locked upstream release loads a color emoji
family by name.

# Unhinted glyphs

`render_glyph_image` builds its swash scaler with `.hint(false)`, where 0.3.9 has
`.hint(true)`. swash hints in skrifa's `Smooth` mode: heights and horizontal strokes snap to
whole pixels, vertical strokes do not, so text drew about a pixel shorter with hard top and
bottom edges (squashed bold). Layout is unhinted upstream already, so only glyph images change.
