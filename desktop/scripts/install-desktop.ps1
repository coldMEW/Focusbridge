<#
.SYNOPSIS
Installs the desktop build and proves the installed program is the one just built.

.DESCRIPTION
Installing this app is easy to get wrong in a way that looks like it worked.
Windows Installer will not overwrite a file that is in use, and it reports
success anyway, so uninstalling while FocusBridge is still running leaves the
previous build in place. Every symptom then points at the code: a feature that
is "missing", a fix that "did not work", a log line that never appears.

So this stops every instance first, uninstalls, checks the file is actually
gone, installs, and then verifies the installed binary really is the new one.
It fails loudly instead of leaving a stale build behind.

Requires elevation for the install itself.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\install-desktop.ps1
#>
[CmdletBinding()]
param(
    [string]$Msi,
    [string]$Installed = 'C:\Program Files\FocusBridge\focusbridge-desktop.exe',
    # A string present in the new build and absent from the old one. Use it when
    # verifying that a specific change actually shipped -- but only a string that
    # survives into the shipped binary. A literal that lives in a #[cfg(test)]
    # block is not in the release build, and checking for one fails on a perfectly
    # good install.
    [string]$ExpectContains
)

$ErrorActionPreference = 'Stop'

# Uninstalling a per-machine install needs elevation, and without it Windows
# Installer fails with 1603 after appearing to start -- which reads as a broken
# build rather than a missing privilege. Ask for it up front instead, and send
# the elevated run's output to a transcript so it is still readable afterwards.
$identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $transcript = Join-Path $env:TEMP 'focusbridge-install.log'
    # The arguments are written to a script rather than spliced into a -Command
    # string. Building that string by hand put quotes inside quotes, and a path
    # given with -Msi came out the other side with the quotes still attached --
    # which surfaced as "Illegal characters in path" and looked like a bad path
    # rather than a bad relaunch. Passing the values as a hashtable in a file has
    # nothing to escape.
    $payload = @{
        Self       = $MyInvocation.MyCommand.Path
        Parameters = @{}
    }
    foreach ($entry in $PSBoundParameters.GetEnumerator()) {
        $payload.Parameters[$entry.Key] = $entry.Value
    }
    $payloadPath = Join-Path $env:TEMP 'focusbridge-install-args.xml'
    $payload | Export-Clixml -Path $payloadPath
    $runner = Join-Path $env:TEMP 'focusbridge-install-elevated.ps1'
    @"
Start-Transcript -Path '$transcript' -Force | Out-Null
try {
    `$payload = Import-Clixml -Path '$payloadPath'
    # Splatting only works from a plain variable, not a property.
    `$splat = `$payload.Parameters
    & `$payload.Self @splat
} finally {
    Stop-Transcript | Out-Null
}
"@ | Set-Content -Path $runner -Encoding UTF8

    Write-Host "Elevating; accept the prompt. Output goes to $transcript"
    $elevated = Start-Process powershell `
        -ArgumentList @('-ExecutionPolicy', 'Bypass', '-NoProfile', '-File', $runner) `
        -Verb RunAs -Wait -PassThru
    if (Test-Path $transcript) { Get-Content $transcript }
    Remove-Item $payloadPath, $runner -Force -ErrorAction SilentlyContinue
    exit $elevated.ExitCode
}

if (-not $Msi) {
    $root = Split-Path -Parent $MyInvocation.MyCommand.Path
    $Msi = Join-Path $root '..\target\release\bundle\msi\FocusBridge_1.0.0_x64_en-US.msi'
}
if (-not (Test-Path $Msi)) { throw "Installer not found: $Msi. Run 'pnpm tauri build' first." }
$Msi = (Resolve-Path $Msi).Path

Write-Host 'Stopping any running FocusBridge...'
Get-Process focusbridge-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3
if (Get-Process focusbridge-desktop -ErrorAction SilentlyContinue) {
    throw 'FocusBridge is still running. Close it first, or the installer will keep the old files.'
}

Write-Host 'Uninstalling the previous build...'
foreach ($root in 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
                  'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*') {
    Get-ItemProperty $root -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -like '*FocusBridge*' } |
        ForEach-Object {
            $p = Start-Process msiexec.exe -ArgumentList '/x', $_.PSChildName, '/qn', '/norestart' -Wait -PassThru
            if ($p.ExitCode -ne 0) { throw "Uninstall failed with exit code $($p.ExitCode)." }
        }
}

# The check that matters. A locked file survives an uninstall that reports
# success, and the install afterwards will not replace it.
if (Test-Path $Installed) {
    throw "$Installed survived the uninstall, so the installer would keep the old build. Something still has it open."
}

Write-Host 'Installing...'
$p = Start-Process msiexec.exe -ArgumentList '/i', "`"$Msi`"", '/qn', '/norestart' -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "Install failed with exit code $($p.ExitCode)." }
if (-not (Test-Path $Installed)) { throw "Install reported success but $Installed is missing." }

# The exact check: what is installed must be what this installer contains.
#
# Comparing against the build directory was wrong whenever the installer came from
# somewhere else, and Rust builds are not reproducible byte for byte, so a
# perfectly good install failed the check and looked like a broken install.
# Extracting the payload from the very .msi being installed does not care where
# that file came from.
$extract = Join-Path $env:TEMP ('focusbridge-msi-' + [Guid]::NewGuid().ToString('N'))
$payloadExe = $null
try {
    $null = New-Item -ItemType Directory -Path $extract -Force
    $admin = Start-Process msiexec -ArgumentList @('/a', $Msi, '/qn', "TARGETDIR=$extract") -Wait -PassThru
    if ($admin.ExitCode -eq 0) {
        $payloadExe = Get-ChildItem $extract -Recurse -Filter 'focusbridge-desktop.exe' -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty FullName
    }
} catch {
    $payloadExe = $null
}

if ($payloadExe) {
    $expected = (Get-FileHash $payloadExe -Algorithm SHA256).Hash
    $live = (Get-FileHash $Installed -Algorithm SHA256).Hash
    if ($expected -ne $live) {
        throw "The installed binary is not the one in $Msi. Expected $expected, found $live."
    }
    Write-Host "Verified the installed binary matches the installer ($($expected.Substring(0,16))...)."
} else {
    Write-Warning "Could not read the installer payload to compare against; skipping the hash check."
}
Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue

if ($ExpectContains) {
    $bytes = [System.IO.File]::ReadAllBytes($Installed)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    if ($text -notlike "*$ExpectContains*") {
        throw "The installed binary does not contain '$ExpectContains', so it is not the build you just made."
    }
    Write-Host "Verified the installed binary contains '$ExpectContains'."
}

$item = Get-Item $Installed
Write-Host ("Installed {0:N0} bytes, built {1}." -f $item.Length, $item.LastWriteTime)
