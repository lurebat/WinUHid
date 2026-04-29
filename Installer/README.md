# Installer

Two artefacts live here:

* [`RootDevCA/`](RootDevCA/) — a tiny C++ project that produces a
  helper used during install/uninstall to register the
  `Root\WinUHid` device node.
* [`WinUHid Package/`](WinUHid%20Package/) — the WiX v3 project that
  produces `WinUHid.msi`. It bundles:
    * The signed `WinUHidDriver.sys` and its catalog.
    * `WinUHid.dll` and `WinUHidDevs.dll`.
    * The dev signing certificate
      (`WinUHidCertificate.cer`).

## Building

```powershell
just package                  # Release x64
just package Debug ARM64
```

Output: `build\<Configuration>\<Platform>\WinUHid.msi`.

## Test signing

The MSI installs the certificate into both *Trusted Root Certification
Authorities* and *Trusted Publishers* on **Local Machine**. That, plus
`bcdedit /set testsigning on` and a reboot, is enough for Windows to
load the test-signed `WinUHidDriver.sys`.

For production deployments you must:

1. Re-sign `WinUHidDriver.sys` and its catalog with your own
   EV / WHQL-attested certificate.
2. Replace `WinUHidCertificate.cer` here with **only** the public
   half of that cert.
3. Re-build the MSI.

**Never** check in a `.pfx` or other private-key material.
