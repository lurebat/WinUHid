# WinUHidDevs (`WinUHidDevs.dll`)

Opinionated, ready-to-use emulations of common HID devices, built on
top of the WinUHid SDK. If you just want a virtual mouse or gamepad,
use these helpers instead of writing a HID descriptor by hand.

| Header | Emulates |
| --- | --- |
| [`WinUHidMouse.h`](WinUHidMouse.h) | Microsoft Precision Mouse (5 buttons + scroll wheel). |
| [`WinUHidPS4.h`](WinUHidPS4.h)     | Sony DualShock 4 (with touchpad, accel, gyro, rumble, lightbar). |
| [`WinUHidPS5.h`](WinUHidPS5.h)     | Sony DualSense / DualSense Edge (with adaptive triggers, mute button, paddles, etc). |
| [`WinUHidXOne.h`](WinUHidXOne.h)   | Xbox One Bluetooth controller (with rumble + impulse triggers). |

All four expose a small create / report-input / destroy API plus
optional callbacks for force-feedback, LEDs, and trigger effects that
the OS or games may send back to the device.

## Building

```powershell
just build-devs
```

Output: `build\<Configuration>\<Platform>\WinUHidDevs.dll`.

## Linking

Link your application against `WinUHidDevs.lib` (and `WinUHid.lib`).
At runtime, `WinUHidDevs.dll` will pick up `WinUHid.dll` from the
same directory or PATH.

## Adding a new preset

1. Add `WinUHidYourDevice.h` exposing a small public ABI matching the
   conventions of the existing files.
2. Add `WinUHidYourDevice.cpp` implementing it on top of `WinUHid.h`.
3. Wire it into `WinUHidDevs.vcxproj`.
4. Add a test to [`WinUHidUnitTests/`](../WinUHidUnitTests/) round-tripping
   real input through SDL3.
5. Wire the new preset into [`web/`](../web/) — the web UI ships a tab
   per preset device.
