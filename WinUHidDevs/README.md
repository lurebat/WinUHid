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

Link your application against `WinUHidDevs.lib` and `WinUHid.lib`. At
runtime, `WinUHidDevs.dll` will pick up `WinUHid.dll` from the same
directory or PATH.

## DualSense guide

This guide covers native use of the DualSense preset from
`WinUHidDevs.dll`. It is not a guide to the web UI.

### Contents

1. [Linking and lifetime](#linking-and-lifetime)
2. [Input reports](#input-reports)
3. [Touch, sensors, and battery](#touch-sensors-and-battery)
4. [Rumble and LEDs](#rumble-and-leds)
5. [Adaptive triggers](#adaptive-triggers)
6. [Custom IDs and DualSense Edge](#custom-ids-and-dualsense-edge)

### Linking and lifetime

Include `WinUHidPS5.h`, link against `WinUHidDevs.lib` and
`WinUHid.lib`, and ship the matching DLLs beside your application or on
`PATH`. A created gamepad stays alive until you call
`WinUHidPS5Destroy`.

Pass callbacks when you want to observe host output reports such as
rumble, LEDs, or adaptive trigger effects. Pass `NULL` for callbacks
your app does not need.

```c
#include <windows.h>
#include <stdio.h>

#include "WinUHidPS5.h"

static VOID OnRumble(PVOID ctx, UCHAR left, UCHAR right);
static VOID OnLightbar(PVOID ctx, UCHAR r, UCHAR g, UCHAR b);
static VOID OnPlayerLed(PVOID ctx, UCHAR value);
static VOID OnTriggerEffect(
    PVOID ctx,
    PCWINUHID_PS5_TRIGGER_EFFECT left,
    PCWINUHID_PS5_TRIGGER_EFFECT right);
static VOID OnMicLed(PVOID ctx, UCHAR state);

typedef struct APP_STATE {
    PWINUHID_PS5_GAMEPAD Pad;
    BOOL Running;
} APP_STATE;

static BOOL StartDualSense(APP_STATE *app) {
    WINUHID_PS5_GAMEPAD_INFO info = { 0 };

    info.MacAddress[0] = 0x02;
    info.MacAddress[1] = 0x11;
    info.MacAddress[2] = 0x22;
    info.MacAddress[3] = 0x33;
    info.MacAddress[4] = 0x44;
    info.MacAddress[5] = 0x55;

    app->Pad = WinUHidPS5Create(
        &info,
        OnRumble,
        OnLightbar,
        OnPlayerLed,
        OnTriggerEffect,
        OnMicLed,
        app);

    if (!app->Pad) {
        printf("WinUHidPS5Create failed: %lu\n", GetLastError());
        return FALSE;
    }

    app->Running = TRUE;
    return TRUE;
}

static VOID StopDualSense(APP_STATE *app) {
    if (app->Pad) {
        WinUHidPS5Destroy(app->Pad);
        app->Pad = NULL;
    }
    app->Running = FALSE;
}
```

### Input reports

Start every report with `WinUHidPS5InitializeInputReport`. Sticks are
unsigned bytes centered at `0x80`, analog triggers are `0..255`, and the
hat helper accepts `-1`, `0`, or `1` per axis.

Set both the analog trigger value and the matching digital `ButtonL2` /
`ButtonR2` bit when you want a full trigger press to look natural to
games.

```c
static UCHAR AxisFromFloat(float value) {
    if (value < -1.0f) value = -1.0f;
    if (value >  1.0f) value =  1.0f;
    return (UCHAR)(128.0f + value * 127.0f);
}

static UCHAR TriggerFromFloat(float value) {
    if (value < 0.0f) value = 0.0f;
    if (value > 1.0f) value = 1.0f;
    return (UCHAR)(value * 255.0f);
}

static BOOL SubmitGameplayInput(PWINUHID_PS5_GAMEPAD pad) {
    WINUHID_PS5_INPUT_REPORT report;
    WinUHidPS5InitializeInputReport(&report);

    report.LeftStickX = AxisFromFloat(-0.75f);
    report.LeftStickY = AxisFromFloat(0.35f);
    report.RightStickX = AxisFromFloat(0.20f);
    report.RightStickY = AxisFromFloat(0.0f);

    report.LeftTrigger = TriggerFromFloat(0.70f);
    report.RightTrigger = TriggerFromFloat(1.0f);
    report.ButtonL2 = report.LeftTrigger > 0;
    report.ButtonR2 = report.RightTrigger > 0;

    report.ButtonCross = TRUE;
    report.ButtonSquare = TRUE;
    report.ButtonL1 = TRUE;
    report.ButtonShare = FALSE;
    report.ButtonOptions = FALSE;
    report.ButtonMute = FALSE;

    WinUHidPS5SetHatState(&report, 1, 0);

    if (!WinUHidPS5ReportInput(pad, &report)) {
        printf("WinUHidPS5ReportInput failed: %lu\n", GetLastError());
        return FALSE;
    }
    return TRUE;
}
```

### Touch, sensors, and battery

The DualSense helper exposes two simultaneous touch contacts. Touch
coordinates use the controller's `1920x1080` touchpad space.
Accelerometer values are in meters per second squared; gyroscope values
are in radians per second.

```c
static BOOL SubmitTouchSensorsAndBattery(PWINUHID_PS5_GAMEPAD pad) {
    WINUHID_PS5_INPUT_REPORT report;
    WinUHidPS5InitializeInputReport(&report);

    WinUHidPS5SetTouchState(&report, 0, TRUE, 960, 540);
    WinUHidPS5SetTouchState(&report, 1, TRUE, 420, 280);

    WinUHidPS5SetAccelState(&report, 0.0f, 0.0f, 9.8f);
    WinUHidPS5SetGyroState(&report, 0.0f, 0.0f, 0.4f);

    WinUHidPS5SetBatteryState(&report, TRUE, 85);
    report.ButtonTouchpad = TRUE;

    return WinUHidPS5ReportInput(pad, &report);
}
```

### Rumble and LEDs

Rumble, lightbar color, player LEDs, and mic LED state are
host-to-device output reports. Your virtual device does not command
these itself; it observes what the OS or game sends.

```c
static VOID OnRumble(PVOID ctx, UCHAR left, UCHAR right) {
    APP_STATE *app = (APP_STATE *)ctx;
    (void)app;
    printf("rumble left=%u right=%u\n", left, right);
}

static VOID OnLightbar(PVOID ctx, UCHAR r, UCHAR g, UCHAR b) {
    (void)ctx;
    printf("lightbar #%02x%02x%02x\n", r, g, b);
}

static VOID OnPlayerLed(PVOID ctx, UCHAR value) {
    (void)ctx;
    printf("player LED mask=0x%02x\n", value & 0x1F);
}

static VOID OnMicLed(PVOID ctx, UCHAR state) {
    (void)ctx;
    const char *name = "unknown";
    if (state == 0) name = "off";
    if (state == 1) name = "solid";
    if (state == 2) name = "pulse";
    printf("mic LED %s (%u)\n", name, state);
}
```

### Adaptive triggers

Games send adaptive trigger effects through output reports. The callback
receives decoded effect payloads; either pointer may be `NULL` when that
trigger has no active effect. The input report also has 4-bit status
fields for reporting trigger state back to the host.

```c
static VOID PrintTriggerEffect(
    const char *label,
    PCWINUHID_PS5_TRIGGER_EFFECT effect)
{
    if (!effect) {
        printf("%s trigger: no effect\n", label);
        return;
    }

    printf("%s trigger effect type=%u data=", label, effect->Type);
    for (UINT i = 0; i < sizeof(effect->Data); ++i) {
        printf("%02x", effect->Data[i]);
    }
    printf("\n");
}

static VOID OnTriggerEffect(
    PVOID ctx,
    PCWINUHID_PS5_TRIGGER_EFFECT left,
    PCWINUHID_PS5_TRIGGER_EFFECT right)
{
    (void)ctx;
    PrintTriggerEffect("left", left);
    PrintTriggerEffect("right", right);
}

static BOOL SubmitTriggerStatus(PWINUHID_PS5_GAMEPAD pad) {
    WINUHID_PS5_INPUT_REPORT report;
    WinUHidPS5InitializeInputReport(&report);

    report.TriggerLeftStatus = 2;
    report.TriggerLeftStopLocation = 8;
    report.TriggerLeftEffect = 1;
    report.TriggerRightStatus = 2;
    report.TriggerRightStopLocation = 10;
    report.TriggerRightEffect = 5;

    return WinUHidPS5ReportInput(pad, &report);
}
```

### Custom IDs and DualSense Edge

Override `BasicInfo` when you need a custom VID, PID, version, instance
ID, or hardware IDs. Use product ID `0x0DF2` for DualSense Edge-specific
controls such as paddles and function buttons.

```c
static PWINUHID_PS5_GAMEPAD CreateDualSenseEdge(PVOID callbackContext) {
    WINUHID_PRESET_DEVICE_INFO basic = { 0 };
    basic.VendorID = 0x054C;
    basic.ProductID = 0x0DF2;
    basic.VersionNumber = 0x0100;

    WINUHID_PS5_GAMEPAD_INFO info = { 0 };
    info.BasicInfo = &basic;
    info.MacAddress[0] = 0x02;
    info.MacAddress[1] = 0xAA;
    info.MacAddress[2] = 0xBB;
    info.MacAddress[3] = 0xCC;
    info.MacAddress[4] = 0xDD;
    info.MacAddress[5] = 0xEE;

    return WinUHidPS5Create(
        &info,
        OnRumble,
        OnLightbar,
        OnPlayerLed,
        OnTriggerEffect,
        OnMicLed,
        callbackContext);
}

static BOOL SubmitEdgeButtons(PWINUHID_PS5_GAMEPAD pad) {
    WINUHID_PS5_INPUT_REPORT report;
    WinUHidPS5InitializeInputReport(&report);

    report.ButtonLeftPaddle = TRUE;
    report.ButtonRightPaddle = TRUE;
    report.ButtonLeftFunction = TRUE;

    return WinUHidPS5ReportInput(pad, &report);
}
```

## Adding a new preset

1. Add `WinUHidYourDevice.h` exposing a small public ABI matching the
   conventions of the existing files.
2. Add `WinUHidYourDevice.cpp` implementing it on top of `WinUHid.h`.
3. Wire it into `WinUHidDevs.vcxproj`.
4. Add a test to [`WinUHidUnitTests/`](../WinUHidUnitTests/) round-tripping
   real input through SDL3.
5. Wire the new preset into [`web/`](../web/) — the web UI ships a tab
   per preset device.
