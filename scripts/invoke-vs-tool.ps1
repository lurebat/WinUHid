[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidateSet("msbuild", "vstest")]
    [string]$Tool,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ToolArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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
        [string]$RequestedTool
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

    $toolPath = & $vswherePath -latest -products * -requires $toolConfig.Requires -find $toolConfig.Pattern |
        Select-Object -First 1
    if ($toolPath) {
        return $toolPath
    }

    $toolPath = & $vswherePath -products * -find $toolConfig.Pattern |
        Select-Object -First 1
    if ($toolPath) {
        return $toolPath
    }

    throw "Could not locate $($toolConfig.Executable). Install the matching Visual Studio 2022 components."
}

$toolPath = Resolve-VisualStudioToolPath -RequestedTool $Tool
& $toolPath @ToolArgs
exit $LASTEXITCODE
