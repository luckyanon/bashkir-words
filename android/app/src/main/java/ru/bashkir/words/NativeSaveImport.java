package ru.bashkir.words;

import android.content.Context;
import android.content.SharedPreferences;
import org.json.JSONException;
import org.json.JSONObject;

final class NativeSaveImport {
    private static final String SAVE_KEY = "bashkir-words:save";
    private static final String IMPORTED = SAVE_KEY + ":native-3.0.0-imported";

    private NativeSaveImport() {}

    static String read(Context context) {
        SharedPreferences capacitor = context.getSharedPreferences("CapacitorStorage", Context.MODE_PRIVATE);
        String latest = capacitor.getString(SAVE_KEY, null);
        // 3.0.1 continued writing to Capacitor after importing the 3.0.0 snapshot.
        if (latest != null && capacitor.getBoolean(IMPORTED, false)) return latest;

        String nativeSnapshot = context.getSharedPreferences("bashkir-words-rust", Context.MODE_PRIVATE)
            .getString("save", null);
        return isNativeSave(nativeSnapshot) ? nativeSnapshot : latest;
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
