-keep class com.focusbridge.android.sync.** { *; }
-keep class kotlinx.serialization.** { *; }

# The Rust engine resolves JNI entry points by their fully qualified Java names
# (Java_com_focusbridge_android_sync_secure_NativeSecureChannel_*), so both the
# class and its native method names must survive minification. Renaming them
# fails only at runtime, on the first cross-network connection.
-keepclasseswithmembernames,includedescriptorclasses class * {
    native <methods>;
}
-keep class com.focusbridge.android.sync.secure.NativeSecureChannel { *; }
