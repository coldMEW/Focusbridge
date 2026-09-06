$ErrorActionPreference = 'Stop'
$crate = Split-Path $PSScriptRoot -Parent
& cargo +stable-x86_64-pc-windows-msvc build --locked --examples --lib --manifest-path "$crate/Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }
$classes = Join-Path $crate 'target/host-jvm'
New-Item -ItemType Directory -Force $classes | Out-Null
& javac -d $classes "$PSScriptRoot/NativeSecureChannel.java"
if ($LASTEXITCODE -ne 0) { throw 'Java fixture compilation failed' }
& java -Xcheck:jni -cp $classes com.focusbridge.android.sync.secure.NativeSecureChannel "$crate/target/debug/focusbridge_secure_channel_jni.dll" "$crate/target/debug/examples/host_peer.exe"
if ($LASTEXITCODE -ne 0) { throw 'JNI integration failed' }
