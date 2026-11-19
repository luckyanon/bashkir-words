package ru.bashkir.words;

import android.content.Context;
import android.content.SharedPreferences;
import android.util.Log;
import org.json.JSONException;
import org.json.JSONObject;

final class NativeSaveImport {
    private static final String SAVE_KEY = "bashkir-words:save";
    private static final String IMPORTED = SAVE_KEY + ":native-rollback-3.1.2-imported";
    private static final String BACKUP = SAVE_KEY + ":before-native-rollback-3.1.2";
    private static final String IMPORTED_300 = SAVE_KEY + ":native-3.0.0-imported";

    private NativeSaveImport() {}

    static void run(Context context) {
        SharedPreferences capacitor = context.getSharedPreferences("CapacitorStorage", Context.MODE_PRIVATE);
        if (capacitor.getBoolean(IMPORTED, false)) return;

        String current = readNative(context, "bashkir-words-renderer");
        if (current == null && !capacitor.getBoolean(IMPORTED_300, false)) {
            current = readNative(context, "bashkir-words-rust");
        }
        if (current == null) return;

        String previous = capacitor.getString(SAVE_KEY, null);
        SharedPreferences.Editor update = capacitor.edit().putString(SAVE_KEY, current).putBoolean(IMPORTED, true);
        if (previous != null && !previous.equals(current) && !capacitor.contains(BACKUP)) {
            update.putString(BACKUP, previous);
        }
        // Capacitor reads preferences from JavaScript as soon as the WebView starts.
        if (!update.commit()) Log.w("NativeSaveImport", "Could not persist the imported save");
    }

    private static String readNative(Context context, String preferences) {
        String value = context.getSharedPreferences(preferences, Context.MODE_PRIVATE).getString("save", null);
        if (value == null || value.length() > 2_000_000) return null;
        try {
            JSONObject state = new JSONObject(value);
            if (state.optInt("version", 0) == 2
                    && state.optInt("generatorVersion", 0) == 6
                    && state.optJSONObject("levels") != null
                    && !state.optString("campaignSeed").isEmpty()) {
                return value;
            }
        } catch (JSONException exception) {
            Log.w("NativeSaveImport", "Could not read the native save format");
        }
        return null;
    }
}
