# Android secure channel JNI

The Kotlin object loads `focusbridge_secure_channel_jni`. Package the built
`libfocusbridge_secure_channel_jni.so` for each Android ABI; this crate does not
change Gradle, application transport wiring, or deployment.

The nine JNI methods match the Kotlin object's instance methods (not companion
or static methods). Handles are process-local monotonically increasing IDs,
limited to 32 active sessions. Every operation and Java result allocation runs
under one registry mutex. Input, protocol, output-allocation, and panic failures
retire the affected handle. A poisoned registry is emptied and stays unusable.
Exceptions contain no key material. Close is idempotent.

Private key and enrollment PSK must be dedicated 32-byte arrays: createPhone
consumes and wipes them, including invalid-length and capacity failures. Native
copies use Zeroizing. Callers must additionally wipe Java secrets in finally for
library-load/VM failures; Java/VM copies cannot be guaranteed erased. Pair IDs
are exactly 16 bytes and the mandatory desktop pin is exactly 32 bytes. There is
no enrollment fallback. Do not mutate input arrays concurrently with JNI calls.

Handshake/confirmation inputs are capped at 256 bytes, encrypted frames at
65,535 bytes, and nonempty records at 1 MiB, before copying. Malformed secret
arrays are wiped using fixed-size scratch space. Seal returns ordered frames;
open returns null only for an authenticated partial record. The caller owns and
must wipe Java plaintext arrays. Poll isReady from the transport supervisor to
enforce session/partial-record expiry even while idle. Complete the confirmation
exchange before treating the connection as ready.

## Verification

Run from this directory using the installed MSVC stable toolchain:

```powershell
cargo +stable-x86_64-pc-windows-msvc test --locked
cargo +stable-x86_64-pc-windows-msvc clippy --locked --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-msvc fmt --check
./tests/run-host.ps1
```

Verified on host: 11 native tests; Java 17 with `-Xcheck:jni` loads the DLL and
exercises all nine exports, key wiping, invalid inputs, capacity, stale handles,
close, and a real Rust desktop peer. The full JNI exchange covers handshake,
confirmation, bidirectional fragmented 1 MiB records, and replay rejection.
The Java ABI fixture mirrors the Kotlin object; it is not Android device proof.
The host peer uses fixed disposable test keys and is never application code.

Android arm64/x86_64 builds and device/runtime acceptance are separate gates.
Use `./tests/build-android.ps1 -Ndk <installed-ndk-directory>` after NDK setup.
This writes only crate-local target outputs, not app jniLibs or Gradle files.
