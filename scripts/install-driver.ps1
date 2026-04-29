<#
.SYNOPSIS
    Installs the WinUHid test certificate and registers WinUHidDriver
    with PnP on a developer machine.

.DESCRIPTION
    This script is intentionally interactive. It will refuse to do
    anything destructive unless:
      * The shell is elevated (Administrator).
      * The host is Windows 10 19041+ or Windows 11.
      * The user types 'INSTALL' verbatim at the confirmation prompt.

    It is not safe to run as part of an autonomous agent run; see
    AGENTS.md rule #8.

.PARAMETER Configuration
    MSBuild configuration that produced the driver (default Release).

.PARAMETER Platform
    MSBuild platform that produced the driver (default x64).

.PARAMETER BuildDir
    Override the artifacts directory entirely. When set,
    Configuration / Platform are ignored for path resolution.
#>
[CmdletBinding()]
param(
    [string]$Configuration = "Release",
    [string]$Platform      = "x64",
    [string]$BuildDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# --- Helpers -------------------------------------------------------------

function Assert-Elevated {
    $id        = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object System.Security.Principal.WindowsPrincipal($id)
    if (-not $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "This script must be run from an elevated PowerShell (Run as Administrator)."
    }
}

function Assert-SupportedOs {
    $os = Get-CimInstance -ClassName Win32_OperatingSystem
    if (-not $os) {
        throw "Could not query Win32_OperatingSystem; refusing to continue."
    }

    $version = [Version]$os.Version
    $minWin10 = [Version]'10.0.19041'
    if ($version -lt $minWin10) {
        throw "WinUHid requires Windows 10 19041+ or Windows 11; detected $($os.Caption) ($($os.Version))."
    }

    Write-Verbose "Detected OS: $($os.Caption) ($($os.Version))"
}

function Test-TestSigningEnabled {
    try {
        $output = & bcdedit /enum '{current}' 2>$null
    } catch {
        return $false
    }
    if ($LASTEXITCODE -ne 0) { return $false }
    return ($output | Select-String -Pattern '^\s*testsigning\s+Yes\s*$' -Quiet)
}

function Read-LiteralConfirmation {
    param(
        [Parameter(Mandatory)] [string] $Expected,
        [Parameter(Mandatory)] [string] $Prompt
    )
    Write-Host ""
    $answer = Read-Host -Prompt $Prompt
    if ($answer -cne $Expected) {
        throw "Confirmation not received (expected '$Expected', got '$answer'). Aborting."
    }
}

function Resolve-BuildDir {
    if ($BuildDir) {
        if (-not (Test-Path -LiteralPath $BuildDir)) {
            throw "BuildDir '$BuildDir' does not exist."
        }
        return (Resolve-Path -LiteralPath $BuildDir).Path
    }

    $repoRoot = Split-Path -Parent $PSScriptRoot
    $candidate = Join-Path -Path $repoRoot -ChildPath ("build/{0}/{1}/WinUHid Driver" -f $Configuration, $Platform)
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw @"
Could not find driver package directory at:
  $candidate

Run `just build-driver $Configuration $Platform` (or `just package`) first,
or pass -BuildDir <path> to point at the directory containing
WinUHidDriver.inf / .dll / .cat.
"@
    }
    return (Resolve-Path -LiteralPath $candidate).Path
}

function Get-DriverArtifacts {
    param([Parameter(Mandatory)] [string] $Dir)

    $inf = Join-Path $Dir 'WinUHidDriver.inf'
    $sys = Join-Path $Dir 'WinUHidDriver.dll'
    $cat = Join-Path $Dir 'WinUHidDriver.cat'

    foreach ($file in @($inf, $sys, $cat)) {
        if (-not (Test-Path -LiteralPath $file)) {
            throw @"
Missing driver artifact: $file

Run `just build-driver` and (if needed) `just package` to produce the
signed driver package, then re-run this script.
"@
        }
    }

    [pscustomobject]@{
        Inf = (Resolve-Path -LiteralPath $inf).Path
        Sys = (Resolve-Path -LiteralPath $sys).Path
        Cat = (Resolve-Path -LiteralPath $cat).Path
    }
}

function Find-Devcon {
    # Check PATH first
    $onPath = Get-Command devcon.exe -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    # Search common WDK install locations
    $candidates = @(
        "C:\Program Files (x86)\Windows Kits\10\Tools\*\x64\devcon.exe",
        "C:\Program Files\Windows Kits\10\Tools\*\x64\devcon.exe"
    )
    foreach ($pattern in $candidates) {
        $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
        if ($found) { return $found.FullName }
    }

    throw @"
devcon.exe not found. It is required to create the Root\WinUHid device node.
Install the Windows Driver Kit (WDK) or add devcon.exe to your PATH.
"@
}

function Get-CertificatePath {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $cer = Join-Path $repoRoot 'Installer/WinUHid Package/WinUHidCertificate.cer'
    if (-not (Test-Path -LiteralPath $cer)) {
        throw "Test-signing certificate not found at: $cer"
    }
    return (Resolve-Path -LiteralPath $cer).Path
}

function Install-CertificateIfMissing {
    param(
        [Parameter(Mandatory)] [string] $StoreName,
        [Parameter(Mandatory)] [System.Security.Cryptography.X509Certificates.X509Certificate2] $Cert
    )

    $existing = Get-ChildItem -Path "Cert:\LocalMachine\$StoreName" -ErrorAction SilentlyContinue |
                Where-Object { $_.Thumbprint -eq $Cert.Thumbprint }
    if ($existing) {
        Write-Host "  [$StoreName] already installed (thumbprint $($Cert.Thumbprint))."
        return
    }

    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store(
        [System.Security.Cryptography.X509Certificates.StoreName]::$StoreName,
        [System.Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine)
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    try {
        $store.Add($Cert)
        Write-Host "  [$StoreName] installed (thumbprint $($Cert.Thumbprint))."
    } finally {
        $store.Close()
    }
}

# --- Main ----------------------------------------------------------------

Assert-Elevated
Assert-SupportedOs

$resolvedBuildDir = Resolve-BuildDir
$artifacts        = Get-DriverArtifacts -Dir $resolvedBuildDir
$certPath         = Get-CertificatePath
$cert             = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($certPath)

Write-Host ""
Write-Host "================ WinUHid driver install ================"
Write-Host "This will perform the following privileged operations:"
Write-Host "  * Add the WinUHid test certificate to:"
Write-Host "      - Cert:\LocalMachine\Root"
Write-Host "      - Cert:\LocalMachine\TrustedPublisher"
Write-Host "    Certificate file:  $certPath"
Write-Host "    Thumbprint:        $($cert.Thumbprint)"
Write-Host "    Subject:           $($cert.Subject)"
Write-Host "  * Register the driver via pnputil /add-driver /install:"
Write-Host "      INF: $($artifacts.Inf)"
Write-Host "      DLL: $($artifacts.Sys)"
Write-Host "      CAT: $($artifacts.Cat)"
Write-Host "  * Create the Root\WinUHid device node via devcon install."
Write-Host "  * pnputil may prompt for a reboot if it touches PnP state."
Write-Host "========================================================"
Write-Host ""

if (-not (Test-TestSigningEnabled)) {
    Write-Warning "bcdedit reports testsigning is NOT enabled."
    Write-Warning "pnputil /install will fail until you run:"
    Write-Warning "    bcdedit /set testsigning on"
    Write-Warning "and reboot."
    Read-LiteralConfirmation -Expected 'YES' `
        -Prompt "Type YES to acknowledge that the install will likely fail"
}

Read-LiteralConfirmation -Expected 'INSTALL' `
    -Prompt "Type INSTALL to proceed"

Write-Host ""
Write-Host "Installing certificate..."
Install-CertificateIfMissing -StoreName 'Root'             -Cert $cert
Install-CertificateIfMissing -StoreName 'TrustedPublisher' -Cert $cert

Write-Host ""
Write-Host "Registering driver with PnP..."
& pnputil.exe /add-driver $artifacts.Inf /install
$pnputilExit = $LASTEXITCODE
if ($pnputilExit -ne 0) {
    Write-Warning "pnputil exited with code $pnputilExit."
    Write-Warning "If this looks like a signature error, ensure test signing is on:"
    Write-Warning "    bcdedit /set testsigning on"
    Write-Warning "    (reboot)"
    Write-Warning "    just install-driver"
    throw "pnputil /add-driver failed (exit code $pnputilExit)."
}

Write-Host ""
Write-Host "Creating Root\WinUHid device node..."
$devcon = Find-Devcon
& $devcon install $artifacts.Inf "Root\WinUHid"
$devExit = $LASTEXITCODE
if ($devExit -ne 0) {
    Write-Warning "devcon install exited with code $devExit."
    Write-Warning "The driver package was registered but the device node could not be created."
    throw "devcon install failed (exit code $devExit)."
}

Write-Host ""
Write-Host "WinUHid driver installed. You can now run ``just web-run``."
