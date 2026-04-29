# WinUHid unit tests

C++ unit tests for the WinUHid stack, using the Microsoft Native Unit
Test framework (VSTest) and SDL3 to round-trip input through the
kernel and back.

## Layout

| File | What it tests |
| --- | --- |
| [`HIDLL.cpp`](HIDLL.cpp) | Generic devices created via `WinUHidCreateDevice()`. |
| [`PS4.cpp`](PS4.cpp)     | PS4 preset (`WinUHidPS4*`). |
| [`PS5.cpp`](PS5.cpp)     | PS5 preset (`WinUHidPS5*`). |
| [`XOne.cpp`](XOne.cpp)   | Xbox One preset (`WinUHidXOne*`). |
| [`Utilities.{h,cpp}`](Utilities.h) | Shared SDL3 setup and helpers. |

## Prerequisites

* The WinUHid driver must be **installed and loaded** on the test
  machine (test-signing or production-signed). Tests will fail with
  device-creation errors otherwise.
* `vcpkg` integrated with MSBuild (`vcpkg integrate install`) so SDL3
  is restored from [`vcpkg.json`](vcpkg.json).
* The Microsoft Native Unit Test framework, which ships with the C++
  testing tools workload of Visual Studio.

## Running

```powershell
just test                  # Release x64 by default
just test Debug ARM64      # other configurations
```

If you don't have `just`, the equivalent is:

```powershell
msbuild "WinUHidUnitTests/WinUHidUnitTests.vcxproj" /p:Configuration=Release /p:Platform=x64
vstest.console.exe build\Release\x64\WinUHidUnitTests.dll /Platform:x64
```

## Writing new tests

Each test class corresponds to one preset device or one slice of the
generic SDK. Tests should:

1. Create a device.
2. Submit specific input reports.
3. Open the same device through SDL3 (or `RawInput`/`HID` directly).
4. Assert that the OS observed exactly what was submitted.
5. Tear the device down.

Always exercise the **destroy** path — even if a test fails the
mid-way, leaked virtual devices will spook later tests.
