# gpui-base: the UIKit text contract

Source: https://static.crates.io/crates/gpui-base/gpui-base-0.7.3.crate
Archive SHA-256: `2e9955b6f4b048fd50ca15fc8e225d80bb00ba9ebc8fecdddde5ad26dbef9270`. Published sources
and the Apache-2.0 license retained; upstream tests unchanged. longbridge's crate
(`repository = github.com/longbridge/gpui-kit`); the one commit here is offered to them as a
PR and this vendored copy goes when they ship it. Consumers patch it in with the other fork
crates (`[patch.crates-io] gpui-base = { git = fork, rev }`); gpui-kit keeps its `=0.7.3`
pin, which the patch satisfies.

ONE commit, "input: the UIKit text contract" (`src/input/base/state.rs`,
`src/input/base/element.rs`):

iOS's keyboard is a `UITextInput` client that reads the document back and writes the
selection on every keystroke (the Korean keyboard composes a syllable by inserting the jamo,
reading the field, selecting the syllable, deleting it and re-inserting the composed one;
S0-ios §3.2). `gpui_ios` lets a field take the keyboard only when the handler reports a
length or an editable range; with the length alone and selection writes ignored, Korean
doubled every syllable ("안안안녕"). 0.7.3's `EntityInputHandler for InputBaseState`
implemented none of the four methods the contract names and never notified the platform of
its own edits. Now:

1. `text_length_utf16` is the document's UTF-16 length; `text_input_editable_range` is the
   whole document while the field is editable and `None` when disabled or read-only (so a
   read-only field raises no keyboard).
2. `set_selected_text_range` writes the selection through `set_selected_range`.
3. `selection_bounds_for_range` returns one rectangle per visible line the range crosses,
   from the last layout, so iOS can draw its selection handles and place the candidate bar.
4. `text_input_configuration` returns the `TextInputConfiguration` the host set with
   `InputState::text_input(cfg)` / `set_text_input(cfg, cx)` (gpui's shape: autocorrect,
   autocapitalize, suggestions, the Return key's action, and the fork's `virtual_keyboard` and
   `secure`, which the iOS backend maps onto `keyboardType` and `secureTextEntry`). A masked
   field answers `secure` whatever the host set. Default: gpui's default, as before.
5. The platform-entry handlers (`replace_text_in_range`, `replace_and_mark_text_in_range`,
   `unmark_text`, `set_selected_text_range`) tell the platform what they changed before they
   return (`tell_platform`: a text generation counter and the selection against what was last
   told), so the backend's own filter for edits the keyboard made (gpui_ios `native_edit`, set
   only for the duration of the native call) applies, and an app's call to the same methods
   (`insert`, `set_value`, `clean()` after a send) reaches native text services. The element
   repeats the comparison once per frame for selection moves made outside those handlers.
6. The software Return key arrives as inserted text `"\n"`, never as an Enter key event
   (S0-ios §3.3). A single-line field treats an inserted text that is only newlines as the
   Enter action (`InputEvent::PressEnter`, gated by `emit_events` like `Change`; a composition
   in flight is cleared), and strips newlines from mixed text as before; the app's own silent
   replaces (`insert`, `set_value`) are untouched, so `set_value("\n")` still clears the field;
   a multi-line field keeps the newline. No single-line field ever holds a newline.

Besides that commit's two source files and this README, `diff -rq` against the archive lists
`examples/ios-input/` (four files): the on-target probe, gpui-base's `Input` under gpui_ios on the
simulator, which the wasm-mobile Korean flows (`lanes/P/maestro`) type into; not part of the
upstream patch.

Drop this vendored copy when the published crate carries the same.
