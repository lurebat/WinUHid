# WinUHid SDK (`WinUHid.dll`)

The user-mode SDK used by applications that want to virtualise HID devices.

* Public header: [`WinUHid.h`](WinUHid.h). Every entry point is documented
  inline — read it before using it.
* Implementation: [`WinUHid.cpp`](WinUHid.cpp).
* Built as a DLL: `build\<Configuration>\<Platform>\WinUHid.dll`.

## ABI stability

The exported C ABI is *stable*. New functions are added at the end of
the header. Existing structs are frozen — fields can never be reordered,
only appended at the end of a struct guarded by an interface version
check via `WinUHidGetDriverInterfaceVersion()`.

If you're modifying this project, see [`AGENTS.md`](../AGENTS.md) for
the rules.

## Preset devices

Use the core SDK when you want to define your own HID report descriptor.
For ready-made devices such as a mouse, DualShock 4, DualSense, or Xbox
One controller, use [`WinUHidDevs.dll`](../WinUHidDevs/) instead.

## Build flags

The header switches between three modes via preprocessor macros:

| Macro | Effect |
| --- | --- |
| (none — default) | Functions are declared `__declspec(dllimport)` — link against `WinUHid.lib`. |
| `WINUHID_EXPORTS` | Used **only** when building `WinUHid.dll`. Functions are `__declspec(dllexport)`. |
| `WINUHID_STATIC`  | Functions are plain `extern "C"` — useful if you intend to compile `WinUHid.cpp` directly into your binary. |

## Quick example

```c
#include <WinUHid.h>

// 5-byte report: report id, X, Y, button1, button2
static const BYTE descriptor[] = { /* ... */ };

WINUHID_DEVICE_CONFIG cfg = {0};
cfg.SupportedEvents      = WINUHID_EVENT_NONE;
cfg.VendorID             = 0xCAFE;
cfg.ProductID            = 0xBEEF;
cfg.VersionNumber        = 0x0100;
cfg.ReportDescriptor     = descriptor;
cfg.ReportDescriptorLength = sizeof(descriptor);

PWINUHID_DEVICE dev = WinUHidCreateDevice(&cfg);
if (!dev) { /* GetLastError() */ }

WinUHidStartDevice(dev, NULL, NULL);
WinUHidSubmitInputReport(dev, /* ... */);
WinUHidDestroyDevice(dev);
```
