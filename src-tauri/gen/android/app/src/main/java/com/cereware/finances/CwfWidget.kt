package com.cereware.finances

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews

/** Widget de escritorio: saldo total y gasto del mes (resumen guardado al abrir la app). */
class CwfWidget : AppWidgetProvider() {
  override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
    for (id in ids) manager.updateAppWidget(id, views(context))
  }

  companion object {
    const val PREFS = "cwf_widget"

    fun refreshAll(context: Context) {
      val manager = AppWidgetManager.getInstance(context)
      val ids = manager.getAppWidgetIds(ComponentName(context, CwfWidget::class.java))
      for (id in ids) manager.updateAppWidget(id, views(context))
    }

    private fun views(context: Context): RemoteViews {
      val p = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
      val v = RemoteViews(context.packageName, R.layout.cwf_widget)
      val balance = p.getString("balance", null)
      v.setTextViewText(R.id.w_balance, balance ?: "Abre la app")
      v.setTextViewText(R.id.w_spent, if (balance == null) "para ver tu resumen" else "Gastado en ${p.getString("label", "")}: ${p.getString("spent", "")}")
      v.setTextViewText(R.id.w_updated, p.getString("updated", "") ?: "")
      val open = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      v.setOnClickPendingIntent(R.id.w_root, PendingIntent.getActivity(context, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
      return v
    }
  }
}
