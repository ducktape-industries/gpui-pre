# Fork implementation report

Commit: `4b4207b341a6ea2655a883d91b3001e74d7b8dfd` on `work/wasm-guest-no-js` (local only).

Implemented default-on web feature. With defaults off, chrono's implicit
wasmbind feature is absent in GPUI, scheduler, and zlog; scheduler uses std
Instant instead of web-time; GPUI wasm UUID JS/random generation, getrandom
wasm backend, and rand OS/thread RNG are absent. Scheduler retains async
flume channels without unused eventual-fairness/select, cutting fastrand/js.
Native target dependencies retain original rand/flume/UUID capabilities.
No fake imports, stubs, or no-op substitutions were added.

Vendored gpui-pre-scheduler and gpui-pre-zlog 0.3.5 archives retain licenses;
archive SHA-256 values are recorded in each README.ducktape.md. Root README
section "wasm guest without JS" documents gates and bump reapplication.

Consumer patches (all same git URL and revision above):
- gpui-pre
- gpui-pre-scheduler (located under vendor/gpui-pre-scheduler)
- gpui-pre-zlog (located under vendor/gpui-pre-zlog)

Verification: TOML parsing and git diff --check passed. Parent reports the
members-view wasm normal dependency tree now contains none of wasm-bindgen,
js-sys, web-sys, web-time, or getrandom. Parent owns all release builds, ABI,
size measurement, native and wasm gates; no build/test commands were run by
this agent to avoid contention on user-mandated target-w2. Their results
remain pending in the parent report.

Handoff: parent release build and ABI checker, then native gates. Fork
working tree is clean except the untracked target report. Reapply gates and archive
hash refresh on bump; inspect dependency feature union in every consumer.
