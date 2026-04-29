<#
.SYNOPSIS
    Removes every WinUHidDriver registered with PnP and (optionally)
    deletes the test certificate from the LocalMachine stores.

.DESCRIPTION
    This script is intentionally interactive. It will refuse to do
    anything destructive unless:
      * The shell is elevated (Administrator).
      * The host is Windows 10 19041+ or Windows 11.
      * The user types 'UNINSTALL' verbatim at the confirmation prompt.

    See AGENTS.md rule #8 — never run this autonomously.
#>
[CmdletBinding()]
param()

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

function Get-WinUHidPublishedNames {
    $raw = & pnputil.exe /enum-drivers 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "pnputil /enum-drivers failed (exit $LASTEXITCODE)."
    }

    $results       = New-Object System.Collections.Generic.List[pscustomobject]
    $publishedName = $null
    $originalName  = $null

    foreach ($line in $raw) {
        if ($line -match '^\s*Published Name\s*:\s*(\S+)') {
            $publishedName = $Matches[1]
            $originalName  = $null
        } elseif ($line -match '^\s*Original Name\s*:\s*(\S+)') {
            $originalName = $Matches[1]
            if ($publishedName -and $originalName -like 'WinUHidDriver*.inf') {
                $results.Add([pscustomobject]@{
                    PublishedName = $publishedName
                    OriginalName  = $originalName
                }) | Out-Null
            }
        }
    }
    return ,$results
}

function Get-CertificatePath {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $cer = Join-Path $repoRoot 'Installer/WinUHid Package/WinUHidCertificate.cer'
    if (Test-Path -LiteralPath $cer) {
        return (Resolve-Path -LiteralPath $cer).Path
    }
    return $null
}

function Remove-CertificateIfPresent {
    param(
        [Parameter(Mandatory)] [string] $StoreName,
        [Parameter(Mandatory)] [string] $Thumbprint
    )

    $existing = Get-ChildItem -Path "Cert:\LocalMachine\$StoreName" -ErrorAction SilentlyContinue |
                Where-Object { $_.Thumbprint -eq $Thumbprint }
    if (-not $existing) {
        Write-Host "  [$StoreName] no matching certificate."
        return
    }

    foreach ($c in $existing) {
        Remove-Item -Path $c.PSPath -Force
        Write-Host "  [$StoreName] removed certificate $($c.Thumbprint)."
    }
}

function Find-Devcon {
    $onPath = Get-Command devcon.exe -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $candidates = @(
        "C:\Program Files (x86)\Windows Kits\10\Tools\*\x64\devcon.exe",
        "C:\Program Files\Windows Kits\10\Tools\*\x64\devcon.exe"
    )
    foreach ($pattern in $candidates) {
        $found = Get-Item $pattern -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
        if ($found) { return $found.FullName }
    }

    return $null
}

# --- Main ----------------------------------------------------------------

Assert-Elevated
Assert-SupportedOs

$drivers  = Get-WinUHidPublishedNames
$certPath = Get-CertificatePath
$thumbprint = $null
if ($certPath) {
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($certPath)
    $thumbprint = $cert.Thumbprint
}

Write-Host ""
Write-Host "================ WinUHid driver uninstall =============="
if ($drivers.Count -eq 0) {
    Write-Host "No WinUHidDriver*.inf entries found via pnputil /enum-drivers."
} else {
    Write-Host "The following PnP driver packages will be removed:"
    foreach ($d in $drivers) {
        Write-Host ("  - {0} (was {1})" -f $d.PublishedName, $d.OriginalName)
    }
}

if ($thumbprint) {
    Write-Host ""
    Write-Host "If found, the WinUHid test certificate will be removed from:"
    Write-Host "  - Cert:\LocalMachine\Root"
    Write-Host "  - Cert:\LocalMachine\TrustedPublisher"
    Write-Host "  Thumbprint: $thumbprint"
} else {
    Write-Host ""
    Write-Host "Certificate file not found in tree; cert stores will be left alone."
}
Write-Host "========================================================"

Read-LiteralConfirmation -Expected 'UNINSTALL' `
    -Prompt "Type UNINSTALL to proceed"

$failures = 0

# Remove the device node first so the driver can be cleanly unloaded.
Write-Host ""
Write-Host "Removing Root\WinUHid device node(s)..."
$devcon = Find-Devcon
if ($devcon) {
    & $devcon remove "Root\WinUHid"
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "devcon remove exited with code $LASTEXITCODE (device may not exist)."
    }
} else {
    Write-Warning "devcon.exe not found; skipping device node removal."
    Write-Warning "You may need to remove the device manually from Device Manager."
}

foreach ($d in $drivers) {
    Write-Host ""
    Write-Host "Removing $($d.PublishedName)..."
    & pnputil.exe /delete-driver $d.PublishedName /uninstall /force
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "pnputil failed for $($d.PublishedName) (exit $LASTEXITCODE)."
        $failures++
    }
}

if ($thumbprint) {
    Write-Host ""
    Write-Host "Removing certificate from local stores..."
    Remove-CertificateIfPresent -StoreName 'Root'             -Thumbprint $thumbprint
    Remove-CertificateIfPresent -StoreName 'TrustedPublisher' -Thumbprint $thumbprint
}

Write-Host ""
if ($failures -gt 0) {
    throw "WinUHid uninstall completed with $failures pnputil failure(s)."
}
Write-Host "WinUHid driver uninstall complete."
