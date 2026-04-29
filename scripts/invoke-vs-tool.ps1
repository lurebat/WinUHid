[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidateSet("msbuild", "vstest")]
    [string]$Tool,

    # Optional vswhere version range, e.g. "[17.0,18.0)" to pin VS 2022.
    # Falls back to the WINUHID_VS_VERSION environment variable, then to
    # VS 2022 ("[17.0,18.0)") when neither is set — the WDK does not yet
    # ship a matching integration component for newer VS versions.
    [string]$Version,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ToolArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Resolve the version range: explicit -Version > env var > VS 2022 default.
# The WDK does not yet ship integration components for VS versions newer
# than 2022, so we pin to 17.x by default.
if (-not $Version) {
    $Version = if ($env:WINUHID_VS_VERSION) { $env:WINUHID_VS_VERSION } else { '[17.0,18.0)' }
}

function Resolve-VswherePath {
    $vswherePath = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswherePath)) {
        throw "Could not find vswhere.exe at '$vswherePath'. Install Visual Studio 2022 or Build Tools 2022."
    }

    return $vswherePath
}

function Resolve-VisualStudioToolPath {
    param(
        [Parameter(Mandatory)]
        [string]$RequestedTool,

        [string]$VersionRange
    )

    $vswherePath = Resolve-VswherePath

    $toolConfig = switch ($RequestedTool) {
        "msbuild" {
            @{
                Executable = "msbuild.exe"
                Pattern = "MSBuild\**\Bin\MSBuild.exe"
                Requires = "Microsoft.Component.MSBuild"
            }
        }
        "vstest" {
            @{
                Executable = "vstest.console.exe"
                Pattern = "Common7\IDE\CommonExtensions\Microsoft\TestWindow\vstest.console.exe"
                Requires = "Microsoft.VisualStudio.PackageGroup.TestTools.Core"
            }
        }
        default {
            throw "Unsupported tool '$RequestedTool'."
        }
    }

    $toolPath = Get-Command $toolConfig.Executable -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty Source -First 1
    if ($toolPath) {
        return $toolPath
    }

    # Build common vswhere arguments. When a version range is given, use it
    # instead of -latest so the caller can pin e.g. VS 2022 ("[17.0,18.0)").
    $vswhereBase = @('-products', '*')
    if ($VersionRange) {
        $vswhereBase += @('-version', $VersionRange)
    } else {
        $vswhereBase += '-latest'
    }

    $toolPath = & $vswherePath @vswhereBase -requires $toolConfig.Requires -find $toolConfig.Pattern |
        Select-Object -First 1
    if ($toolPath) {
        return $toolPath
    }

    $toolPath = & $vswherePath @vswhereBase -find $toolConfig.Pattern |
        Select-Object -First 1
    if ($toolPath) {
        return $toolPath
    }

    throw "Could not locate $($toolConfig.Executable). Install the matching Visual Studio 2022 components."
}

$toolPath = Resolve-VisualStudioToolPath -RequestedTool $Tool -VersionRange $Version
& $toolPath @ToolArgs
exit $LASTEXITCODE
