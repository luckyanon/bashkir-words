package ru.bashkir.words;

import android.Manifest;
import android.os.Build;
import android.app.NotificationManager;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.getcapacitor.annotation.Permission;
import com.getcapacitor.annotation.PermissionCallback;
import com.getcapacitor.PermissionState;

@CapacitorPlugin(name = "DailyGift", permissions = {
    @Permission(alias = "notifications", strings = {Manifest.permission.POST_NOTIFICATIONS})
})
public class DailyGiftPlugin extends Plugin {
    @PluginMethod public void sync(PluginCall call) {
        long day = call.getLong("claimedDay", -1L);
        getContext().getSharedPreferences(DailyGiftReceiver.PREFS, 0).edit().putLong("claimedDay", day).apply();
        NotificationManager manager = (NotificationManager) getContext().getSystemService(android.content.Context.NOTIFICATION_SERVICE);
        if (manager != null) manager.cancel(DailyGiftReceiver.ID);
        DailyGiftReceiver.schedule(getContext());
        if (call.getBoolean("requestPermission", false) && Build.VERSION.SDK_INT >= 33
            && getPermissionState("notifications") != PermissionState.GRANTED) {
            requestPermissionForAlias("notifications", call, "permissionResult");
        } else permissionResult(call);
    }
    @PermissionCallback private void permissionResult(PluginCall call) {
        JSObject result = new JSObject();
        result.put("granted", Build.VERSION.SDK_INT < 33 || getPermissionState("notifications") == PermissionState.GRANTED);
        call.resolve(result);
    }
}
