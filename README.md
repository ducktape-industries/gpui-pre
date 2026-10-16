# gpui-pre (ducktape-industries fork)

**What:** [`gpui-pre` 0.3.9](https://crates.io/crates/gpui-pre/0.3.9) exactly as published on crates.io (the base commit), plus our patches. The first four are accessibility:

1. `test-support: the test window keeps the last accessibility tree update` — `TestWindow` retains the last AccessKit `TreeUpdate` it is sent; `TestWindow::last_a11y_tree_update` and `Window::last_a11y_tree_update` (test-support only) read it.
2. `window: a public switch activates accessibility for one window` — `Window::activate_a11y()` builds that window's tree with no assistive technology attached.
3. `div: an aria_disabled setter on the element accessibility API` — `aria_disabled(bool)` beside `aria_selected`/`aria_expanded`/`aria_toggled`.
4. `window: read the accessibility tree, its element ids, and dispatch an action at runtime` — `Window::a11y_tree()` (the last `TreeUpdate`, in every build), `Window::a11y_element_id(node)` (the `GlobalElementId` that built a node), `Window::dispatch_a11y_action(request)` (the adapter's own action path). Read-only accessors plus one public entry to the existing handler; nothing changes unless called.

The later ones each have a section below: link wrapping, letter spacing on the shaped line, `Window::set_bounds`, `Window::set_resizable`, bounds observers on full-screen/maximize, one tab stop per focus handle, refused accessibility nodes, the active-descendant claim gate, accessibility nodes through cache reuse, focus moves that redraw two views, SVG hrefs that read no file and inflate within a bound, SVG rasters a caller drops from the atlas, guest layers (masked deferred draws take no click outside their mask, hit order is paint order, tooltips take a layer), a held key's repeat begins no keyboard click, throttled animations on one frame grid, a deferred draw's accessibility nodes under the node it was drawn in, a node id an element id cannot solve, a linear gradient's payload read back, taffy 0.14.0, AccessKit 0.25 (a disabled control reads as disabled over AT-SPI), unhinted glyphs on Linux, Linux clipboard reads that turn the app's loop while they wait, the first key of Apple's Korean input method in a field with no selection (macOS), the iOS platform (zed #63068's core half and `vendor/gpui-pre-ios`), the JavaScript-free wasm guest gates, and the vendored siblings (`vendor/`) those need.

**Why:** ducktape-app #114 gates merges on an accessibility contract (`ax_contract`) that reads every screen's AccessKit tree headless. Without (1) and (2) no headless test can see the tree: the test window dropped it and a window only built it once an adapter activated. (4) is for the app's opt-in loopback test door, which serves that same tree to a QA runner in a real (release) build and acts through the same path a screen reader does.

**Not upstreamed, by owner decision (2026-09-19).** No PR to zed-industries/zed and no gpui-kit issue; this private fork stays. The app consumes it through `[patch.crates-io]`, pinned by full rev.

### Re-applying on a gpui-pre bump

1. Start a branch from `main` whose first commit is the new `gpui-pre` release exactly as published on crates.io (one commit: the tree is the unpacked `.crate`, nothing else; `diff -rq` against the unpacked archive prints nothing).
2. `git cherry-pick -x` every patch commit of the previous bump in `git log` order (the accessibility ones: the four above, the tab-stop fix, refused nodes, the claim gate, nodes through cache reuse with `src/view/probes.rs`, focus moves that redraw two views, a div's Click answered by its click listeners; and `Window::drop_svg` with its key helpers; a held key's repeat begins no keyboard click, whose X11 half is in `vendor/gpui-pre-linux`; and `App::until_next_animation_frame`, the throttled animations' frame grid; and a deferred draw's accessibility parent; and the node id that is a SipHash of the id path, unless upstream derives node ids from something an element id cannot solve: then that commit is dropped, see its section; and `Background::as_linear_gradient`; and taffy 0.14.0, unless the new release itself depends on taffy 0.14 or later: then that commit is dropped, see its section; and AccessKit 0.25, unless the new release itself asks for accesskit 0.25 or later: then that commit is dropped, see its section; and the #63068 core half, unless the new release itself carries `Window::insets` and `App::on_app_lifecycle`: then that commit is dropped, see "iOS"; and letter spacing, `TextStyle::letter_spacing`). A commit that carries a `vendor/` crate (the wasm-guest gates: scheduler; `set_bounds` and `set_resizable`: linux and macos; the wgpu emoji fix and unhinted glyphs; the XIM client's decoding, `zed-xim`, when the new `gpui-pre-linux` asks for another `zed-xim`; the clipboard wait, X11 and Wayland: linux; the Korean first key: macos; the iOS platform: `vendor/gpui-pre-ios`, refreshed from zed's `crates/gpui_ios` at the snapshot's zed rev rather than from a published archive, or replaced by a published `gpui-pre-ios` crate if one appears) is a refresh, not a plain pick: `cherry-pick -n`, replace the vendor directory with the new published archive, reapply our delta (diff of the old vendor against the old published archive), record the new archive SHA-256 in its `README.ducktape.md`, then commit with the original message plus a note. `diff -rq <published> vendor/<crate>` must list only `README.ducktape.md` and the patched files. On 0.3.7 to 0.3.8 two picks conflicted: `src/text_system/line_wrapper.rs` (upstream's `IndentAdjustment` beside our `Link`, and a third `wrap_line` parameter) and `vendor/gpui-pre-linux/src/linux/x11/client.rs` (upstream moved the client state's construction into `X11Connection::attach`). On 0.3.8 to 0.3.9 five did. (1) The wasm-guest gates: upstream rewrote its dependency feature lists (zed d544470d), so the root and scheduler manifests are the published 0.3.9 lines with the gates re-derived on them (see "wasm guest without JS"), and `vendor/gpui-pre-zlog` is gone. (2) The wgpu emoji condition: upstream stopped removing a face with no Latin `m` from the font database but still skips it (zed d6358b2d); the condition is kept beside upstream's comment. (3) A deferred draw's accessibility parent: `DeferredDraw::a11y_parent` beside `element_id_stack`, now an `ElementIdStack`. (4) taffy: upstream turned taffy's default features off and names seven; only the version line moves. (5) AccessKit: `accesskit_unix` is optional in `gpui-pre-linux` 0.3.9; the vendor line keeps `optional = true`. Two picks applied but did not compile: 0.3.9 made `GlobalElementId` a struct of the ids and their hash (zed 1dd9bbfd), so a test that built one from a tuple now calls `GlobalElementId::new(&[..])` and a probe that read `.0.last()` reads `.last()`.
3. Run this crate's own tests with the vendored siblings patched in and `test-support` on: `cargo test --lib --features test-support --config 'patch.crates-io.gpui-pre-scheduler.path="vendor/gpui-pre-scheduler"'`, and the a11y/tab_stop tests again with `--release` (`a_second_element_with_one_id_is_refused` only records in release). Two tests `include_bytes!` zed's `assets/fonts/{ibm-plex-sans,lilex}` three directories above `src/`; fetch them from zed at the snapshot rev. From 0.3.8 the crate's own `randomized_element_tree` test uses `#[gpui::property_test(config = ..)]`, which crates.io `proptest-macro` 0.5.0 cannot expand (its codegen panics), so the lib tests build only against the proptest zed pins: add `--config 'patch.crates-io.proptest.git="https://github.com/proptest-rs/proptest"' --config 'patch.crates-io.proptest.rev="3dca198a8fef1b32e3a66f1e1897c955b4dc5b5b"'` and `cargo update -p proptest` with the same flags (the published lock holds a crates.io proptest, 1.12.0 in 0.3.9; the pin is 1.10.0). From 0.3.9 the tests in `src/style.rs` use `util_macros::perf`, which zed made a dev-dependency (d544470d); the published manifest has no dev-dependencies, and `gpui-pre-util-macros` and `gpui-pre-perf` were last published at 0.3.8, pinned to `gpui-pre-collections =0.3.8`. So the lib tests also need a `[dev-dependencies.util_macros]` entry (`package = "gpui-pre-util-macros"`) pointing at an unpacked copy of those two 0.3.8 archives with that one pin moved to the new version. Both are test-time overrides; the manifest and lock stay as published. Then `cargo tree --target wasm32-unknown-unknown --no-default-features -e normal` must show no `wasm-bindgen`, `js-sys`, `web-sys`, `web-time` or `getrandom`. Root `Cargo.lock` is the published one and is not committed with resolver changes.
4. In ducktape-app and modules, set the new full rev in every `[patch.crates-io]` entry (all fork crates at one rev), then run `cargo test ax_contract` and the door tests: they fail if any of the accessibility commits is missing.

### Link wrapping

`LineWrapper::wrap_line` and `LineLayout::compute_wrap_boundaries` (what laid-out text such as `StyledText` wraps with) break links sensibly: never inside or right after `scheme://`, not in the authority, and after `/`, `-` or `.` in the path; in prose a `/` breaks after itself rather than before (`and/` | `or`). Upstream broke before every `/`, so `duck://a-b/c` wrapped as `duck:/` | `/a-b/c`. `test_wrap_link` and `test_wrap_boundaries_keep_links_whole_until_their_path` pin it.

### Letter spacing

`TextStyle::letter_spacing: Option<Pixels>` adds space after each letter, as CSS `letter-spacing` and React Native's `letterSpacing` do. It refines and cascades like the other text fields, and `Styled::letter_spacing(px)` sets it. `TextLayout` (what `StyledText`, `InteractiveText` and a string child lay out with) shapes with the element's spacing through `WindowTextSystem::shape_letter_spaced_text` (crate-private; `shape_text` keeps its signature and shapes with none, so no caller of it changes, the vendored wgpu tests included). `LineLayoutCache` keys the line by it and applies it to the shaped line, beside `force_width`. The spacing goes after each glyph the pen advances past (its next glyph lies right of it; the last glyph when it is the rightmost and the line ends right of it), so a left-to-right line of n glyphs is n x spacing wider. Every glyph moves by the spacings before it and the width grows by exactly those. A glyph the pen does not advance past (a combining mark, a zero-width character) keeps its base's place. In a left-to-right run both shapers put a mark at its base's end: cosmic-text with the base's text index, CoreText with its own, later one (measured: `q\u{301}x` and `ab\u{301}\u{301}c`, cosmic-text 0.19 with DejaVu Sans, CoreText with Helvetica). In a right-to-left run inside a left-to-right line, both put a point before its base, at the base's x (a pointed shin, Hebrew fallback fonts). So a glyph the pen does not pass takes the shift of the glyph before it when the next glyph the pen passes comes later in the text (it follows its base), and keeps the running shift otherwise (it precedes its base, and no spacing lies between them). Wrapping (`compute_wrap_boundaries`), hit testing (`index_for_x`, `x_for_index`, `closest_index_for_x`) and painting read those positions, so they agree with no change of their own.

It is one place for every platform. Each shaper (CoreText on macOS and iOS, cosmic-text on Linux and Android, DirectWrite on Windows, the web's) returns its line to that cache through `PlatformTextSystem::layout_line`. `FontRun`, which the shapers build and read, and `TextRun` are unchanged, so no platform crate, no `TextRun { .. }` literal and no public signature downstream changes. With no spacing nothing is applied and the layout is the shaper's, as before. `TextStyleRefinement`'s serde form omits the field when it is unset.

Why: ducktape's phone views port the Expo app's type roles exactly, and three of them track: `eyebrow` 11/14 +0.66, `bigTitle` 22/27 -0.22, `display` 36/44 +1.44 (the mobile repo's `src/design/theme.ts`). gpui's text style had no letter spacing.

Not covered: `shape_line`, `layout_line` and the by-hash shapers take no spacing (gpui-base's input and editor shape through them). Full right-to-left tracking: cosmic-text gives the glyphs of a paragraph that starts right to left in right-to-left order (x falling), so no pen move passes them and such a line keeps the shaper's positions and width. CoreText gives every line left to right and spaces it. A ligature is one glyph and gets one spacing, so `ffi` stays tight (CSS turns optional ligatures off under letter spacing; this keeps them). `LineWrapper`'s truncation estimate (`text_overflow`) sums unspaced character advances, so a spaced line too long for its box is cut up to characters x spacing off (the `ponytail:` note in `src/elements/text.rs`). The fork does not bound the value; a host that takes it from an untrusted guest bounds it itself (in ducktape, view-wire's style sanitizer).

`letter_spacing_widens_a_line_by_spacing_times_its_glyphs`, `letter_spacing_keeps_index_for_x_and_x_for_index_inverse`, `letter_spacing_wraps_at_the_spaced_width`, `no_letter_spacing_lays_out_as_the_shaper_did`, `letter_spacing_keeps_a_combining_mark_with_its_base` (on the glyph streams the two shapers gave) and `letter_spacing_leaves_glyphs_given_right_to_left_as_shaped` (`src/text_system/line_layout.rs`), and `a_letter_spaced_element_shapes_its_text_spaced` (`src/elements/text.rs`) pin it.

When it goes: when upstream gpui's text style carries letter spacing as an `Option<Pixels>` applied after shaping. Then this commit is dropped at the bump. With another type or applied elsewhere, keep this patch or port ducktape to upstream's form deliberately: programs' view-wire takes `TextStyleRefinement` apart field by field, so a different field breaks it. Until then it is cherry-picked. Where it conflicts, the patch is:
- `src/text_system/line_layout.rs`: `apply_letter_spacing`; the `letter_spacing` argument of `LineLayoutCache::layout_line` and `layout_wrapped_line` (between `font_size` and `runs`); and the `letter_spacing` field of `CacheKey` and `CacheKeyRef`.
- `src/text_system.rs`: `shape_text`'s body moved into `shape_letter_spaced_text`, with the one new argument.
- `src/elements/text.rs`: `TextLayout::layout`'s two shaping calls (the truncation fit check and the final shape) pass `text_style.letter_spacing`.

### One tab stop per focus handle

`TabStopMap::insert` keeps one entry per focus handle per frame: the first, which is the outermost element's (a div inserts its own handle before its children paint). Upstream kept every insert in the order and the last in its lookup, so a field whose labelled wrapper tracks the input's own handle held two stops, and `focus_prev` from the field landed on its other entry: Shift+Tab never left it. The skipped insert is still recorded, so a cached element's replayed range keeps the stop when nothing outside it tracks the handle. `test_one_tab_stop_per_focus_handle` pins it.

### Refused accessibility nodes

`Window::a11y_refused_elements()` lists the elements whose accessibility node the last frame with accessibility active left out because an earlier node had the same id, each with that shared `NodeId`, so `a11y_element_id(id)` names the element that kept it. In a debug build a duplicate pushed live panics first, so only duplicates skipped while replaying a cached view are listed there; a release build dropped such a node without a trace, and this lets the app's test door report it. `a_second_element_with_one_id_is_refused` pins it (run it with `--release`).

### Active-descendant claim gate

An `aria_active_descendant` claim counts only when the claimant's nearest focusable ancestor (the first node below it on the accessibility stack registered with `set_focusable`) is the focused node. Upstream honored a claim under a focused ancestor at any depth, so a focused outer box (a held pane, a focusable frame) let every composite inside it claim: two claims panicked in debug ("active descendant claimed by multiple nodes") and the last won in release, telling assistive technology that a row of an unfocused composite had the focus. A focusable element with no role pushes no node and is passed over; it can never be the accessibility tree's focused node either. `active_descendant_ignored_under_unfocused_focusable_ancestor`, `focused_composite_claim_wins_over_unfocused_one` and `focused_box_over_two_unfocused_composites_stays_focused` pin it.

### Accessibility nodes survive cache reuse

A cached view (`.cached()`) whose prepaint is reused keeps its accessibility nodes. Upstream `reuse_prepaint` replayed hitboxes, tooltips, element states, layouts, the dispatch subtree and deferred draws but built no nodes, so under a screen reader a cached view vanished from the tree (and its action listeners, focus and bounds with it) on every frame that hit the cache; the only remedy was to switch caching off while accessibility was active. Now `A11yNodeBuilder` logs `(parent, id, node)` beside every node it finishes, `PrepaintStateIndex` records the log position, and `A11y::begin_frame` moves the previous frame's log and side maps (`focus_ids`, `node_bounds`, `element_ids`, `action_listeners`, focus, the active-descendant claims with their gates) into `A11y::rendered` instead of clearing them. `reuse_prepaint` then replays the range's nodes onto the current tree (`A11y::reuse_range`): a node whose parent lies outside the range becomes a child of the node on top of the stack, side entries are copied, listeners moved, the focused node carried when the reused dispatch subtree holds the focus, and each replayed claim judged again by this frame's focus (see Focus moves redraw two views). Deferred draws reuse through the same call, so a menu inside a cached view keeps its nodes too.

The handoff happens at the start of the next frame, not at `end_frame`: the live maps are read between frames by `a11y_element_id` and action dispatch. Reuse only runs when the previous draw built a tree (`rendered.built`); otherwise the cached view's nodes are absent that frame, as before, until the refresh that follows activation renders it. A node whose id was already pushed this frame (an element that moved between a cached and an uncached parent) is left out with its subtree and listed by `a11y_refused_elements()`, as a refused push is, and its replayed parent no longer lists it (accesskit rejects a child under two parents); the cache key demands equal bounds, so none are re-mapped. The refused list itself is not replayed: a refusal inside a cached view is listed only on frames that render the view live, though its node stays left out.

`src/view/probes.rs` pins the cache semantics as numbers (P1-P6: sibling isolation, nested notify, a mid-draw write, an unobserved read, nested caches, an uncached layer) and P7/P7b prove the reuse: with accessibility active, ten sibling notifies leave a cached card un-rendered while every frame's tree holds its nodes under the right parents, `a11y_element_id` and a Click action answer after a reused frame, and a Focus into it survives the next reuse, directly and through a `deferred(anchored())` child.

### Focus moves redraw two views

`Window::focus` and `Window::blur` called `refresh()`, which set `refreshing` for the whole next draw, so every cached view in the window missed on a Tab, an arrow in a roving group or an Escape, guest trees included. Now a focus move marks dirty the views that rendered the old and the new focus ids (`DispatchTree::view_of_focusable`: the focusable's nearest ancestor node with a `view_id`, in the rendered frame, replayed ranges included) and their ancestors, then asks for a frame; `focus_generation` and the pending-keystroke clear are as before, and a move made by a focus listener during a draw is invalidated the same way once the draw ends. Focus-visible styling and `key_context` changes live in those two views' subtrees, so they redraw. `in_focus` styling does not: `within_focused` is true under a focused *ancestor*, and a cached view under it replays the prepaint that read it. So a div that styles itself by `in_focus` marks its dispatch node (`DispatchNode::in_focus_reader`, carried on reuse), and the move also dirties the views that rendered a reader under either id (`DispatchTree::views_reading_focus_within`); a cached view with no reader keeps its cache. A view that reads a focus handle it did not render (`is_focused`, `contains_focused`) must observe focus (`on_focus`/`on_blur` listeners, or its own notify) or it keeps its stale render, as any unobserved read does (P4). `p8_a_focus_move_re_renders_the_two_views_that_own_it` pins the numbers: four cached siblings, `focus(B)` then `focus(C)` render `[root, A, B, C, D]` `[1,1,1,1,1] → [2,1,2,1,1] → [3,1,3,2,1]`, D reading C's handle without observing stays stale. `p8b_blur_and_a_listener_move_re_render_two_views` pins the other two paths: a focus listener on B forwarding to C renders `[+2, 0, +2, +1, 0]` once the draw-end move is drawn, and `blur` then adds `[+1, 0, 0, +1, 0]` (totals `[+3, 0, +2, +2, 0]`). `p9_a_focus_move_re_renders_the_in_focus_readers_under_it` pins the reader rule: the root's own focusable over a cached stop and a cached `in_focus` reader, `[root, stop, reader]` `[1,1,1] → focus(stop) [2,2,1] → focus(own) [3,3,2] → focus(stop) [4,4,3]`, the reader's render seeing `within_focused` `false, true, false` (the first move reuses the reader, so its flag is found after a replay); P8 and P8b are unchanged by it.

An active-descendant claim inside a cached view depends on a focus outside it (its gate, the claimant's nearest focusable ancestor), so a replayed range is judged again by the current frame's focus: `A11y` records every claim with its gate, honored or not, and `reuse_range` honors a replayed claim when its gate is the focused node this frame, whether the gate gained or lost the focus since the range was built (`a_replayed_claim_is_judged_by_this_frames_focus`, and P7c's fresh frame, which the two-view invalidation would otherwise leave without its claim).

### A div's Click from assistive technology is its click

A div with click listeners answers AccessKit `Click` by running them, with a `ClickEvent::Keyboard` (Enter, the element's bounds) as a focused element's Enter does, and no pointer event is synthesized. Upstream advertised `Click` for `.on_click()` (as `_accessibility.rs` promises it answers) but registered nothing, so the request fell to `Window::handle_a11y_action`'s built-in arm: a `MouseDown`/`MouseUp` at the centre of the node's unclipped bounds, hit-tested like a pointer, which missed a row scrolled or clipped out of its list and pressed whatever covered the node. An element that registers its own `on_a11y_action(Click)` keeps it, and only it runs; the built-in press stays for a node with no answer. `a11y_click_runs_the_click_listener_of_a_clipped_or_covered_element` pins it; P7's card answers through that built-in press (an `on_mouse_up`, no click listener) at its replayed bounds.

### SVG hrefs read no file and inflate within a bound

`SvgRenderer` parses untrusted SVG (a ducktape view's pictures). usvg's default `image_href_resolver` read any file an `<image>` or `<feImage>` href names (`/dev/zero` grew until OOM, a FIFO blocked the window thread, a local picture showed in the view) and parsed a nested SVG data URL, gzip included, inflating it without a bound. Now an href that is not a data URL is never read, a data URL resolves to PNG, JPEG, GIF or WebP only (by its mime, or by magic for `text/plain` as usvg's default does) and a nested SVG is refused. A raster whose header declares more than 8192 px a side (`MAX_RASTER_SIDE`, the rasterizer's own cap) or more than 16,777,216 pixels (`MAX_RASTER_PIXELS`, 64 MiB of RGBA) is refused before resvg decodes it; only headers are read. The caps apply to the size each decode actually uses: every `VP8 ` and `VP8L` frame of a WebP (top level and inside each `ANMF`, every chunk to the end of the bytes, since image-webp reads past the declared RIFF size), whatever its `VP8X` canvas declares, since image-webp decodes a frame whole before comparing it with the canvas; and a GIF's first frame (read with the `gif` crate, now a direct dependency at the version image and resvg use), whatever its logical screen declares. A WebP whose chunks do not read is refused. A top-level svgz inflates to at most 16 MiB (`MAX_SVGZ_INFLATED_BYTES`, refused as `usvg::Error::MalformedGZip`), so `flate2` is a direct dependency (the version usvg already uses). `svg_image_href_never_reads_a_local_file`, `svg_nested_svg_data_href_is_refused`, `svg_raster_data_href_still_renders`, `svg_raster_data_href_past_the_size_caps_is_refused`, `svg_webp_frame_past_the_caps_is_refused_whatever_the_canvas`, `svg_gif_first_frame_past_the_caps_is_refused_whatever_the_screen` and `svgz_inflating_past_the_cap_is_refused` pin it.

### SVG rasters a caller drops from the atlas

`Window::drop_svg(params)` takes one SVG raster's tile out of the window's sprite atlas, as `drop_image` does for an image's frames; the next paint of that SVG rasterises it again. Upstream kept every SVG tile for the window's life (`sprite_atlas` is private and `drop_image` covers image keys only), so a window resized through many sizes or scrolled through many icons only grew its atlas, and a caller budgeting SVG rasters could free nothing. The key has one source: `Window::svg_params(bounds, path)` is the `RenderSvgParams` `paint_svg` rasterises under (the snapped bounds' size at `SMOOTH_SVG_SCALE_FACTOR`), and `Svg::data_path(bytes)` is the `__binary_svg__{hash}` path `svg().data(..)` paints under; `paint_svg` and `Svg::data` call them, so a caller never re-derives gpui's rounding or naming. `Window::has_svg_atlas_entry` (test-support) sits beside `has_image_atlas_entry`. Nothing changes unless `drop_svg` is called. `a_dropped_svg_leaves_the_atlas_and_the_next_paint_rasterises_it` pins it for a path SVG and a data SVG.

### Guest layers: masked deferred hits, hit order by paint order, tooltip layers

A host draws an untrusted view's overlays as deferred draws masked to the view's slot, and its own dialogs as deferred draws of higher priority; three changes make the mask and the priority hold for clicks as they do for paint. (1) A deferred draw given a content mask (`defer_draw(.., Some(mask))`) is prepainted under it, as `defer_draw`'s doc promises, so its hitboxes, `content_mask()` readers and nested draws see the mask and an area the mask hides takes no click; upstream pushed the mask at paint only. (2) `Frame::hit_test` walks the deferred draws' hitboxes in paint order (`Frame::deferred_draw_traversal_order`, which painting uses too); upstream walked them in prepaint order, so a draw nested in another (a later round) took the click above a higher-priority draw painted over it. Frames whose deferred draws are all in one round hit as before. (3) A tooltip is a deferred draw. `Window::with_tooltip_layer(Some(TooltipLayer { priority, mask }), ..)` puts every tooltip registered inside it at that priority among the deferred draws, fitted inside the mask (flipped and clamped as it is to the window) and clipped to it, so it paints and takes clicks only there and never above a higher-priority draw. The layer is the caller's (a host passes its pane's box, not the source's nearest clip, so a tooltip on a row near a list's edge still floats out of the list); deferred draws made inside the layer carry it, and so does the view cache key; `Window::tooltip_layer()` reads the layer in effect, so a host's masked deferred draws take the same mask. A tooltip outside every layer draws as upstream did: above every deferred draw (`usize::MAX`, last registered), unclipped, fitted to the window, hit first; its dispatch parent is now the root node (upstream: whatever node the last deferred draw left active). A `deferred` inside a tooltip is now prepainted (upstream panicked with "must call prepaint before paint"), and draws inside the tooltip's draw. `a_masked_deferred_draw_takes_no_click_outside_its_mask`, `a_later_rounds_lower_priority_draw_takes_no_click_from_above_it`, `a_layered_tooltip_paints_and_takes_clicks_under_a_higher_deferred_draw`, `a_layered_tooltip_fits_inside_its_layer_mask_not_its_sources_clip`, `a_cached_views_tooltip_takes_the_layer_it_is_drawn_in_now` and `a_deferred_draw_inside_a_tooltip_paints` pin it. (4) Deferred draws order as a tree, so a host can stack its panes as deferred draws and keep each view's layer inside its own pane's: a draw registered while another prepaints paints and takes clicks inside it, right after it and before its next sibling, by priority among its siblings; a layered tooltip draws inside the draw its source was drawn in (an unlayered one stays a root draw at `usize::MAX`). A draw or tooltip replayed from a cached view sits inside the draw replaying it now. Upstream sorted every draw by priority alone (and so did (2)), so a draw nested in a back pane at a high priority painted and took clicks over a front pane drawn at a lower one; frames without nested draws order as before. `a_draw_made_in_a_draw_stays_under_that_draws_higher_sibling`, `a_replayed_draw_stays_inside_the_draw_it_is_replayed_in` and `a_layered_tooltip_stays_under_its_source_draws_higher_sibling` pin it.

### A held key's repeat begins no keyboard click

A focused element's Enter or Space click is a press that began on it: the key-down records the focus generation and the key-up clicks if it still matches. A repeat (`KeyDownEvent::is_held`) now records nothing and leaves the record as it is, so a key held since before the element had the focus (a dialog opened under a held Enter and its button took the keys) no longer clicks it on release; a press that began on the element still clicks once, however long it is held. Upstream let each repeat record a new press. X11 reported every repeat as a fresh press (`is_held: false`), and a repeat whose auto-repeat release came in another read than its press passed as a release and a new press; `vendor/gpui-pre-linux` now asks XKB for detectable auto-repeat (no release before a repeat) and reports a press of a key already down as held, forgetting the keys down when the window loses the keyboard. macOS (`isARepeat`) and Wayland (client-side repeat) already did. `keyboard_activation_ignores_a_key_held_from_before_focus` and `keyboard_activation_survives_its_own_repeats` pin it.

### Throttled animations share one frame grid

`App::until_next_animation_frame(fps)` is the time from now to the next multiple of `1 / fps` (whole nanoseconds) since `synced_animation_epoch`, the App clock synced repeating animations already phase-lock to; it is never zero. A `with_max_fps` animation arms its next render on it. Upstream armed each one `1 / max_fps` after its own render, so two animations at one rate started apart each kept their own clock and a window drew both: twice the frames, settling in or out of phase by when each started. Now every animation at one rate wakes on the same instants, and a host's own timed animation (a timer and a notify) that waits on the helper shares them too. Only the wake moves: an animation's value is still elapsed time over its duration. `test_max_fps_animations_started_apart_share_frames` pins it: two 30 fps animations mounted 10 ms apart draw at 30 instants over the next second (59 before).

### A deferred draw's accessibility nodes are its parent's

A deferred draw's accessibility nodes are children of the node it was drawn in, as its dispatch nodes and element ids already were. Upstream prepainted every deferred draw with only the window's root on the node stack, so a popover's or a dialog's deferred content hung off the window: an empty dialog node in place, its controls beside it, and a modal dialog hid them from assistive technology. The node that was finished first cannot be pushed again, so `defer_draw` records the stack's head (`DeferredDraw::a11y_parent`, copied on cache reuse) and `prepaint_deferred_draws` reopens it (`A11yNodeBuilder::reopen`): a collector on the stack under that id takes the draw's top-level nodes (live or replayed by `reuse_range`, whose rule "a parent outside the range is the stack's head" then names the right node), and `finalize` lists them under the finished node. A root draw, or a parent absent this frame, leaves the nodes under the root as before. P7b pins it: the cached card's `deferred(anchored())` button is the card's child on the fresh frame and on every reused one (the root's before).

### A node id a view cannot solve

`GlobalElementId::accesskit_node_id()` is a SipHash of the element's id path (`std::hash::DefaultHasher` over the ids), as it was through 0.3.8, and it is the one source of a node id: a synthetic child's id is hashed from its parent's. 0.3.9 (zed 1dd9bbfd, #64209) folds the path into one hash when a `GlobalElementId` is built, with `FxHasher` seeded by the prefix's hash, and made that cached hash the node id. The cached hash is kept as upstream made it, for the element-state maps, where a collision costs a comparison because equality still compares the ids. As a node id it is the identity, and the Fx fold can be solved by whoever picks an element id: given the hash of the path an element is drawn under, `Integer(x)` has one `x` for every 64-bit target (the multiplier is odd, so the multiply inverts), the window root's 0 included; and `NamedInteger("row", n)` and `NamedInteger("cell", m)` for a computed `m` share a hash under every prefix, with nothing known about the tree. ducktape draws an untrusted view's ids under its own path and keeps its own ids disjoint from a view's by construction; with a solvable node id that held for element state and not for the accessibility tree. A second node with one id is refused (see Refused accessibility nodes), and a host dialog is a deferred draw prepainted after the view's tree, so a view could have the host's own button left out and take an assistive technology's Click on that id. With SipHash a chosen target costs about 2^64 hashes. Two ids of a view's own that collide cost about 2^32 hashes and the whole path they are drawn under, and get that view's own second node refused, as through 0.3.8. The cost of the patch is one SipHash of the path per element that builds a node, only while accessibility is active, as before 0.3.9.

`a_node_id_is_not_solvable_from_an_element_id` forges both ids against the real `ElementId` and asserts the node ids differ. Its precondition is that the forged ids do collide in the cached hash: when it fails, `FxHasher`'s arithmetic, `extend_path_hash` or `ElementId`'s layout changed, and the forgery is re-derived, or the patch dropped if the path hash can no longer be solved. `a_node_id_is_the_one_0_3_8_gave_the_path` compares the id with the hash 0.3.8's tuple struct derived, so node ids did not move with the bump.

When it goes: when upstream derives node ids from something an element id cannot solve (a keyed or cryptographic hash of the path, or ids the window allots). Until then it is cherry-picked like the others; if `src/element.rs` conflicts, the patch is the body of `accesskit_node_id` and the `node_id_tests` module.

### A linear gradient reads back

`Background::as_linear_gradient()` returns a gradient's angle, its two stops and its colour space, as `as_solid()` returns a solid's colour; the fields stay private. A host that bounds an untrusted guest's styles (ducktape's view-wire sanitizer) clamps each number and rebuilds the gradient with `linear_gradient(..).color_space(..)`; without the read it could keep only solids and dropped every gradient. `a_linear_gradient_reads_back_as_it_was_built` pins it.

### taffy 0.14.0

The manifest asks for `taffy = "0.14.0"` (0.3.9 as published asks for `0.13.0`, with default features off and the seven it uses named; that list is kept), and `TaffyLayoutEngine::compute_layout` is ported to it: taffy 0.14 hands the closure of `compute_layout_with_measure` a childless node's whole `LayoutInput` and takes its `LayoutOutput`, so the closure calls `taffy::compute_leaf_layout` around the same measure as before, for a node with or without a measure of its own. That is the leaf algorithm taffy 0.13 ran around the closure. `Style::min_size` and `max_size` are `LengthPercentageAuto` in 0.14, which `Length` already converts to. No public type or signature of gpui's changes, and no test of this crate changed its expectation.

Why: taffy 0.13.0 keeps nine size answers a node, in slots chosen by which dimensions are known. A flex column that is itself stretched by a flex column asks its children the same questions twice, once with its own width known and once without, into one slot, so each pass overwrites the other's answers, and every further such column above a node doubles the times it is computed: a fixed-height row is computed 13, 25, 49 times under one, two, three of them. gpui builds a new taffy tree every frame, so every frame pays for it. taffy 0.14.0 keeps the measure cache's entries by second-chance eviction (taffy#1010) and skips the automatic min-content measurements flexbox does not need (taffy#1119). Measured in ducktape-app (release build, p50 of one solve of a view's tree, 2026-10-08): a trading view of 513 wire nodes whose order book nests three such columns, 2,493 to 1,816 microseconds; the chat view, 556 wire nodes, 846 to 643; the same pixels in both. With taffy alone, where nothing repeats a computation is 10 to 20% dearer on 0.14.0.

When it goes: at the first gpui-pre release that itself depends on taffy 0.14 or later. This commit is then dropped at the bump, not re-applied. Until then it is cherry-picked like the others; if `src/taffy.rs` conflicts, the port is the two changes above. `Cargo.toml.orig` carries the same version and feature list, in place of zed's workspace line, so the two manifests agree.

### AccessKit 0.25

The manifests ask for `accesskit` 0.25.1 (0.3.9 as published asks for 0.24.0), `vendor/gpui-pre-linux` for `accesskit_unix` 0.24.0 (published: 0.22) and `vendor/gpui-pre-macos` for `accesskit_macos` 0.27.1 (published: 0.26.0). This is a fork patch only for its fix: `accesskit_unix` 0.24 brings `accesskit_atspi_common` 0.21, whose AccessKit#788 (6ee0558b) stops exposing a disabled node as `enabled` and `sensitive` over AT-SPI. 0.19 inserted both on every node whose role has no read-only state, disabled or not, so a screen reader on Linux read a disabled button as one that can be pressed. A node that is not disabled reads as before: `read-only` where its role has that state and it is set, `enabled` and `sensitive` otherwise; a disabled node now gets none of the three (0.19 gave a disabled text field `read-only`). Two more Linux changes ride along: a busy node now reads `busy`, and `Role::Image` nodes answer the AT-SPI Image interface (description and extents).

`accesskit` 0.25's one breaking change (property buffers reused when a node is cloned) changes no API gpui calls. `accesskit_macos` 0.27 is not only a dependency move: it changes what VoiceOver reads. Sub-roles it does not map are now none instead of `AXUnknownSubrole` (AccessKit#775). `Label`, `ListBoxOption`, `ListMarker`, `MenuListOption` and `TitleBar` give their label as the value, not the title (#756). A live region announces the node's label, not its value (#782). None of this is checked with VoiceOver yet. `accesskit::Tree` is now a deprecated alias of `TreeInfo`, so the two places that build a `TreeUpdate` (`Window::new`, `A11yNodeBuilder`) name `TreeInfo`. The `Cargo.toml.orig` files take these crates from zed's workspace (`{ workspace = true }`) and have no version line to move. The lockfiles stay the published ones. The two vendor manifest lines are part of the linux and macOS vendors' delta at a refresh. `gpui-pre-windows` is not vendored: 0.3.9 asks for `accesskit` ^0.24 and `accesskit_windows` ^0.34, so a Windows build fails to compile in `gpui-pre-windows` (its 0.24 handler and `TreeUpdate` types against gpui-pre's 0.25 ones).

When it goes: at the first gpui-pre release that itself asks for accesskit 0.25 or later (0.3.8 and 0.3.9 ask for 0.24, `accesskit_unix` 0.22 and `accesskit_macos` 0.26). This commit is then dropped at the bump, not re-applied, along with its vendor manifest lines.

### What the patch does (kept for a future reader)

## wasm guest without JS

The default-on `web` feature retains browser-backed clocks and randomness.
Wire-only guests use `default-features = false`; they retain GPUI's real
`StyleRefinement`, `Styled`, geometry, colors, and serialization types.
No JavaScript import is replaced with a shim.

- GPUI selects chrono's `serde`, `std`, and `clock` explicitly instead of
  its defaults, and the scheduler keeps upstream's `std`, `serde`, and `now`
  without `wasmbind`. `web` restores `chrono/wasmbind`. Native chrono clocks
  keep their existing implementations.
- `web-time` is optional in the vendored scheduler, which exports
  `std::time::Instant` without `web`. GPUI 0.3.9 made its own `web-time`
  optional (only `test-support` asks for it), so GPUI carries no gate for it.
  A bare wasm guest must not call the OS-clock or platform APIs: its host
  provides timing through its guest protocol. Removing the JS dependency
  also removes js-sys's JSPI spawn-poll and wasm-bindgen registration code.
- GPUI's wasm getrandom backend and UUID `js` and `v4` generation features
  require `web`. Native UUID generation remains enabled by a target
  dependency. UUID values and serde remain available. (0.3.9 no longer asks
  uuid for `v5` or `v7`.)
- GPUI and scheduler use seeded rand generators without OS/thread RNG on a
  bare wasm target. Native target dependencies name `os_rng` (and, for GPUI,
  `thread_rng`); `web` restores them for browser builds.
- The scheduler's flume has only its async channel support on every target,
  as published since 0.3.9, so the fork carries no flume gate. Browser
  worker threads imply `web`.
- `gpui-pre-zlog` is not vendored since 0.3.9: `gpui-pre-ztracing` 0.3.9
  takes it only under its `web` and `tracing-tracy` features, so no guest
  graph holds it. If a later release brings it back into the graph, its
  chrono line needs the same gate again (no defaults; `serde`, `clock`,
  `std`).

Consumers must patch `gpui-pre` and every package in `vendor/` from the same
fork revision. Each vendored package's `README.ducktape.md` records its
original crates.io archive SHA-256. Cargo
ignores dependency manifests' patch tables, so patches belong in each
consumer workspace's `[patch.crates-io]` table, not this dependency.

On a gpui-pre bump, refresh the scheduler sources and archive hash,
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
`vendor/gpui-pre-macos`, each 0.3.9 as published plus that one method (see
their `README.ducktape.md`). Wayland and Windows keep the trait default, which
only resizes. Consumers patch `gpui-pre-linux` and `gpui-pre-macos` from the
same revision as `gpui-pre`.

On X11 a window also opens where `WindowOptions::window_bounds` puts it.
Upstream wrote no position into `WM_NORMAL_HINTS`, so the window manager
placed every window itself (openbox: a window asked at 300,200 opened at
640,307), and created it 2 px right of the asked x. The vendored backend
creates it at the asked origin, writes that origin as a program-specified
position, and gives the window static gravity: the x/y name where the
client area sits, which is what `Window::bounds` reports, so a window
reopened or moved to bounds it reported comes back there with no frame-sized
drift (`set_bounds` no longer steps back by `_NET_FRAME_EXTENTS`). Checked
by hand on Xvfb + openbox (see the PR).

### Fixing a window's size

`Window::set_resizable(bool)` lets the person resize a window, or fixes it at
its current size, at any time; `WindowOptions::is_resizable` is read only when
the window opens, and the Linux backends never read it. ducktape's launcher and
desk are one window, the launcher fixed and the desk resizable, so the switch
has to move with the screen. `Window::is_resizable()` follows it, so the
client-side resize edges and the title bar double-click's zoom do too.
`resize` and `set_bounds` still size a fixed window, and it stays fixed at the
size they give it. The backends: macOS toggles `NSResizableWindowMask` (a fixed
window has no zoom or full-screen button), then puts the traffic lights back as
`set_title` does. X11 writes `WM_NORMAL_HINTS` with the window's size as both
its minimum and its maximum (resizable again: the minimum it was opened with
and the GPU's maximum), now also when a window opens not resizable; a fixed
window's `resize` and `set_bounds` write the new size first, so the window
manager never holds it to the old one. Wayland asks the same of the
`xdg_toplevel` (`set_min_size`/`set_max_size` with the window geometry) and
commits. Windows keeps the trait default, which does nothing.
`set_resizable_fixes_the_window_and_lets_it_go` pins the window's state; the
backends were checked by hand (see the PR).

### Linux color emoji

`vendor/gpui-pre-wgpu` is 0.3.9 as published plus one condition in
`load_family`: a known color emoji face is not skipped for lacking a Latin
`m`, so its family loads by name and stays in a caller's fallback chain.
0.3.9 itself stopped removing such a face from the font database (zed
d6358b2d), which was the other half of this patch through 0.3.8. It also
carries gpui-mobile's `offscreen` feature (the headless renderer for
`gpui-pre-mobile`), so the fork holds one `gpui-pre-wgpu`: with a second copy
under `vendor/gpui-pre-mobile`, cargo picked that one for a consumer and the
emoji drew as tofu. See its `README.ducktape.md`. The app repo's
`font_fallback` tests pin the behavior.

### Unhinted glyphs on Linux

`vendor/gpui-pre-wgpu` rasterizes glyphs with swash hinting off (`.hint(false)` in
`render_glyph_image`). swash 0.2.10 hints in skrifa's `Smooth` mode, which snaps a face's
heights and horizontal strokes to whole pixels but leaves its vertical strokes where they fall.
With Instrument Sans' ttfautohint instructions that pulled ascenders down and descenders up by
about a pixel and gave headings hard top and bottom edges, so bold text looked squashed next to
macOS, where Core Text does not hint. Layout was already unhinted (`Hinting::Disabled` in
`layout_line`), so advances and line breaks do not move; only the glyph images do. Drop this
when upstream stops hinting or makes it a choice.

### Korean and Chinese input over XIM

`vendor/zed-xim` is the X11 input-method client `gpui-pre-linux` uses, 0.4.0-zed as published plus a
newer decoder (`xim-ctext` 0.4.1) and no panic on text it cannot decode. ibus, Ubuntu's default input
method, sends Hangul and Chinese through Xlib in KS C 5601 and GB 2312; the published client's decoder read
neither and `.expect()`ed the result, so the first composed syllable crashed the app. See its
`README.ducktape.md`. Consumers patch `zed-xim` from the same revision as `gpui-pre`.

### iOS

Zed added an iOS backend after the 0.3.9 snapshot (zed #63068, `a47f721832`, 2026-10-14): a
new `crates/gpui_ios` plus a core half in `crates/gpui`. The fork carries both so the phone
host (ducktape-app, wasm views on phones) can run on it before a gpui-pre release ships it.

The core half, one commit, is #63068's `crates/gpui/src` diff applied as is (zed's own tests
included; the 0.3.9 snapshot is the commit's direct parent in `crates/gpui/src`, so it applied
without a conflict against our patches): `App::{on_app_lifecycle, on_memory_warning}`
subscriptions wired to the `Platform` hooks of the same names, `AppLifecyclePhase::Disconnected`,
`Window::insets()`, `Window::{notify_text_input_changed, notify_text_selection_changed}`,
`PlatformInputHandler::{focus_id, query_accepts_focused_text_input, selection_bounds_for_range}`
with `InputHandler::selection_bounds_for_range` and its `EntityInputHandler` forward, a public
`Window::last_input_was_touch`, and `Window::focused_text_input: Option<FocusId>` in place of
the bool. Nothing changes for a desktop platform that does not call the hooks. The test platform
records the two callbacks.

`vendor/gpui-pre-ios` is zed's `crates/gpui_ios` at that rev renamed onto the gpui-pre names
(see its `README.ducktape.md` for the three files that differ and the libc note: `backtrace`
0.3.76 needs libc <= 0.2.189 on iOS). It is iOS-only: on any other target it is an empty crate.
Consumers patch it in with the others; `cargo build --target aarch64-apple-ios{,-sim}` on a Mac
is its check.

Two more pieces make a text field usable under the iOS keyboard. `TextInputConfiguration::virtual_keyboard`
(`VirtualKeyboard`: the HTML `inputmode` value set, `Text` by default) is the kind of software
keyboard a field asks for, and `secure` says the field holds a secret; the vendored iOS backend
maps them onto `keyboardType` and `secureTextEntry`, nothing else reads them. And `vendor/gpui-base` is gpui-base 0.7.3
as published plus one commit, "input: the UIKit text contract": the host's `InputState` reports its
length and editable range, takes the selection the keyboard writes, answers selection rectangles,
returns the configuration the host set with `InputState::text_input(cfg)`, tells the platform about
app-driven edits once per frame, and treats a software Return key's inserted newline on a
single-line field as the Enter action (its `README.ducktape.md`). Without it no keyboard rises
for the field at all, and with the length alone Korean doubles every syllable (S0-ios). On a bump:
the `VirtualKeyboard` commit is a plain pick (dropped if upstream grows an equivalent); `vendor/gpui-base`
is a refresh from the new published gpui-base archive with that one commit re-applied, dropped when
longbridge ships it.

### Korean input's first key on macOS

`vendor/gpui-pre-macos` reads a key again when Apple's Korean input method commits its bare jamo into a field that
reports no selection. On the first key after the input source changes, that input method answers `insertText:`
with the key's jamo, then composes the next keys into it by reading it back (`attributedSubstringForProposedRange:`)
and replacing it (a replacement range). A field that reports a selection (gpui-kit's input) ends up right. A field
that reports none cannot give the jamo back, so the input method starts marked text without it on the next key: in
the app's web view (the page's text is the browser's) 한글 typed as ㅎㅏㄴ글. Traced on macOS 27 with Apple Korean
2-Set; winit#4744 reads the key again for the same report (Apple FB17460926). `interpret_key` replaces the key
handler's two `handleEvent:` calls: for a key whose character is one compatibility jamo, not a repeat, no
Control/Option/Command/Function, an Apple Korean input source (`com.apple.inputmethod.Korean.*`), and a field with
nothing marked and no selection, `insertText:` holds back exactly that jamo (no replacement range) while the event
is read; if it was the reading's only answer, the same event is read again, unwatched, and the input method starts
the syllable as marked text (the held jamo is put down only if the second reading answers nothing). Any other
answer to the first reading (marked text, unmark, a command, more text) puts the held jamo down first, in order.
See its `README.ducktape.md`.

### Clipboard reads that wait with the app's loop

`vendor/gpui-pre-linux` adds `gpui_linux::set_clipboard_wait(fn())`. A clipboard read waits on the main thread
for the owner's answer (X11 `read_single`, Wayland `read_fd_with_timeout`); an owner in the same process that
answers only while an event loop on that thread turns (an embedded browser engine's) never answered, so a paste
froze the app. With the function set, each wait runs it between 1 ms polls, within the same timeout. Nothing
changes unless it is set. See its `README.ducktape.md`.

### Guest wire size

The fork also carries `vendor/rmp-serde` 1.3.1 with shared parser operations and a
default-off `typed` decoding feature. It is a consumer patch, not a GPUI dependency.
The view wire enables typed decoding only on wasm; native MessagePack remains
unrestricted. See its `README.ducktape.md` for the archive checksum, exact accepted
shapes, unchanged encoding and bump checks. Consumers patch `rmp-serde` to the
same fork revision alongside the scheduler. Guest builds additionally
share serialization writers and use a pinned Binaryen optimizer; GPUI itself was
not the main remaining size contributor after the JS gates removed its exports.
