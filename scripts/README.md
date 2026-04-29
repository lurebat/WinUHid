# `scripts/`

Helper PowerShell scripts that wrap privileged WinUHid setup steps.
They are invoked via the `just install-driver` and `just
uninstall-driver` recipes, but you can also run them by hand from an
elevated PowerShell.

## What lives here

* `install-driver.ps1` — installs the WinUHid test-signing
  certificate into `LocalMachine\Root` and `LocalMachine\TrustedPublisher`,
  then registers the freshly-built driver via `pnputil /add-driver
  /install`. Accepts `-Configuration`, `-Platform`, and `-BuildDir`
  to find the right `WinUHidDriver.inf` / `.sys` / `.cat` triple.
* `uninstall-driver.ps1` — walks `pnputil /enum-drivers`, runs
  `pnputil /delete-driver <pubname> /uninstall /force` against every
  `WinUHidDriver*.inf` it finds, then (best-effort) removes the test
  certificate from both local stores.

## Pre-conditions

These scripts will refuse to do anything unless **all** of the
following hold:

1. The PowerShell session is **elevated** (Run as Administrator).
2. The host is Windows 10 19041+ or Windows 11 (verified with
   `Get-CimInstance Win32_OperatingSystem`).
3. The user types `INSTALL` (or `UNINSTALL`) verbatim at the
   confirmation prompt — there is no `-Force` / `-Yes` flag and that
   is deliberate.
4. For installs only: either `bcdedit` reports `testsigning Yes`, or
   the user explicitly acknowledges (`YES` prompt) that
   `pnputil /install` will fail without it.

You also need to have run `just build-driver` (and ideally
`just package`) so the artifacts are present under
`build/<Configuration>/<Platform>/`.

## Why the manual confirmations

Per [`AGENTS.md`](../AGENTS.md) rule #8, automated agents must not run
privileged operations on behalf of a human. The typed-confirmation
gate is a safety rail so these scripts can never silently brick a
non-developer machine even if invoked by accident, by CI, or by a
copy-pasted command line.
