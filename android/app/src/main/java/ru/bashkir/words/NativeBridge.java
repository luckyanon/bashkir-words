package ru.bashkir.words;

import android.content.res.AssetManager;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;

final class NativeBridge {
    static { System.loadLibrary("bashkir_words"); }

    private NativeBridge() {}

    static native long create(String save, long nowMillis, int localDay, AssetManager assets);
    static native String dispatch(long session, String eventJson);
    static native void destroy(long session);

    static byte[] readAsset(AssetManager assets, String path) throws IOException {
        try (InputStream stream = assets.open(path); ByteArrayOutputStream data = new ByteArrayOutputStream()) {
            byte[] chunk = new byte[16384];
            int length;
            while ((length = stream.read(chunk)) >= 0) data.write(chunk, 0, length);
            return data.toByteArray();
        }
    }
}
