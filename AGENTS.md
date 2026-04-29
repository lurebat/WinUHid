# Agent guide

This file is read by automated coding agents (GitHub Copilot CLI, Cursor,
Claude Code, etc.) before they touch the repository. Read it end-to-end
before you do anything destructive.

## What this repository is

A Windows user-mode HID virtualisation stack:

* `WinUHid Driver/` — UMDF driver written in C, layered on `Vhf.sys`.
* `WinUHid/` — `WinUHid.dll`, the user-mode C ABI SDK.
* `WinUHidDevs/` — `WinUHidDevs.dll`, preset emulations (mouse, PS4,
  PS5, Xbox One).
* `WinUHidUnitTests/` — C++ tests using SDL3.
* `Installer/` — WiX MSI plus the dev signing certificate.
* `web/` — Rust + axum + vanilla JS web UI for driving everything.

The whole stack is **Windows-only** by design. Do not port driver code
to Linux/macOS; the `web/` Rust crate also assumes Windows (it dynamically
loads `WinUHid.dll`) and that's intentional.

## Build / test / lint commands

Always prefer the `just` recipes — they encode the right MSBuild flags
and Cargo invocations.

```powershell
just build              # Release x64 (default)
just build Debug x64    # any config / platform
just rebuild
just clean
just test               # build + run unit tests on the current host
just package            # MSI

just web-run            # runs the web UI in dev mode
just web-build          # release build of the web UI
just web-check          # cargo check + clippy -D warnings
just web-fmt            # cargo fmt
just web-test           # cargo test (web crate)

just                    # list all recipes
```

If `just` is not on PATH, install it from <https://github.com/casey/just>
(or `winget install Casey.Just`) — don't invent your own scripts.

## Hard rules for agents

1. **Don't change `LICENSE`** without an explicit request from a human.
2. **Don't modify CI** (`.github/workflows/`) opportunistically. If you
   need a CI tweak, justify it in the PR description.
3. **Don't bump driver / SDK ABI** (the public types in `WinUHid/WinUHid.h`
   and the headers under `WinUHidDevs/`) silently. Adding fields to a
   public struct or changing an enum value is an ABI break — get a human
   review.
4. **Don't introduce new top-level dependencies** without a reason. The
   native build already pulls vcpkg + SDL3 for the unit tests; the web
   crate has its own `Cargo.toml`. New native packages must be discussed
   first.
5. **Don't commit signed binaries, private keys, or `.pfx` files.** The
   only credential in the tree is the public `WinUHidCertificate.cer`
   used for test-signing.
6. **Don't break the build matrix.** CI compiles Debug + Release for
   `x86`, `x64`, and `ARM64`. If your change is x64-only, gate it
   properly in the project files instead of making other configurations
   fail.
7. **Don't reformat unrelated code.** If the file you're editing has a
   weird existing style, match it; don't run a global formatter over the
   tree as part of your PR.
8. **Don't run privileged operations automatically.** Loading the driver
   needs admin and a reboot for `bcdedit /set testsigning on`. Never
   issue these as part of an autonomous run; ask the human.

## When you are working on the driver or SDK

* The driver is C, UMDF v2, no exceptions, no CRT. Use WDF allocators.
* The SDK is C with a stable ABI. New entry points must be added at
  the *end* of `WinUHid.h`. Existing struct layouts are frozen.
* The IOCTL surface between user mode and the driver is private; if you
  add IOCTLs, version-gate them with the existing
  `WinUHidGetDriverInterfaceVersion()` mechanism.
* Always update [`WinUHidUnitTests/`](WinUHidUnitTests/) to cover new
  behaviour you introduce.

## When you are working on the web app

The web app is in [`web/`](web/) and is the place where most agent
churn is expected to happen.

* Backend: Rust 2024-edition, axum + tokio + tracing.
* Frontend: vanilla JS, no build step. Files live in `web/frontend/`
  and are baked into the binary by `rust-embed`.
* All FFI calls into `WinUHid.dll` / `WinUHidDevs.dll` must go through
  `web/src/ffi.rs`. Don't sprinkle `extern "C"` blocks elsewhere.
* The DLLs are loaded **dynamically** via `libloading`. Don't add
  `[link]` directives or `.lib` references — building the web crate
  must work even when the WinUHid native build hasn't been done yet.
* Always run `just web-check` before declaring your work done.

## When you are writing tests / fixtures

* New unit tests go in `WinUHidUnitTests/` and follow the existing
  Microsoft Native Unit Test framework + SDL3 pattern.
* Manual repro steps for browser-driven flows go in `web/README.md`
  under "Manual smoke test".

## Style summary

| Area | Indent | Notes |
| --- | --- | --- |
| Driver / SDK / Devs (C/C++) | tab (4) | `WINUHID_` prefix on public types |
| Unit tests (C++) | tab (4) | TEST_METHOD per scenario |
| Rust | 4 spaces | `cargo fmt`; clippy clean with `-D warnings` |
| HTML/CSS/JS | 2 spaces | no transpilers, no bundler |
| Markdown | 2 spaces | wrap at ~80 columns |

## Anti-patterns to avoid

* Trying to "fix" tabs vs spaces by mass-editing files. Don't.
* Adding TODOs without a tracking issue.
* Logging at `info` in tight per-event paths in the Rust web app.
  Use `trace`/`debug`. The default tracing level is `info`.
* Re-implementing functionality that's already in `WinUHidDevs/` — if
  you're emulating a mouse/PS4/PS5/XOne controller, *use the helpers*.
* Changing the WebSocket message schema without updating both ends and
  the schema documentation in `web/README.md`.
