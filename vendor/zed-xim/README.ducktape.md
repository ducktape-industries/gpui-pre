# Text from an XIM server that is not UTF-8

Source: https://static.crates.io/crates/zed-xim/zed-xim-0.4.0-zed.crate
Archive SHA-256: `0c0b46ed118eba34d9ba53d94ddc0b665e0e06a2cf874cfa2dd5dec278148642`. Published sources, the MIT
license and the archive's `Cargo.lock` retained (the archive's own `.gitignore` names `Cargo.lock`, so it was
added with `git add -f`); upstream tests unchanged.

Two changes:

- `Cargo.toml`: `xim-ctext` 0.3.0 -> 0.4.1. An XIM server sends preedit and commit text as COMPOUND_TEXT,
  and one built on Xlib (ibus) encodes it with the X locale's charsets: Hangul goes out as KS C 5601
  (`ESC $ ( C`), Chinese as GB 2312 (`ESC $ ( A`), before UTF-8. 0.3.0 decoded only a UTF-8 segment and
  ISO-2022-JP and returned `UnsupportedEncoding` for KS C 5601 and GB 2312; 0.4.1 decodes those, the
  ISO 8859 right halves, and text that switches charsets mid-string. `Cargo.toml.orig` is left as
  published.
- `src/client.rs`: the preedit, commit and reset-reply paths called `.expect()` on the decode, so one
  string the decoder could not read panicked the client's thread (gpui's main thread: the whole app). Now
  `decode_text` logs it and drops that one string; a synchronous commit still gets its `SyncReply`.
  Returning an error instead would end the input method for the window's life: gpui-pre-linux treats any
  client error as the server having died. `hangul_in_ks_c_5601_decodes` and `undecodable_text_is_dropped`
  pin it (`cargo test --lib --features x11rb-client,x11rb-xcb` here).

Drop this vendored copy when the `zed-xim` that gpui-pre-linux asks for decodes KS C 5601 and does not
panic on text it cannot decode.
