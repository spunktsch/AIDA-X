<#
.SYNOPSIS
    Bundle AIDA-X (CLAP + VST3) and run clap-validator and pluginval on the bundles (TICKET-021).

.DESCRIPTION
    1. `cargo xtask bundle aidadx_plugin --release` -> target/bundled/AIDA-X.clap, AIDA-X.vst3
    2. Downloads pinned clap-validator / pluginval releases into target/tools/<tool>-<version>/
       if they are not there yet (nothing is installed system-wide).
    3. clap-validator validate target/bundled/AIDA-X.clap
       pluginval --strictness-level 5 --validate target/bundled/AIDA-X.vst3
    Exits non-zero as soon as any step fails.

    The versions and download URLs below are UNVERIFIED (written from memory, never
    downloaded). Check them against the GitHub release pages before the first run and fill in
    the SHA-256 constants so later downloads are verified.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/validate-plugin.ps1
    powershell -ExecutionPolicy Bypass -File scripts/validate-plugin.ps1 -SkipBundle -Strictness 8
#>
[CmdletBinding()]
param(
    # Reuse an existing target/bundled/ instead of rebuilding.
    [switch]$SkipBundle,
    # pluginval strictness level (1..10). TICKET-021 requires 5 or higher.
    [ValidateRange(1, 10)]
    [int]$Strictness = 5
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # Invoke-WebRequest is very slow with progress on 5.1

# --- Pinned tool versions (adjust here) ---------------------------------------------------
# https://github.com/free-audio/clap-validator/releases   (tags have no "v" prefix)
$ClapValidatorVersion = '0.3.2'
$ClapValidatorUrl     = "https://github.com/free-audio/clap-validator/releases/download/$ClapValidatorVersion/clap-validator-$ClapValidatorVersion-windows.zip"
$ClapValidatorSha256  = ''   # optional: fill in to verify the download
# https://github.com/Tracktion/pluginval/releases          (tags have a "v" prefix)
$PluginvalVersion     = '1.0.4'
$PluginvalUrl         = "https://github.com/Tracktion/pluginval/releases/download/v$PluginvalVersion/pluginval_Windows.zip"
$PluginvalSha256      = ''   # optional: fill in to verify the download
# ------------------------------------------------------------------------------------------

$RepoRoot   = Split-Path -Parent $PSScriptRoot
$BundleDir  = Join-Path $RepoRoot 'target\bundled'
$ToolsDir   = Join-Path $RepoRoot 'target\tools'
$ClapBundle = Join-Path $BundleDir 'AIDA-X.clap'
$Vst3Bundle = Join-Path $BundleDir 'AIDA-X.vst3'

function Invoke-Checked {
    param([string]$What, [string]$Exe, [string[]]$Arguments)
    Write-Host "==> $What"
    Write-Host "    $Exe $($Arguments -join ' ')"
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$What failed with exit code $LASTEXITCODE"
    }
}

# Download + extract a zip into target/tools/<name>-<version>/ unless already present, and
# return the path of the executable `$ExeName` inside it.
function Get-Tool {
    param([string]$Name, [string]$Version, [string]$Url, [string]$Sha256, [string]$ExeName)

    $dir = Join-Path $ToolsDir "$Name-$Version"
    $exe = Get-ChildItem -Path $dir -Recurse -Filter $ExeName -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($exe) { return $exe.FullName }

    Write-Host "==> Downloading $Name $Version"
    Write-Host "    $Url"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $zip = Join-Path $dir "$Name-$Version.zip"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $Url -OutFile $zip -UseBasicParsing

    if ($Sha256) {
        $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash
        if ($actual -ne $Sha256.ToUpperInvariant()) {
            Remove-Item -Force $zip
            throw "$Name download hash mismatch: expected $Sha256, got $actual"
        }
    } else {
        Write-Warning "$Name download not hash-verified (set the SHA-256 constant in $PSCommandPath)"
    }

    Expand-Archive -Path $zip -DestinationPath $dir -Force
    Remove-Item -Force $zip
    $exe = Get-ChildItem -Path $dir -Recurse -Filter $ExeName -File | Select-Object -First 1
    if (-not $exe) { throw "$ExeName not found in $Url" }
    return $exe.FullName
}

Push-Location $RepoRoot
try {
    if (-not $SkipBundle) {
        Invoke-Checked 'Bundle plugin' 'cargo' @('xtask', 'bundle', 'aidadx_plugin', '--release')
    }
    if (-not (Test-Path $ClapBundle)) { throw "Missing $ClapBundle" }
    if (-not (Test-Path $Vst3Bundle)) { throw "Missing $Vst3Bundle" }

    $clapValidator = Get-Tool -Name 'clap-validator' -Version $ClapValidatorVersion `
        -Url $ClapValidatorUrl -Sha256 $ClapValidatorSha256 -ExeName 'clap-validator.exe'
    $pluginval = Get-Tool -Name 'pluginval' -Version $PluginvalVersion `
        -Url $PluginvalUrl -Sha256 $PluginvalSha256 -ExeName 'pluginval.exe'

    $failures = @()
    try {
        Invoke-Checked 'clap-validator' $clapValidator @('validate', $ClapBundle)
    } catch { $failures += $_.Exception.Message; Write-Host $_.Exception.Message -ForegroundColor Red }
    try {
        Invoke-Checked "pluginval (strictness $Strictness)" $pluginval `
            @('--strictness-level', "$Strictness", '--validate', $Vst3Bundle)
    } catch { $failures += $_.Exception.Message; Write-Host $_.Exception.Message -ForegroundColor Red }

    if ($failures.Count -gt 0) {
        throw "Validation failed:`n  $($failures -join "`n  ")"
    }
    Write-Host '==> All validators passed' -ForegroundColor Green
} finally {
    Pop-Location
}
