# WinUHid driver (`WinUHidDriver.sys`)

A UMDF v2 driver that layers on top of Microsoft's
[`Vhf.sys`](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/virtual-hid-framework--vhf--)
to let user-mode processes spawn virtual HID devices.

* `WinUHid.c` — driver entry point and queue/IOCTL dispatch.
* `WinUHid.h` — internal driver-only declarations.
* `Public.h`  — IOCTL codes and structs shared with `WinUHid.dll`.
* `Trace.h`   — WPP tracing macros.
* `WinUHidDriver.inf` — INF describing the `Root\WinUHid` device node.

## Architecture

The driver creates a single Plug-and-Play device node at
`Root\WinUHid`. User mode opens that device with `CreateFile()`
(this is what `WinUHidCreateDevice()` does internally) and exchanges
HID events with the driver over a private IOCTL surface.

For each user-mode-created device, the driver creates a child device
under `Vhf.sys`, forwarding HID descriptors and reports between
user mode and the kernel HID stack.

## Building

```powershell
just build-driver           # Release x64 by default
just build-driver Debug ARM64
```

Or:

```powershell
msbuild "WinUHid Driver/WinUHid Driver.vcxproj" /p:Configuration=Release /p:Platform=x64
```

The driver is **test-signed** with the cert at
`Installer/WinUHid Package/WinUHidCertificate.cer`. To load it on a
machine, see "Installing the driver" in the [top-level README](../README.md).

## Versioning

Whenever the IOCTL surface gains capabilities, bump the interface
version in `Public.h` and have the user-mode SDK call
`WinUHidGetDriverInterfaceVersion()` to feature-gate at runtime.
Never silently change the meaning of an existing IOCTL.
