package com.cereware.finances

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // Puente JS → Android para instalar actualizaciones y alimentar el widget.
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(CwfBridge(this), "CwfAndroid")
  }
}
