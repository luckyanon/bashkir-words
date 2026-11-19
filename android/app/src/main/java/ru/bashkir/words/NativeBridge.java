package ru.bashkir.words;

import android.content.res.AssetManager;
import android.util.Log;
import android.webkit.JavascriptInterface;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import org.json.JSONException;
import org.json.JSONObject;

public final class NativeBridge {
    static { System.loadLibrary("bashkir_words"); }
    private final AssetManager assets;

    NativeBridge(AssetManager assets) {
        this.assets = assets;
    }

    @JavascriptInterface public String dispatch(String requestJson) {
        if (requestJson == null || requestJson.length() > 2_500_000) {
            return "{\"ok\":false,\"error\":\"Invalid native request\"}";
        }
        String response = nativeDispatch(requestJson, assets);
        if (requestJson.startsWith("{\"op\":\"init\",")) {
            try {
                if (response != null && new JSONObject(response).optBoolean("ok")) {
                    Log.i("BashkirRust", "Rust game initialized");
                } else {
                    Log.e("BashkirRust", "Rust game initialization failed");
                }
            } catch (JSONException exception) {
                Log.e("BashkirRust", "Invalid Rust initialization response");
            }
        }
        return response;
    }

    private static native String nativeDispatch(String requestJson, AssetManager assets);

    // Rust reads the same versioned data as WebView. Capacitor stores it in assets/public.
    static byte[] readAsset(AssetManager assets, String path) throws IOException {
        try (InputStream stream = assets.open("public/" + path);
                ByteArrayOutputStream data = new ByteArrayOutputStream()) {
            byte[] chunk = new byte[16384];
            int length;
            while ((length = stream.read(chunk)) != -1) data.write(chunk, 0, length);
            return data.toByteArray();
        }
    }
}
