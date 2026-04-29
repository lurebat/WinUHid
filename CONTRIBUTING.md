# Contributing to WinUHid

Thanks for your interest in helping improve WinUHid. This document
describes the practical things you need to know to get a change merged.

## Ground rules

* **Be kind.** Treat reviewers and other contributors with respect.
* **Keep PRs small and focused.** One concern per PR. Refactors that are
  unrelated to a bug fix or feature should land in their own PR.
* **Don't break the build matrix.** The CI in
  [`.github/workflows/build.yml`](.github/workflows/build.yml) builds
  Debug and Release for `x86`, `x64`, and `ARM64`. All six configurations
  must pass.
* **Don't commit secrets.** This includes signing certificates with
  private keys; only the public dev cert may be checked in.

## Filing issues

When filing an issue, please include:

* The Windows build number (`winver`) and architecture.
* Whether the WinUHid driver is installed test-signed, production-signed,
  or via INF, and which version.
* The smallest reproduction you can produce — ideally a short C snippet
  using `WinUHid.dll` or a reproducible action in the web UI.
* For crashes or bugchecks, the contents of `%WINDIR%\Minidump\` or the
  faulting stack from WinDbg.

## Building

You'll need everything listed in the "Getting started" section of
[`README.md`](README.md). With `just` installed, the relevant recipes
are:

```powershell
just build                      # default: Release x64
just build Debug x64            # specific config
just rebuild                    # clean + build
just test                       # builds + runs WinUHidUnitTests
just package                    # builds the MSI
just web-run                    # runs the Rust web UI
just                            # list all recipes
```

If you don't want to install `just`, every recipe is a one-liner around
`msbuild` or `cargo` — read the `Justfile`, copy what you need.

## Coding style

### C / C++ (driver, SDK, helpers, tests)

* Follow the style already in the file you're editing. Where there's no
  existing convention, the rules below apply.
* **Indentation:** tabs (4-column). Braces on their own line for top-level
  functions and types; same-line braces for control flow.
* **Naming:**
    * Public ABI types and functions: `PascalCase`, prefixed with
      `WINUHID_` / `WinUHid…` and respect the existing
      `PWINUHID_THING` / `PCWINUHID_THING` typedef shapes.
    * File-local functions: `PascalCase` is fine.
    * Local variables: `camelCase` or `snake_case`, just be consistent
      within a function.
* **Headers:** add doc comments to every public ABI function describing
  ownership, threading, and error semantics. Existing entries in
  `WinUHid/WinUHid.h` are good models.
* **No exceptions in the driver.** The UMDF driver is built without
  C++ exceptions. The user-mode SDK is C and stays that way.
* **No CRT in the driver.** The driver code uses `KMDF`/`UMDF` allocators
  and Windows APIs, not `malloc`/`new`/`std::vector`.

### Rust (web UI)

* Run `just web-fmt` (`cargo fmt`) before committing.
* `just web-check` (`cargo check` + `cargo clippy -- -D warnings`) must
  pass cleanly on Windows.
* Public functions get rustdoc comments. Internal helpers do not need
  them, but their *names* should be self-documenting.
* No `unwrap()` / `expect()` in request paths — return an error.
* All Win32 / WinUHid FFI calls must be in `web/src/ffi.rs` and
  documented with the matching `WinUHid.h` declaration.

### Frontend (HTML/CSS/JS)

* Plain ES2020+ JavaScript. No build step, no transpiler, no node
  dependencies. If something gets so complex it really wants a build
  step, raise an issue first so we can discuss it.
* Two-space indent, `const`/`let` over `var`, no semicolon-free style.
* Keep the frontend asset budget tiny — the whole UI ships embedded in
  the `winuhid-web.exe` binary.

## Tests

* Anything that touches the IOCTL surface or HID descriptor parsing
  must come with a test in `WinUHidUnitTests/`. The existing tests use
  SDL3 to round-trip events through the kernel.
* Web UI changes that touch the protocol need at least a manual
  walkthrough described in the PR.

## Driver signing & deployment

Production-signed binaries are built by the maintainer and shipped via
GitHub Releases. Contributors don't need to worry about EV / WHQL
signing — your local development uses the test cert in
`Installer/WinUHid Package/WinUHidCertificate.cer` and `bcdedit /set
testsigning on`.

**Never** commit a private key, `.pfx`, `.snk`, or signed `.cat` for a
production cert.

## Commit & PR conventions

* Commit messages: short imperative subject (≤72 chars), optional body
  explaining *why*. Reference issues with `Fixes #123`.
* Squash-merge friendly — keep your branch tidy. Force-pushes to your
  own PR branch are fine.
* For driver-impacting changes, mention the affected Windows builds in
  the PR description.

## Security

If you find a security vulnerability — particularly anything that lets
an unprivileged process get kernel/admin code execution via the
driver — please **don't** open a public issue. Email the maintainer
listed in `LICENSE` or use GitHub's private security advisory feature.
