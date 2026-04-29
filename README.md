# WinUHid

WinUHid is a Windows user-mode HID virtualisation driver and the user-mode
SDK that drives it. It lets ordinary user-mode processes spawn fully-virtual
Human Interface Devices — keyboards, mice, gamepads, joysticks, custom HID
collections — that the rest of the OS sees as real hardware.

Under the hood, WinUHid is a pair of pieces:

| Component | What it is |
| --- | --- |
| `WinUHid Driver/` | A UMDF driver (`WinUHidDriver.sys`) layered on top of Microsoft's `Vhf.sys` virtual HID framework. It exposes a private IOCTL interface to user mode. |
| `WinUHid/` | `WinUHid.dll` — the user-mode SDK. A small C ABI for creating devices, handling `GET_FEATURE` / `SET_FEATURE` / read / write report events, and submitting input reports. |
| `WinUHidDevs/` | `WinUHidDevs.dll` — opinionated helpers built on top of the SDK that emulate well-known devices: a Microsoft Precision Mouse, a Sony DualShock 4, a Sony DualSense, and an Xbox One gamepad. |
| `WinUHidUnitTests/` | C++ tests using SDL3 to round-trip input through the kernel and back. |
| `Installer/` | A WiX MSI that ships the driver, its certificate, and the installation hooks. |
| `web/` | A Rust + axum web app (and bundled HTML/JS frontend) that wraps the SDK so you can create, exercise, and debug virtual devices from a browser. See [`web/README.md`](web/README.md). |

A diagram of the data flow:

```
+---------------+   IOCTL   +-------------------+   VHF   +----------+
| Your process  | <-------> | WinUHidDriver.sys | <-----> |  Vhf.sys |
| + WinUHid.dll |           +-------------------+         +-----+----+
+---------------+                                               |
                                                                v
                                                       Windows HID stack
                                                       (HidClass, GameInput,
                                                        DirectInput, RawInput…)
```

## Getting started

Prerequisites:

* Windows 10 19041+ or Windows 11 (x86, x64, or ARM64).
* **Visual Studio 2022** with the **Desktop development with C++**
  workload. Newer Visual Studio versions (e.g. 2026) are not yet
  supported because the WDK does not currently ship a matching
  integration component for them.
* The standalone **Windows Driver Kit (WDK) 10.0** matching your
  Windows SDK (e.g. WDK 10.0.26100). Install both the WDK MSI *and*
  the matching `Component.Microsoft.Windows.DriverKit` individual
  component for VS 2022 (in the Visual Studio Installer search for
  *Windows Driver Kit*) — the MSI alone provides only headers/libs;
  the VS component provides the `WindowsUserModeDriver10.0` platform
  toolset that the driver project targets via UMDF v2.
* [`vcpkg`](https://github.com/microsoft/vcpkg) (only required to build
  `WinUHidUnitTests`, which depends on SDL3). Set `VCPKG_ROOT` and
  run `vcpkg integrate install` so MSBuild picks it up.
* (Optional, for the web UI) [Rust](https://rustup.rs/) 1.74+ and
  [`just`](https://github.com/casey/just).
* No separate WiX install is required for the MSI — the `Installer/`
  project uses **WiX v6** via NuGet (`WixToolset.Sdk` 6.x), restored
  automatically by `just restore` / the first build.

Clone, install the prerequisites, and you can build everything from a
regular PowerShell session with:

```powershell
just build               # Release / x64
just test                # build + run unit tests
just web-run             # build & start the web UI on http://127.0.0.1:7878
```

The `just` recipes auto-locate Visual Studio's `msbuild` and
`vstest.console.exe` via `vswhere`, so you only need a developer shell
for raw `msbuild` commands. For example:

```powershell
msbuild WinUHid.sln /p:Configuration=Release /p:Platform=x64 /m
```

Build outputs land in `build/<Configuration>/<Platform>/` next to the
solution file.

### Installing the driver

The driver is **test-signed**. To use it on a machine you must:

1. Enable test signing: `bcdedit /set testsigning on` and reboot.
2. Install the certificate that ships in
   `Installer/WinUHid Package/WinUHidCertificate.cer` into the
   `Trusted Root Certification Authorities` and `Trusted Publishers`
   stores of *Local Machine*.
3. Install the MSI built by the `WinUHid Package` project, **or** run
   `pnputil /add-driver WinUHidDriver.inf /install` against the driver
   produced by an MSBuild of `WinUHid Driver`.

The driver auto-creates a single Plug-and-Play device under
`Root\WinUHid` which is the entry point for user-mode clients.

## Using the SDK

The user-mode API lives in [`WinUHid/WinUHid.h`](WinUHid/WinUHid.h). At
its narrowest, virtualising a HID device is three calls:

```c
PWINUHID_DEVICE dev = WinUHidCreateDevice(&config);
WinUHidStartDevice(dev, /* event callback */ NULL, NULL);
WinUHidSubmitInputReport(dev, report, sizeof(report));
// ...
WinUHidDestroyDevice(dev);
```

The header has thorough comments on every function. Higher-level helpers
for common devices live in [`WinUHidDevs/`](WinUHidDevs/) — for example
a complete mouse is exactly:

```c
PWINUHID_MOUSE_DEVICE mouse = WinUHidMouseCreate(NULL);
WinUHidMouseReportButton(mouse, WUHM_BUTTON_LEFT, TRUE);
WinUHidMouseReportMotion(mouse, 50, 0);
WinUHidMouseReportButton(mouse, WUHM_BUTTON_LEFT, FALSE);
WinUHidMouseDestroy(mouse);
```

## The web UI

If you would rather click than write code, run `just web-run` and open
<http://127.0.0.1:7878>. You'll get:

* A device list and a "Create device" pane with tabs for Mouse, PS4,
  PS5, Xbox One, and a "Generic HID" tab that takes a raw report
  descriptor.
* Per-device debug views with live HID event logs.
* Visual gamepads with **force-press** controls so you can drive any
  button or axis from the browser, plus indicators for any rumble,
  lightbar, or trigger-effect feedback the OS sends back.

See [`web/README.md`](web/README.md) for details and the security
caveats.

## Repository layout

```
.
├── WinUHid/             # WinUHid.dll — user-mode SDK
├── WinUHid Driver/      # WinUHidDriver.sys — UMDF driver
├── WinUHidDevs/         # WinUHidDevs.dll — preset device helpers
├── WinUHidUnitTests/    # C++ tests (SDL3)
├── Installer/           # WiX MSI + dev cert
├── web/                 # Rust web UI
├── .github/workflows/   # CI: builds all configurations on push/PR
├── Justfile             # `just` recipes for everything
├── WinUHid.sln
├── WinUHidCppProps.props
├── README.md            # ← you are here
├── CONTRIBUTING.md
├── AGENTS.md
└── LICENSE              # MIT
```

## Contributing

Bug reports and pull requests are welcome. Please read
[`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a PR.

If you're an AI agent working in this repository, also read
[`AGENTS.md`](AGENTS.md).

## License

MIT — see [`LICENSE`](LICENSE).
