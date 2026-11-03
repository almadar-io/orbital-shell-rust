package io.almadar.nativegpu;

import android.app.NativeActivity;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;

public final class MainActivity extends NativeActivity {
    static {
        System.loadLibrary("orbital_client_dioxus");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        ViewGroup parent = (ViewGroup) getWindow().getDecorView();
        parent = (ViewGroup) parent.getChildAt(0);
        parent = (ViewGroup) parent.getChildAt(1);
        View nativeActivityView = parent.getChildAt(0);
        nativeActivityView.setFocusable(true);
        nativeActivityView.setFocusableInTouchMode(true);
        nativeActivityView.requestFocus();
    }
}
