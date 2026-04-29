# Building WinUHid from Source

## Prerequisites

- Visual Studio 2022 (Community or Build Tools)
  - Desktop development with C++ workload
  - Windows Driver Kit (WDK) component (`Microsoft.VisualStudio.Component.WDK`)
- Windows SDK 10.0.22621 or later
- Windows Driver Kit 10.0.26100 or later (`winget install Microsoft.WindowsWDK.10.0.26100`)

## Building the User-Mode Library

The user-mode library (`WinUHid.dll` + `WinUHidDevs.dll`) builds without any special setup:

```
msbuild -t:restore -p:RestorePackagesConfig=true
msbuild WinUHid.sln /p:Configuration=Release /p:Platform=x64 /m
```

The library can also be compiled with MinGW/GCC by including the source files directly. A WRL compatibility shim is needed for `wrl/wrappers/corewrappers.h` since MinGW doesn't ship those headers.

## Building the UMDF Driver

The driver project (`WinUHid Driver/`) requires the `WindowsUserModeDriver10.0` platform toolset. This is registered when you install the WDK component through the Visual Studio Installer.

### WPP Tracing (WinUHid.tmh)

The driver uses WPP tracing. The WPP preprocessor generates `WinUHid.tmh` during the build. If your build environment doesn't run WPP (e.g. building outside the full WDK pipeline), create a stub file at `WinUHid Driver/WinUHid.tmh`:

```c
#pragma once
#define WPP_INIT_TRACING(...)
#define WPP_CLEANUP(...)
#define TraceEvents(level, flags, msg, ...)
```

This disables trace logging but the driver functions normally.

### INF Processing (stampinf)

The INF file contains macros (`$ARCH$`, `$UMDFVERSION$`) that must be expanded before the driver can be installed. The WDK build pipeline runs `stampinf` automatically, but if you need to process it manually:

```
stampinf -f "WinUHid Driver\WinUHidDriver.inf" -a amd64 -u 2.23.0 -d * -v *
```

The UMDF version (`-u`) must match the WDF framework on the target system. Common values:
- Windows 10 21H2: `2.23.0`
- Windows 11 22H2+: `2.33.0`

If the version is too high for the target system, the UMDF reflector will refuse to load the driver. Check `setupapi.dev.log` for messages like "Using WDF schema version X.XX when section requires version Y.YY".

### UMDF Version Mismatch

If the driver installs but `WUDFHost.exe` never starts, and `setupapi.dev.log` shows a WDF schema version error, rebuild with headers matching the target OS:

```
WDK include path: C:\Program Files (x86)\Windows Kits\10\Include\wdf\umdf\2.23
WDK lib path: C:\Program Files (x86)\Windows Kits\10\Lib\wdf\umdf\x64\2.23
```

And set `-u 2.23.0` in stampinf.

## Signing

UMDF drivers don't require kernel-mode signing. Self-signed certificates work with Secure Boot enabled.

### For local testing

```powershell
# Create and trust a self-signed certificate
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=WinUHid Test" -CertStoreLocation Cert:\LocalMachine\My
$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPublisher", "LocalMachine")
$store.Open("ReadWrite"); $store.Add($cert); $store.Close()

# Sign the driver
signtool sign /a /sm /s My /n "WinUHid Test" /fd sha256 WinUHidDriver.dll

# Create and sign the catalog
New-FileCatalog -Path $stagingDir -CatalogFilePath WinUHidDriver.cat -CatalogVersion 2.0
signtool sign /a /sm /s My /n "WinUHid Test" /fd sha256 WinUHidDriver.cat
```

### For distribution

- **x86/x64**: OV (Organization Validation) code signing certificate is sufficient
- **ARM64**: Requires WHQL submission through Microsoft's Hardware Developer Program (EV certificate + legal entity)

## Installation

### 1. Create the device node

The `Root\WinUHid` device node must be created through the SetupDI API (`SetupDiCreateDeviceInfoW` + `SetupDiCallClassInstaller(DIF_REGISTERDEVICE)`). The `pnputil /add-device` command doesn't reliably work for this device type.

See `Installer/RootDevCA/RootDevCA.cpp` for the reference implementation.

### 2. Install the driver package

```
stampinf -f WinUHidDriver.inf -a amd64 -u 2.23.0 -d * -v *
pnputil /add-driver WinUHidDriver.inf /install
```

If `pnputil /add-driver` shows "The third-party INF does not contain digital signature information", make sure the catalog file is signed and present alongside the INF.

### 3. Bind the driver

After creating the device node and adding the driver package:

```powershell
# PowerShell - requires P/Invoke for UpdateDriverForPlugAndPlayDevicesW from newdev.dll
UpdateDriverForPlugAndPlayDevicesW(NULL, "Root\WinUHid", "path\to\WinUHidDriver.inf", INSTALLFLAG_FORCE, &needReboot)
```

### 4. Verify

```
pnputil /enum-devices /class System | findstr WinUHid
```

Should show `Status: Started`. Confirm the device is accessible:

```powershell
[System.IO.File]::Open("\\.\WinUHid", "Open", "ReadWrite", "ReadWrite").Close()
```

Note: `Test-Path '\\.\WinUHid'` returns `False` even when the device is working. Use the file handle test above instead.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `wdf.h` not found | WDK not installed or UMDF include path not set | Install WDK component through VS Installer |
| `WinUHid.tmh` not found | WPP preprocessor not running | Create the stub file (see above) |
| `$ARCH$` / `$UMDFVERSION$` in INF | stampinf not run | Run stampinf manually |
| Device "Started" but `\\.\WinUHid` not accessible | WUDFRd service not running | `sc start WUDFRd`, then disable/enable the device |
| Device "Started" but WUDFHost not running | UMDF version mismatch | Check `setupapi.dev.log`, rebuild with matching UMDF version |
| `error 87` from WUDFCoinstaller | `$UMDFVERSION$` not expanded in INF | Run stampinf |
| `error 259` from UpdateDriver | `$ARCH$` not expanded in INF | Run stampinf with `-a amd64` |
