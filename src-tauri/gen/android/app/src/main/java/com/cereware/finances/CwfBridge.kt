package com.cereware.finances

import android.app.Activity
import android.content.Intent
import android.webkit.JavascriptInterface
import androidx.core.content.FileProvider
import java.io.File

class CwfBridge(private val activity: Activity) {
  /** Abre el instalador de Android con un APK descargado en la caché de la app. */
  @JavascriptInterface
  fun installApk(path: String): Boolean {
    val file = File(path)
    if (!file.exists() || file.canonicalPath.startsWith(activity.cacheDir.canonicalPath).not()) return false
    val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
    val intent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    activity.runOnUiThread { activity.startActivity(intent) }
    return true
  }

  /** Guarda el resumen que muestra el widget (texto ya formateado) y lo refresca. */
  @JavascriptInterface
  fun saveWidget(balance: String, spent: String, label: String, updated: String) {
    activity.getSharedPreferences(CwfWidget.PREFS, Activity.MODE_PRIVATE).edit()
      .putString("balance", balance).putString("spent", spent).putString("label", label).putString("updated", updated)
      .apply()
    CwfWidget.refreshAll(activity)
  }

  /** Borra el resumen (al desactivar el widget o cerrar sesión). */
  @JavascriptInterface
  fun clearWidget() {
    activity.getSharedPreferences(CwfWidget.PREFS, Activity.MODE_PRIVATE).edit().clear().apply()
    CwfWidget.refreshAll(activity)
  }
}
