# Cross-compiles the JNI library for every shipped Android ABI and, unless
# -SkipPackage is given, publishes the stripped release objects into the app's
# jniLibs tree. Debug objects are never packaged.
param(
    [Parameter(Mandatory = $true)][string]$Ndk,
    [switch]$SkipPackage
)
$ErrorActionPreference = 'Stop'
$crate = Split-Path $PSScriptRoot -Parent
# <repo>/shared/secure-channel-jni -> <repo>/android/app/src/main/jniLibs
$repo = Split-Path (Split-Path $crate -Parent) -Parent
$jniLibs = Join-Path $repo 'android/app/src/main/jniLibs'
if (!(Test-Path (Join-Path $repo 'android/app/build.gradle.kts'))) { throw "Cannot locate the Android app from $crate" }
$bin = Join-Path $Ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'
if (!(Test-Path (Join-Path $bin 'clang.exe'))) { throw "NDK clang not installed at $bin" }

# Rust target -> Android ABI directory name. All four ABIs Google Play accepts
# for minSdk 26 are built so no supported device falls back to a missing library.
$abis = [ordered]@{
    'aarch64-linux-android'   = 'arm64-v8a'
    'armv7-linux-androideabi' = 'armeabi-v7a'
    'x86_64-linux-android'    = 'x86_64'
    'i686-linux-android'      = 'x86'
}
# The NDK clang wrapper name uses the "eabi"-less triple for 32-bit ARM.
$clangPrefix = @{ 'armv7-linux-androideabi' = 'armv7a-linux-androideabi' }

$saved = @{}
try {
    foreach ($target in $abis.Keys) {
        $suffix = $target.Replace('-', '_')
        $prefix = if ($clangPrefix.ContainsKey($target)) { $clangPrefix[$target] } else { $target }
        $compiler = Join-Path $bin ($prefix + '26-clang.cmd')
        if (!(Test-Path $compiler)) { throw "NDK compiler missing: $compiler" }
        $settings = @{}
        $settings['CARGO_TARGET_' + $suffix.ToUpperInvariant() + '_LINKER'] = $compiler
        $settings['CC_' + $suffix] = $compiler
        $settings['AR_' + $suffix] = Join-Path $bin 'llvm-ar.exe'
        foreach ($name in $settings.Keys) {
            if (!$saved.ContainsKey($name)) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
            [Environment]::SetEnvironmentVariable($name, $settings[$name], 'Process')
        }
        & cargo +stable-x86_64-pc-windows-msvc build --release --locked --lib --target $target --manifest-path "$crate/Cargo.toml"
        if ($LASTEXITCODE -ne 0) { throw "Android build failed: $target" }
    }
} finally {
    foreach ($name in $saved.Keys) {
        [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process')
    }
}
if ($SkipPackage) { return }

$strip = Join-Path $bin 'llvm-strip.exe'
foreach ($target in $abis.Keys) {
    $built = Join-Path $crate "target/$target/release/libfocusbridge_secure_channel_jni.so"
    if (!(Test-Path $built)) { throw "Missing build output: $built" }
    $destination = Join-Path $jniLibs $abis[$target]
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $published = Join-Path $destination 'libfocusbridge_secure_channel_jni.so'
    Copy-Item $built $published -Force
    # Strip after copying so the crate target directory keeps its symbols.
    if (Test-Path $strip) { & $strip --strip-unneeded $published; if ($LASTEXITCODE -ne 0) { throw "strip failed: $published" } }
    Write-Host ("packaged {0,-12} {1,9:N0} bytes" -f $abis[$target], (Get-Item $published).Length)
}
