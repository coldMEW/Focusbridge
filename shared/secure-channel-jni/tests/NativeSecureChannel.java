package com.focusbridge.android.sync.secure;

import java.util.Arrays;
import java.util.HexFormat;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintWriter;

// Host-only ABI fixture matching the Kotlin object's instance native methods.
public final class NativeSecureChannel {
    native long createPhone(byte[] key, byte[] psk, byte[] pair, byte[] desktop);
    native byte[] writeHandshake(long handle);
    native void readHandshake(long handle, byte[] frame);
    native byte[] writeConfirmation(long handle);
    native void readConfirmation(long handle, byte[] frame);
    native boolean isReady(long handle);
    native byte[][] seal(long handle, byte[] plaintext);
    native byte[] open(long handle, byte[] frame);
    native void close(long handle);

    static byte[] filled(int length, int value) {
        byte[] result = new byte[length];
        Arrays.fill(result, (byte) value);
        return result;
    }
    static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }
    static void fails(Runnable action) {
        try { action.run(); } catch (IllegalStateException expected) { return; }
        throw new AssertionError("Expected fail-closed exception");
    }
    long create() {
        byte[] key = filled(32, 7), psk = filled(32, 9);
        long handle = createPhone(key, psk, filled(16, 1), filled(32, 2));
        check(Arrays.equals(key, new byte[32]) && Arrays.equals(psk, new byte[32]));
        check(handle > 0);
        return handle;
    }
    static String exchange(PrintWriter writer, BufferedReader reader, String command, byte[] bytes) throws Exception {
        writer.println(command + " " + HexFormat.of().formatHex(bytes));
        return reader.readLine();
    }
    void roundTrip(String executable) throws Exception {
        Process peer = new ProcessBuilder(executable).redirectError(ProcessBuilder.Redirect.INHERIT).start();
        long handle = 0;
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(peer.getInputStream()));
             PrintWriter writer = new PrintWriter(peer.getOutputStream(), true)) {
            HexFormat hex = HexFormat.of();
            handle = createPhone(filled(32, 7), filled(32, 9), filled(16, 1), hex.parseHex(reader.readLine()));
            exchange(writer, reader, "readHandshake", writeHandshake(handle));
            readHandshake(handle, hex.parseHex(exchange(writer, reader, "writeHandshake", new byte[0])));
            exchange(writer, reader, "readHandshake", writeHandshake(handle));
            check(!isReady(handle));
            readConfirmation(handle, hex.parseHex(exchange(writer, reader, "writeConfirmation", new byte[0])));
            check(!isReady(handle));
            exchange(writer, reader, "readConfirmation", writeConfirmation(handle));
            check(isReady(handle));
            byte[] plaintext = filled(1024 * 1024, 42);
            byte[][] frames = seal(handle, plaintext);
            check(frames.length > 1);
            for (int i = 0; i < frames.length; i++) {
                String result = exchange(writer, reader, "open", frames[i]);
                if (i + 1 == frames.length) check(Arrays.equals(hex.parseHex(result), plaintext));
                else check(result.equals("partial"));
            }
            String[] incoming = exchange(writer, reader, "seal", plaintext).split(",");
            for (int i = 0; i < incoming.length; i++) {
                byte[] record = open(handle, hex.parseHex(incoming[i]));
                if (i + 1 == incoming.length) check(Arrays.equals(record, plaintext));
                else check(record == null);
            }
            final long replayHandle = handle;
            fails(() -> open(replayHandle, hex.parseHex(incoming[0])));
            fails(() -> isReady(replayHandle));
        } finally {
            close(handle);
            peer.destroy();
            peer.waitFor();
        }
    }
    public static void main(String[] args) throws Exception {
        System.load(args[0]);
        NativeSecureChannel api = new NativeSecureChannel();
        long first = api.create();
        check(!api.isReady(first));
        byte[] initial = api.writeHandshake(first);
        // Noise PSK mode includes an authentication tag even on the initial message.
        check(initial.length == 48);
        fails(() -> api.readHandshake(first, new byte[257]));
        fails(() -> api.isReady(first));
        api.close(first);
        api.close(first);
        fails(() -> api.writeHandshake(-1));
        long second = api.create();
        check(second > first);
        fails(() -> api.seal(second, new byte[1024 * 1024 + 1]));
        fails(() -> api.writeHandshake(second));
        long third = api.create();
        fails(() -> api.open(third, null));
        fails(() -> api.isReady(third));
        long fourth = api.create();
        fails(() -> api.writeConfirmation(fourth));
        long fifth = api.create();
        fails(() -> api.readConfirmation(fifth, new byte[257]));
        byte[] key = filled(31, 7), psk = filled(32, 9);
        fails(() -> api.createPhone(key, psk, filled(16, 1), filled(32, 2)));
        check(Arrays.equals(key, new byte[31]) && Arrays.equals(psk, new byte[32]));
        long[] handles = new long[32];
        for (int i = 0; i < handles.length; i++) handles[i] = api.create();
        fails(() -> api.create());
        for (long handle : handles) api.close(handle);
        long shared = api.create();
        Thread closer = new Thread(() -> api.close(shared));
        closer.start();
        closer.join();
        fails(() -> api.writeHandshake(shared));
        api.roundTrip(args[1]);
        System.out.println("PASS: JNI exports, secret wiping, bounds, capacity, stale handles, close");
        System.out.println("PASS: JNI/Rust desktop handshake, confirmation, 1 MiB bidirectional records, replay rejection");
    }
}
