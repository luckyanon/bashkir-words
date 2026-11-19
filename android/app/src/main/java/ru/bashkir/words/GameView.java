package ru.bashkir.words;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.os.Bundle;
import android.util.SparseArray;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

final class GameView extends View {
    private static final int NO_FOCUS = View.NO_ID;
    private final MainActivity host;
    private final Paint brush = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final SparseArray<float[]> pointers = new SparseArray<>();
    private final AccessibilityManager accessibility;
    private final List<Control> controls = new ArrayList<>();
    private Bitmap surface;
    private boolean surfaceDirty = true;
    private int accessibilityFocus = NO_FOCUS, keyFocus = NO_FOCUS;
    private String controlSignature = "";

    GameView(MainActivity host) {
        super(host);
        this.host = host;
        accessibility = host.getSystemService(AccessibilityManager.class);
        setFocusable(true);
        setFocusableInTouchMode(true);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_YES);
    }

    static JSONObject fields(Object... pairs) {
        JSONObject result = new JSONObject();
        try {
            for (int i = 0; i < pairs.length; i += 2) result.put((String) pairs[i], pairs[i + 1]);
        } catch (JSONException error) {
            throw new IllegalArgumentException("Invalid Android input", error);
        }
        return result;
    }

    private float density() { return getResources().getDisplayMetrics().density; }

    @Override protected void onSizeChanged(int w, int h, int oldw, int oldh) {
        super.onSizeChanged(w, h, oldw, oldh);
        refreshSize();
    }

    void refreshSize() {
        if (getWidth() > 0 && getHeight() > 0) {
            host.send("resize", fields("width", getWidth() / density(), "height", getHeight() / density(), "density", density()));
        }
    }

    void updateFrame(JSONObject frame) {
        surfaceDirty |= frame.optBoolean("render");
        JSONArray newControls = frame.optJSONArray("controls");
        if (newControls != null) updateControls(newControls);
        invalidate();
    }

    private void updateControls(JSONArray source) {
        String signature = source.toString();
        if (signature.equals(controlSignature)) return;
        controlSignature = signature;
        controls.clear();
        for (int i = 0; i < source.length(); i++) {
            JSONObject item = source.optJSONObject(i);
            if (item != null && item.has("id")) controls.add(new Control(item));
        }
        if (control(accessibilityFocus) == null) accessibilityFocus = NO_FOCUS;
        if (control(keyFocus) == null) keyFocus = NO_FOCUS;
        if (accessibility != null && accessibility.isEnabled()) {
            AccessibilityEvent event = AccessibilityEvent.obtain(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
            event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
            sendAccessibilityEventUnchecked(event);
        }
    }

    private static float decimal(JSONObject item, String name) {
        return (float) item.optDouble(name, 0);
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getWidth() <= 0 || getHeight() <= 0) return;
        if (surface == null || surface.getWidth() != getWidth() || surface.getHeight() != getHeight()) {
            surface = Bitmap.createBitmap(getWidth(), getHeight(), Bitmap.Config.ARGB_8888);
            surface.setDensity(Bitmap.DENSITY_NONE);
            surfaceDirty = true;
        }
        if (surfaceDirty) {
            if (!host.render(surface, density(), hasFocus() ? keyFocus : NO_FOCUS)) {
                canvas.drawColor(Color.rgb(52, 32, 72));
                return;
            }
            surfaceDirty = false;
        }
        brush.setStyle(Paint.Style.FILL);
        brush.setAlpha(255);
        canvas.drawBitmap(surface, 0, 0, brush);
    }

    @Override protected void onFocusChanged(boolean focused, int direction, Rect previous) {
        super.onFocusChanged(focused, direction, previous);
        surfaceDirty = true;
        invalidate();
    }

    private void sendPointer(String action, MotionEvent event, int index) {
        int id = event.getPointerId(index);
        float x = event.getX(index) / density(), y = event.getY(index) / density();
        if ("up".equals(action)) pointers.remove(id);
        else if ("move".equals(action)) {
            float[] previous = pointers.get(id);
            if (previous != null && previous[0] == x && previous[1] == y) return;
            pointers.put(id, new float[]{x, y});
        } else pointers.put(id, new float[]{x, y});
        host.send(action, fields("x", x, "y", y, "id", id));
    }

    void cancelPointers() {
        for (int i = 0; i < pointers.size(); i++) {
            float[] point = pointers.valueAt(i);
            host.send("cancel", fields("x", point[0], "y", point[1], "id", pointers.keyAt(i)));
        }
        pointers.clear();
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                markActivation();
                sendPointer("down", event, event.getActionIndex());
                return true;
            case MotionEvent.ACTION_POINTER_DOWN:
                sendPointer("down", event, event.getActionIndex());
                return true;
            case MotionEvent.ACTION_MOVE:
                for (int i = 0; i < event.getPointerCount(); i++) sendPointer("move", event, i);
                return true;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_POINTER_UP:
                sendPointer("up", event, event.getActionIndex());
                return true;
            case MotionEvent.ACTION_CANCEL:
                cancelPointers();
                return true;
            default: return super.onTouchEvent(event);
        }
    }

    @Override public boolean onGenericMotionEvent(MotionEvent event) {
        if (event.getActionMasked() != MotionEvent.ACTION_SCROLL) return super.onGenericMotionEvent(event);
        host.send("scroll", fields("x", event.getX() / density(), "y", event.getY() / density(),
            "delta", -event.getAxisValue(MotionEvent.AXIS_VSCROLL)));
        return true;
    }

    private void markActivation() {
        host.markTouch();
        if (keyFocus != NO_FOCUS) surfaceDirty = true;
        keyFocus = NO_FOCUS;
        requestFocus();
    }

    @Override public boolean onKeyDown(int key, KeyEvent event) {
        if (controls.isEmpty()) return super.onKeyDown(key, event);
        if (key == KeyEvent.KEYCODE_TAB || key == KeyEvent.KEYCODE_DPAD_LEFT || key == KeyEvent.KEYCODE_DPAD_UP
            || key == KeyEvent.KEYCODE_DPAD_RIGHT || key == KeyEvent.KEYCODE_DPAD_DOWN) {
            int index = -1;
            for (int i = 0; i < controls.size(); i++) if (controls.get(i).id == keyFocus) index = i;
            boolean previous = event.isShiftPressed() || key == KeyEvent.KEYCODE_DPAD_LEFT || key == KeyEvent.KEYCODE_DPAD_UP;
            int start = index < 0 && previous ? 0 : index;
            int direction = previous ? -1 : 1;
            for (int step = 1; step <= controls.size(); step++) {
                Control next = controls.get(Math.floorMod(start + direction * step, controls.size()));
                if (next.readOnly) continue;
                keyFocus = next.id;
                surfaceDirty = true;
                invalidate();
                return true;
            }
            return super.onKeyDown(key, event);
        }
        if ((key == KeyEvent.KEYCODE_ENTER || key == KeyEvent.KEYCODE_DPAD_CENTER || key == KeyEvent.KEYCODE_SPACE)
            && keyFocus != NO_FOCUS) {
            if (event.getRepeatCount() == 0) activate(keyFocus);
            return true;
        }
        return super.onKeyDown(key, event);
    }

    private void activate(int id) {
        Control item = control(id);
        if (item == null || item.readOnly) return;
        host.markTouch();
        host.send("activate", fields("id", id));
    }

    @Override public boolean dispatchHoverEvent(MotionEvent event) {
        if (accessibility == null || !accessibility.isTouchExplorationEnabled()) return super.dispatchHoverEvent(event);
        int target = NO_FOCUS;
        if (event.getActionMasked() != MotionEvent.ACTION_HOVER_EXIT) {
            for (Control control : controls) {
                if (control.bounds.contains(event.getX() / density(), event.getY() / density())) target = control.id;
            }
        }
        if (target != hovered) {
            sendVirtualEvent(hovered, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);
            hovered = target;
            sendVirtualEvent(hovered, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER);
        }
        return target != NO_FOCUS || super.dispatchHoverEvent(event);
    }

    private int hovered = NO_FOCUS;

    private Control control(int id) {
        for (Control item : controls) if (item.id == id) return item;
        return null;
    }

    private void sendVirtualEvent(int id, int type) {
        Control item = control(id);
        if (item == null || accessibility == null || !accessibility.isEnabled() || getParent() == null) return;
        AccessibilityEvent event = AccessibilityEvent.obtain(type);
        event.setSource(this, id);
        event.setEnabled(true);
        event.setPackageName(getContext().getPackageName());
        event.setClassName(item.readOnly ? "android.widget.TextView" : "android.widget.Button");
        event.setContentDescription(item.label);
        getParent().requestSendAccessibilityEvent(this, event);
    }

    @Override public AccessibilityNodeProvider getAccessibilityNodeProvider() { return nodeProvider; }

    private final AccessibilityNodeProvider nodeProvider = new AccessibilityNodeProvider() {
        @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
            if (id == View.NO_ID) {
                AccessibilityNodeInfo hostNode = AccessibilityNodeInfo.obtain(GameView.this);
                onInitializeAccessibilityNodeInfo(hostNode);
                for (Control control : controls) hostNode.addChild(GameView.this, control.id);
                return hostNode;
            }
            Control item = control(id);
            if (item == null) return null;
            AccessibilityNodeInfo node = AccessibilityNodeInfo.obtain(GameView.this, id);
            node.setSource(GameView.this, id);
            node.setParent(GameView.this);
            node.setPackageName(getContext().getPackageName());
            node.setClassName(item.readOnly ? "android.widget.TextView" : "android.widget.Button");
            node.setText(item.label);
            node.setEnabled(true);
            node.setClickable(!item.readOnly);
            node.setFocusable(!item.readOnly);
            node.setVisibleToUser(isShown());
            node.setAccessibilityFocused(id == accessibilityFocus);
            if (!item.readOnly) node.addAction(AccessibilityNodeInfo.ACTION_CLICK);
            node.addAction(id == accessibilityFocus
                ? AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS : AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
            float factor = density();
            Rect bounds = new Rect(Math.round((float) Math.floor(item.bounds.left) * factor),
                Math.round((float) Math.floor(item.bounds.top) * factor),
                Math.round((float) Math.ceil(item.bounds.right) * factor),
                Math.round((float) Math.ceil(item.bounds.bottom) * factor));
            node.setBoundsInParent(bounds);
            int[] location = new int[2];
            getLocationOnScreen(location);
            bounds.offset(location[0], location[1]);
            node.setBoundsInScreen(bounds);
            return node;
        }

        @Override public AccessibilityNodeInfo findFocus(int focus) {
            int id = focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY ? accessibilityFocus : keyFocus;
            return id == NO_FOCUS ? null : createAccessibilityNodeInfo(id);
        }

        @Override public boolean performAction(int id, int action, Bundle options) {
            if (id == View.NO_ID) return GameView.this.performAccessibilityAction(action, options);
            Control item = control(id);
            if (item == null) return false;
            switch (action) {
                case AccessibilityNodeInfo.ACTION_CLICK:
                    if (item.readOnly) return false;
                    sendVirtualEvent(id, AccessibilityEvent.TYPE_VIEW_CLICKED);
                    activate(id);
                    return true;
                case AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS:
                    if (accessibilityFocus == id) return false;
                    sendVirtualEvent(accessibilityFocus, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                    accessibilityFocus = id;
                    sendVirtualEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED);
                    return true;
                case AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS:
                    if (accessibilityFocus != id) return false;
                    accessibilityFocus = NO_FOCUS;
                    sendVirtualEvent(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                    return true;
                default: return false;
            }
        }
    };

    private static final class Control {
        final int id;
        final String label;
        final RectF bounds;
        final boolean readOnly;

        Control(JSONObject item) {
            id = item.optInt("id");
            label = item.optString("label");
            readOnly = item.optBoolean("readOnly");
            float x = decimal(item, "x"), y = decimal(item, "y");
            bounds = new RectF(x, y, x + decimal(item, "w"), y + decimal(item, "h"));
        }
    }
}
