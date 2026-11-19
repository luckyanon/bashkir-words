package ru.bashkir.words;

import android.Manifest;
import android.app.NotificationManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.res.AssetFileDescriptor;
import android.content.res.Configuration;
import android.graphics.Color;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.media.AudioAttributes;
import android.media.MediaPlayer;
import android.media.SoundPool;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.HapticFeedbackConstants;
import android.widget.FrameLayout;
import android.widget.TextView;
import androidx.activity.ComponentActivity;
import androidx.activity.OnBackPressedCallback;
import androidx.core.graphics.Insets;
import androidx.core.splashscreen.SplashScreen;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import java.io.IOException;
import java.util.Calendar;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import org.json.JSONException;
import org.json.JSONObject;

public class MainActivity extends ComponentActivity implements SensorEventListener {
    private static final String TAG = "BashkirWords";
    private static final String NATIVE_PREFS = "bashkir-words-rust";
    private static final String SAVE_KEY = "save";
    private static final String LEGACY_PREFS = "CapacitorStorage";
    private static final String LEGACY_SAVE_KEY = "bashkir-words:save";
    private static final long TICK_MILLIS = 200;
    private static final int PERMISSION_REQUEST = 1200;

    private long session;
    private GameView game;
    private SharedPreferences storage;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private boolean resumed, touched, motion = true, music, animating, runningTick;
    private String lastSaved;
    private SensorManager sensorManager;
    private Sensor sensor;
    private boolean sensorListening;
    private long lastSensorSample;
    private final float[] rotation = new float[9], gravity = new float[3];
    private SoundPool effectsPlayer;
    private final Map<String, Integer> effectIds = new HashMap<>();
    private final Set<Integer> readyEffects = new HashSet<>();
    private MediaPlayer soundtrack;
    private boolean soundtrackReady, soundtrackFailed;

    private final Runnable tick = new Runnable() {
        @Override public void run() {
            if (!resumed || session == 0) return;
            runningTick = true;
            send("tick", new JSONObject());
            runningTick = false;
            if (resumed) handler.postDelayed(this, animating ? 16 : TICK_MILLIS);
        }
    };

    @Override public void onCreate(Bundle state) {
        SplashScreen.installSplashScreen(this);
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        if (Build.VERSION.SDK_INT < 35) {
            getWindow().setStatusBarColor(Color.TRANSPARENT);
            getWindow().setNavigationBarColor(Color.TRANSPARENT);
        }
        if (Build.VERSION.SDK_INT >= 29) getWindow().setNavigationBarContrastEnforced(false);
        WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView()).setAppearanceLightStatusBars(false);
        WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView()).setAppearanceLightNavigationBars(false);

        storage = getSharedPreferences(NATIVE_PREFS, MODE_PRIVATE);
        lastSaved = storage.getString(SAVE_KEY, null);
        String save = lastSaved;
        if (save == null) {
            save = getSharedPreferences(LEGACY_PREFS, MODE_PRIVATE).getString(LEGACY_SAVE_KEY, null);
        }
        try {
            session = NativeBridge.create(save == null ? "" : save, System.currentTimeMillis(), localDay(), getAssets());
        } catch (LinkageError | RuntimeException error) {
            Log.e(TAG, "Could not start native renderer", error);
        }
        if (session == 0) {
            TextView message = new TextView(this);
            message.setGravity(android.view.Gravity.CENTER);
            message.setText("Не удалось запустить игру. Закройте и откройте приложение.");
            setContentView(message);
            return;
        }

        FrameLayout content = new FrameLayout(this);
        content.setBackgroundColor(Color.rgb(52, 32, 72));
        game = new GameView(this);
        content.addView(game, new FrameLayout.LayoutParams(-1, -1));
        ViewCompat.setOnApplyWindowInsetsListener(content, (view, insets) -> {
            Insets safe = insets.getInsets(WindowInsetsCompat.Type.systemBars()
                | WindowInsetsCompat.Type.displayCutout() | WindowInsetsCompat.Type.ime());
            if (view.getPaddingLeft() != safe.left || view.getPaddingTop() != safe.top
                || view.getPaddingRight() != safe.right || view.getPaddingBottom() != safe.bottom) {
                view.setPadding(safe.left, safe.top, safe.right, safe.bottom);
            }
            return WindowInsetsCompat.CONSUMED;
        });
        setContentView(content);
        ViewCompat.requestApplyInsets(content);
        loadSoundEffects();

        sensorManager = (SensorManager) getSystemService(SENSOR_SERVICE);
        if (sensorManager != null) {
            sensor = sensorManager.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR);
            if (sensor == null) sensor = sensorManager.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR);
            if (sensor == null) sensor = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
        }
        getOnBackPressedDispatcher().addCallback(this, new OnBackPressedCallback(true) {
            @Override public void handleOnBackPressed() { goBack(); }
        });
        DailyGiftReceiver.schedule(this);
    }

    private static int localDay() {
        return (int) DailyGiftSchedule.localDay(Calendar.getInstance());
    }

    private static long nextMidnight() {
        Calendar time = Calendar.getInstance();
        time.add(Calendar.DATE, 1);
        time.set(Calendar.HOUR_OF_DAY, 0);
        time.set(Calendar.MINUTE, 0);
        time.set(Calendar.SECOND, 0);
        time.set(Calendar.MILLISECOND, 0);
        return time.getTimeInMillis();
    }

    private void goBack() {
        if (session == 0) finish();
        else send("back", new JSONObject());
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (game != null) game.refreshSize();
    }

    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (game != null && !hasFocus) game.cancelPointers();
    }

    @Override protected void onResume() {
        super.onResume();
        resumed = true;
        send("resume", new JSONObject());
        refreshSensor();
        refreshMusic();
        handler.post(tick);
    }

    @Override protected void onPause() {
        resumed = false;
        handler.removeCallbacks(tick);
        if (game != null) game.cancelPointers();
        send("pause", new JSONObject());
        refreshSensor();
        refreshMusic();
        if (effectsPlayer != null) effectsPlayer.autoPause();
        super.onPause();
    }

    @Override protected void onDestroy() {
        handler.removeCallbacksAndMessages(null);
        if (session != 0) {
            NativeBridge.destroy(session);
            session = 0;
        }
        if (effectsPlayer != null) effectsPlayer.release();
        if (soundtrack != null) soundtrack.release();
        super.onDestroy();
    }

    void markTouch() {
        if (!touched) {
            touched = true;
            refreshMusic();
        }
    }

    void send(String type, JSONObject event) {
        if (session == 0) return;
        try {
            event.put("type", type);
            event.put("nowMillis", System.currentTimeMillis());
            event.put("localDay", localDay());
            if ("resume".equals(type) || "tick".equals(type)) event.put("nextMidnightMillis", nextMidnight());
            String raw = NativeBridge.dispatch(session, event.toString());
            if (raw == null || raw.isEmpty()) return;
            JSONObject frame = new JSONObject(raw);
            if (frame.has("commands") || frame.has("controls")) game.updateFrame(frame);
            applyEffects(frame);
            JSONObject effects = frame.optJSONObject("effect");
            if (effects != null) applyEffects(effects);
        } catch (JSONException | RuntimeException error) {
            Log.e(TAG, "Native frame could not be applied", error);
        }
    }

    private void applyEffects(JSONObject value) {
        String save = value.optString("save", null);
        if (save != null && !save.equals(lastSaved)) {
            storage.edit().putString(SAVE_KEY, save).apply();
            lastSaved = save;
        }
        if (value.has("claimedDay")) {
            long claimedDay = value.optLong("claimedDay", -1);
            SharedPreferences prefs = getSharedPreferences(DailyGiftReceiver.PREFS, MODE_PRIVATE);
            if (claimedDay != prefs.getLong("claimedDay", -1)) {
                prefs.edit().putLong("claimedDay", claimedDay).apply();
                if (claimedDay >= localDay()) {
                    NotificationManager notifications = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
                    if (notifications != null) notifications.cancel(DailyGiftReceiver.ID);
                }
            }
        }
        if ((value.optBoolean("requestNotifications") || value.optBoolean("giftPermission"))
            && Build.VERSION.SDK_INT >= 33
            && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{ Manifest.permission.POST_NOTIFICATIONS }, PERMISSION_REQUEST);
        }
        if (value.has("animate")) {
            boolean startsAnimation = !animating && value.optBoolean("animate");
            animating = value.optBoolean("animate");
            if (startsAnimation && resumed && !runningTick) {
                handler.removeCallbacks(tick);
                handler.post(tick);
            }
        }
        if (value.has("motion") && motion != value.optBoolean("motion")) {
            motion = value.optBoolean("motion");
            refreshSensor();
        }
        if (value.has("music") && music != value.optBoolean("music")) {
            music = value.optBoolean("music");
            refreshMusic();
        }
        String sound = value.optString("sound");
        if (!sound.isEmpty() && resumed) playSound(sound);
        String haptic = value.optString("haptic");
        if (!haptic.isEmpty() && !"false".equals(haptic) && resumed) {
            game.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK);
        }
        String link = value.optString("openLink");
        if (!link.isEmpty()) openExternal(link);
        if (value.optBoolean("finish") || value.optBoolean("exit")) finish();
    }

    private void openExternal(String value) {
        Uri url = Uri.parse(value);
        if (!"https".equals(url.getScheme()) || url.getHost() == null) return;
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, url));
        } catch (ActivityNotFoundException error) {
            Log.i(TAG, "No browser available");
        }
    }

    private void playSound(String sound) {
        if (effectsPlayer == null) return;
        switch (sound) {
            case "negative": sound = "invalid"; break;
            case "duplicate": sound = "repeat"; break;
            case "positive": sound = "target"; break;
            case "click": sound = "select"; break;
            default: break;
        }
        Integer effectId = effectIds.get(sound);
        if (effectId != null && readyEffects.contains(effectId)) effectsPlayer.play(effectId, 1, 1, 1, 0, 1);
    }

    private void loadSoundEffects() {
        effectsPlayer = new SoundPool.Builder().setMaxStreams(5).setAudioAttributes(
            new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_GAME)
                .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build()).build();
        effectsPlayer.setOnLoadCompleteListener((pool, sample, status) -> {
            if (status == 0) readyEffects.add(sample);
        });
        for (String name : new String[]{"select", "target", "bonus", "repeat", "invalid", "hint", "victory"}) {
            try (AssetFileDescriptor asset = getAssets().openFd("assets/native-sound-" + name + ".wav")) {
                effectIds.put(name, effectsPlayer.load(asset, 1));
            } catch (IOException | RuntimeException error) {
                Log.i(TAG, "Sound unavailable");
            }
        }
    }

    private void refreshMusic() {
        if (!resumed || !touched || !music) {
            if (soundtrackReady && soundtrack.isPlaying()) soundtrack.pause();
            return;
        }
        if (soundtrackFailed) return;
        try {
            if (soundtrack == null) {
                try (AssetFileDescriptor stream = getAssets().openFd("assets/native-music.wav")) {
                    soundtrack = new MediaPlayer();
                    soundtrack.setAudioAttributes(new AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_GAME)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build());
                    soundtrack.setDataSource(stream.getFileDescriptor(), stream.getStartOffset(), stream.getLength());
                    soundtrack.setLooping(true);
                    soundtrack.setOnPreparedListener(player -> {
                        soundtrackReady = true;
                        refreshMusic();
                    });
                    soundtrack.setOnErrorListener((player, what, extra) -> {
                        disableMusic();
                        return true;
                    });
                    soundtrack.prepareAsync();
                }
            }
            if (soundtrackReady) soundtrack.start();
        } catch (IOException | RuntimeException error) {
            disableMusic();
        }
    }

    private void disableMusic() {
        if (soundtrack != null) {
            soundtrack.release();
            soundtrack = null;
        }
        soundtrackReady = false;
        soundtrackFailed = true;
        Log.i(TAG, "Music unavailable");
    }

    private void refreshSensor() {
        if (sensorManager == null || sensor == null) return;
        if (resumed && motion && !sensorListening) {
            lastSensorSample = 0;
            sensorListening = sensorManager.registerListener(this, sensor, SensorManager.SENSOR_DELAY_GAME);
        } else if ((!resumed || !motion) && sensorListening) {
            sensorManager.unregisterListener(this);
            sensorListening = false;
        }
    }

    @Override public void onAccuracyChanged(Sensor sensor, int accuracy) {}

    @Override public void onSensorChanged(SensorEvent event) {
        if (!resumed || !motion || event.timestamp - lastSensorSample < 50_000_000L) return;
        lastSensorSample = event.timestamp;
        double beta, gamma;
        if (event.sensor.getType() == Sensor.TYPE_ACCELEROMETER) {
            for (int i = 0; i < 3; i++) gravity[i] = gravity[i] * .8f + event.values[i] * .2f;
            beta = Math.toDegrees(Math.atan2(gravity[1], gravity[2]));
            gamma = Math.toDegrees(Math.atan2(-gravity[0], Math.hypot(gravity[1], gravity[2])));
        } else {
            SensorManager.getRotationMatrixFromVector(rotation, event.values);
            beta = Math.toDegrees(Math.atan2(rotation[7], rotation[8]));
            gamma = Math.toDegrees(Math.atan2(-rotation[6], Math.hypot(rotation[7], rotation[8])));
        }
        send("sensor", GameView.fields("x", gamma, "y", beta, "gamma", gamma, "beta", beta));
    }
}
