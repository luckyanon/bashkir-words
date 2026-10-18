package ru.bashkir.words;

import android.Manifest;
import android.app.AlarmManager;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;
import androidx.core.app.NotificationCompat;
import androidx.core.content.ContextCompat;
import java.util.Calendar;
import java.util.TimeZone;

/** Local noon reminder; one-shot scheduling recalculates local time after DST and travel. */
public class DailyGiftReceiver extends BroadcastReceiver {
    static final String PREFS = "daily-gift-reminder", CHANNEL = "daily-gift";
    static final int ID = 1200;
    static final String ACTION = "ru.bashkir.words.DAILY_GIFT";
    public static void schedule(Context context) {
        Intent intent = new Intent(context, DailyGiftReceiver.class).setAction(ACTION);
        PendingIntent pending = PendingIntent.getBroadcast(context, ID, intent, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        AlarmManager alarm = (AlarmManager) context.getSystemService(Context.ALARM_SERVICE);
        // No exact-alarm special access: Android may defer the reminder during battery saving.
        if (alarm != null) alarm.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, DailyGiftSchedule.nextNoon(System.currentTimeMillis(), TimeZone.getDefault()), pending);
    }
    @Override public void onReceive(Context context, Intent intent) {
        schedule(context);
        if (!ACTION.equals(intent.getAction())) return;
        Calendar now = Calendar.getInstance();long day = DailyGiftSchedule.localDay(now);
        android.content.SharedPreferences prefs = context.getSharedPreferences(PREFS, 0);
        if (prefs.getLong("claimedDay", -1) >= day || prefs.getLong("notifiedDay", -1) >= day) return;
        if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return;
        NotificationManager manager = (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager == null || !manager.areNotificationsEnabled()) return;
        if (Build.VERSION.SDK_INT >= 26) manager.createNotificationChannel(new NotificationChannel(CHANNEL, "Ежедневный подарок", NotificationManager.IMPORTANCE_DEFAULT));
        PendingIntent open = PendingIntent.getActivity(context, ID, new Intent(context, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        manager.notify(ID, new NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_gift_notification).setContentTitle("Подарок снова доступен!")
            .setContentText("Заберите 100 монет и продолжите играть в «Слова».")
            .setContentIntent(open).setAutoCancel(true).build());
        prefs.edit().putLong("notifiedDay", day).apply();
    }
}
