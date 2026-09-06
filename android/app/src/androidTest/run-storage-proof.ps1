param(
    [ValidatePattern('^emulator-[0-9]+$')][string]$Serial = 'emulator-5580',
    [ValidateSet('Baseline', 'Suite', 'Crash', 'All')][string]$Mode = 'All',
    [switch]$Install,
    [string]$Adb = 'C:\Users\DSU\android-sdk\platform-tools\adb.exe'
)

$ErrorActionPreference = 'Stop'
$app = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$output = Join-Path $app ('build\storage-proof\' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $output -Force | Out-Null
$runner = 'com.focusbridge.android.test/androidx.test.runner.AndroidJUnitRunner'
$testClass = 'com.focusbridge.android.storage.EncryptedDatabaseMigrationTest'

function Assert-DiskReserve {
    if ([System.IO.DriveInfo]::new('C').AvailableFreeSpace -lt 2GB) {
        # Only our named disposable emulator may be stopped; never the shared ADB server.
        Get-CimInstance Win32_Process | Where-Object {
            $_.Name -in @('emulator.exe', 'qemu-system-x86_64-headless.exe') -and
            $_.CommandLine -match '-avd focusbridge-storage-proof-api34(?:\s|$)'
        } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        throw 'Stopped storage proof: host is below the 2 GiB reserve. Fixture files preserved.'
    }
}

function Invoke-Adb([string]$Label, [string[]]$Arguments, [int]$TimeoutSeconds = 300) {
    Assert-DiskReserve
    $stdout = Join-Path $output "$Label.stdout.log"
    $stderr = Join-Path $output "$Label.stderr.log"
    $process = Start-Process -FilePath $Adb -ArgumentList (@('-s', $Serial) + $Arguments) `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    # Retain the native handle before a fast ADB command exits (Windows PowerShell 5.1).
    $null = $process.Handle
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    try {
        while (-not $process.WaitForExit(1000)) {
            Assert-DiskReserve
            if ([DateTime]::UtcNow -ge $deadline) { throw "ADB timeout: $Label. Fixture files preserved." }
        }
        $process.WaitForExit()
        $result = (Get-Content -LiteralPath $stdout -Raw) + (Get-Content -LiteralPath $stderr -Raw)
        if ($process.ExitCode -ne 0) { throw "ADB failed ($Label): $result" }
        Write-Host "$Label -> $stdout"
        return $result
    } finally {
        if (-not $process.HasExited) { $process.Kill() }
        $process.Dispose()
    }
}

if (((Invoke-Adb 'identity' @('emu', 'avd', 'name') 15) -split '\s+')[0] -ne 'focusbridge-storage-proof-api34') {
    throw 'Refusing any device other than the dedicated disposable AVD'
}
if ((Invoke-Adb 'qemu' @('shell', 'getprop', 'ro.kernel.qemu') 15).Trim() -ne '1') {
    throw 'Refusing a non-emulator device'
}
if ((Invoke-Adb 'boot' @('shell', 'getprop', 'sys.boot_completed') 15).Trim() -ne '1') {
    throw 'Emulator boot is incomplete'
}
Invoke-Adb 'platform' @('shell', 'getprop') 15 | Out-Null

if ($Install) {
    foreach ($apk in @('outputs\apk\debug\app-debug.apk', 'outputs\apk\androidTest\debug\app-debug-androidTest.apk')) {
        $file = Join-Path $app "build\$apk"
        Get-FileHash -LiteralPath $file -Algorithm SHA256 | Format-List | Out-File -FilePath (Join-Path $output 'apk-sha256.txt') -Append
        $result = Invoke-Adb ([IO.Path]::GetFileNameWithoutExtension($file)) @('install', '-r', '-t', "`"$file`"")
        if ($result -notmatch '(?m)^Success\s*$') { throw "APK installation failed: $result" }
    }
}

function Invoke-Suite([string]$Label, [string]$Classes, [int]$Expected) {
    $result = Invoke-Adb $Label @('shell', 'am', 'instrument', '-w', '-r', '-e', 'class', $Classes, $runner) 600
    if ($result -notmatch "OK \($Expected tests?\)" -or $result -match 'FAILURES!!!|INSTRUMENTATION_FAILED') {
        throw "Native suite failed ($Label): $result"
    }
    Write-Host "$Label PASS ($Expected tests)"
}

if ($Mode -in @('Baseline', 'All')) {
    $baseline = @(
        'migratesVersionsOneTwoAndThreeWithoutLosingData',
        'preservesRoomGeneratedSchemaIdentityAndReopensWithEncryptedFactory',
        'migratesCommittedRowsThatExistOnlyInWal',
        'rejectsWrongAndEmptyKeysWithoutDeletingOrChangingDatabase',
        'tamperedExportIsRejectedBeforeReplacingPlaintextSource',
        'activeWalReaderPreventsReplacementRatherThanDroppingCommittedRows',
        'recoversAtEveryMigrationBoundary',
        'corruptedStageIsReexportedFromIntactSource',
        'refusesMissingSourceInsteadOfPromotingUnprovenStage',
        'corruptSourceIsPreservedAndNeverReplaced',
        'missingWrappedRecordFailsClosedForEncryptedDatabase',
        'missingKeystoreAliasFailsClosedWithoutReplacingRecord',
        'wrappingKeyIsNonExportableAndDoesNotRequireAuthentication',
        'interruptedKeyRecordCannotBeTreatedAsFirstRun'
    ) | ForEach-Object { "$testClass#$_" }
    Invoke-Suite 'baseline-14' ($baseline -join ',') 14
}
if ($Mode -in @('Suite', 'All')) {
    $source = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'java\com\focusbridge\android\storage\EncryptedDatabaseMigrationTest.kt') -Raw
    $expected = [regex]::Matches($source, '@Test fun ').Count
    Invoke-Suite 'expanded-suite' $testClass $expected
}
if ($Mode -in @('Crash', 'All')) {
    foreach ($phase in @('CHECKPOINTED', 'EXPORTED', 'VERIFIED', 'BEFORE_REPLACE', 'REPLACED', 'COMMITTED', 'WAL_COMMITTED', 'KEY_PENDING')) {
        $id = [guid]::NewGuid().ToString()
        $arguments = @('shell', 'am', 'instrument', '-w', '-r', '-e', 'class',
            'com.focusbridge.android.storage.StorageProcessDeathTest#survivesRealProcessDeath',
            '-e', 'storageProofId', $id, '-e', 'storageProofPhase', $phase)
        $death = Invoke-Adb "$phase-kill" ($arguments + @('-e', 'storageProofAction', 'kill', $runner))
        if ($death -notmatch 'Process crashed' -or $death -match 'OK \(') { throw "Expected real process death: $death" }
        $recovery = Invoke-Adb "$phase-recover" ($arguments + @('-e', 'storageProofAction', 'recover', $runner))
        if ($recovery -notmatch 'OK \(1 test\)' -or $recovery -match 'FAILURES!!!|INSTRUMENTATION_FAILED') {
            throw "Recovery failed at $phase (fixture $id): $recovery"
        }
        Write-Host "$phase PASS (SIGKILL, new PID, persisted boundary, recovered data)"
    }
}
Write-Host "Storage proof complete: $output"
