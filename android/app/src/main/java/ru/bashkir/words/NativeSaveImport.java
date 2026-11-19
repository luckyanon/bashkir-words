package ru.bashkir.words;

import android.content.Context;
import android.content.SharedPreferences;
import android.util.Log;
import org.json.JSONException;
import org.json.JSONObject;

final class NativeSaveImport {
    private static final String SAVE_KEY = "bashkir-words:save";
    private static final String IMPORTED = SAVE_KEY + ":native-3.0.0-imported";
    private static final String BACKUP = SAVE_KEY + ":before-native-3.0.0";

    private NativeSaveImport() {}

    static void run(Context context) {
        SharedPreferences legacy = context.getSharedPreferences("CapacitorStorage", Context.MODE_PRIVATE);
        if (legacy.getBoolean(IMPORTED, false)) return;
        SharedPreferences previousNative = context.getSharedPreferences("bashkir-words-rust", Context.MODE_PRIVATE);
        String current = previousNative.getString("save", null);
        if (!isNativeSave(current)) return;
        String previous = legacy.getString(SAVE_KEY, null);
        SharedPreferences.Editor update = legacy.edit().putString(SAVE_KEY, current).putBoolean(IMPORTED, true);
        if (previous != null && !previous.equals(current) && !legacy.contains(BACKUP)) {
            update.putString(BACKUP, previous);
        }
        // Commit before WebView starts so Preferences exposes a single, complete snapshot.
        if (!update.commit()) Log.w("NativeSaveImport", "Could not persist the imported save");
    }

    private static boolean isNativeSave(String value) {
        if (value == null || value.length() > 2_000_000) return false;
        try {
            JSONObject state = new JSONObject(value);
            return state.optInt("version", 0) == 2
                && state.optInt("generatorVersion", 0) == 6
                && state.optJSONObject("levels") != null
                && !state.optString("campaignSeed").isEmpty();
        } catch (JSONException exception) {
            return false;
        }
    }
}
