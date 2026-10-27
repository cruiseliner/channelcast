package com.cruiseliner.channelcast

import android.content.Context
import android.util.Log
import java.io.File

/**
 * Optional Florida Today subscriber Cookie for full-text enrich.
 *
 * Rust [floridatoday::resolve_cookie] reads
 * `/data/data/<package>/files/floridatoday.cookies` when present.
 * Titles/blurbs work without it; the jar only unlocks fuller article bodies.
 *
 * Install (debuggable APK):
 * ```
 * # After exporting Netscape cookies.txt or a raw Cookie header to a file:
 * adb push floridatoday.cookies /sdcard/Download/floridatoday.cookies
 * adb shell run-as com.cruiseliner.channelcast.tv.debug \
 *   cp /sdcard/Download/floridatoday.cookies files/floridatoday.cookies
 * ```
 * Phone debug package: `com.cruiseliner.channelcast.debug`
 *
 * Also copies from app-external `Download/floridatoday.cookies` into filesDir
 * on cold start when the private copy is missing (user dropped the file there).
 */
object SubscriberCookies {
    private const val TAG = "ChannelCast"
    const val FILE_NAME = "floridatoday.cookies"

    fun ensurePrivateCopy(context: Context) {
        val dest = File(context.filesDir, FILE_NAME)
        if (dest.exists() && dest.length() > 0L) {
            Log.i(TAG, "Florida Today cookie present (${dest.length()} bytes)")
            return
        }
        val candidates = listOfNotNull(
            context.getExternalFilesDir(null)?.let { File(it, FILE_NAME) },
            context.getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS)
                ?.let { File(it, FILE_NAME) },
            File("/sdcard/Download/$FILE_NAME"),
            File("/storage/emulated/0/Download/$FILE_NAME"),
        )
        for (src in candidates) {
            if (!src.exists() || src.length() == 0L) continue
            try {
                src.copyTo(dest, overwrite = true)
                Log.i(TAG, "Copied Florida Today cookie from ${src.path} (${dest.length()} bytes)")
                return
            } catch (e: Exception) {
                Log.w(TAG, "Cookie copy failed from ${src.path}: ${e.message}")
            }
        }
        Log.i(TAG, "No Florida Today cookie — listing titles only (see docs/ANDROID.md)")
    }
}
