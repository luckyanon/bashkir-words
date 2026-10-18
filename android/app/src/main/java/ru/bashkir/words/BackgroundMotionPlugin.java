package ru.bashkir.words;

import android.content.Context;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

@CapacitorPlugin(name = "BackgroundMotion")
public class BackgroundMotionPlugin extends Plugin implements SensorEventListener {
    private SensorManager manager;
    private Sensor sensor;
    private boolean requested, listening;
    private long lastSample;
    private final float[] rotation = new float[9], angles = new float[3], gravity = new float[3];
    @Override public void load() {
        manager = (SensorManager) getContext().getSystemService(Context.SENSOR_SERVICE);
        if (manager == null) return;
        sensor = manager.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR);
        if (sensor == null) sensor = manager.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR);
        if (sensor == null) sensor = manager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
    }
    private void listen() {
        if (!requested || listening || sensor == null) return;
        lastSample = 0;
        listening = manager.registerListener(this, sensor, SensorManager.SENSOR_DELAY_GAME);
    }
    private void unlisten() {
        if (manager != null) manager.unregisterListener(this);
        listening = false; lastSample = 0;
    }
    @PluginMethod public void start(PluginCall call) {
        getActivity().runOnUiThread(() -> {requested = true; listen(); JSObject result = new JSObject(); result.put("available", listening); call.resolve(result);});
    }
    @PluginMethod public void stop(PluginCall call) {
        getActivity().runOnUiThread(() -> {requested = false; unlisten(); call.resolve();});
    }
    @Override protected void handleOnPause() {unlisten();}
    @Override protected void handleOnResume() {listen();}
    @Override protected void handleOnDestroy() {requested = false; unlisten();}
    @Override public void onAccuracyChanged(Sensor sensor, int accuracy) {}
    @Override public void onSensorChanged(SensorEvent event) {
        if (!listening) return;
        if (event.timestamp - lastSample < 33000000L) return;
        lastSample = event.timestamp;
        double beta, gamma;
        if (event.sensor.getType() == Sensor.TYPE_ACCELEROMETER) {
            for (int i = 0; i < 3; i++) gravity[i] = gravity[i] * .8f + event.values[i] * .2f;
            beta = Math.toDegrees(Math.atan2(gravity[1], gravity[2]));
            gamma = Math.toDegrees(Math.atan2(-gravity[0], Math.hypot(gravity[1], gravity[2])));
        } else {
            SensorManager.getRotationMatrixFromVector(rotation, event.values);
            // Gravity in device coordinates stays continuous when a phone is held upright.
            // Euler pitch/roll swap axes near 90 degrees and can pin the wallpaper at an edge.
            beta = Math.toDegrees(Math.atan2(rotation[7], rotation[8]));
            gamma = Math.toDegrees(Math.atan2(-rotation[6], Math.hypot(rotation[7], rotation[8])));
        }
        JSObject value = new JSObject();value.put("beta", beta);value.put("gamma", gamma);
        notifyListeners("orientation", value);
    }
}
