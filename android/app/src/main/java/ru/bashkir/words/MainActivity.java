package ru.bashkir.words;
import android.os.Bundle;
import com.getcapacitor.BridgeActivity;
public class MainActivity extends BridgeActivity {
    @Override public void onCreate(Bundle savedInstanceState) {
        registerPlugin(BackgroundMotionPlugin.class);
        super.onCreate(savedInstanceState);
    }
}
