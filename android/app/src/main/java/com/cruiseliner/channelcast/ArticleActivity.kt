package com.cruiseliner.channelcast

import android.annotation.SuppressLint
import android.graphics.Color as AndroidColor
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.Charset
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.zip.GZIPInputStream

/**
 * In-app article reader for devices without a browser (typical Android TV).
 *
 * Chromium WebView on some TVs hits ERR_TIMED_OUT / "Webpage not available" for
 * major news sites even when the app's own HTTP stack (RSS) works. Strategy:
 * 1) Try WebView.loadUrl (best for JS-heavy pages).
 * 2) On main-frame error, prefetch HTML via HttpURLConnection (same stack as feeds)
 *    and inject with loadDataWithBaseURL so the document still renders.
 * 3) Prefer prefetch-first on known-flaky hosts when loadUrl already failed once.
 */
class ArticleActivity : ComponentActivity() {
    private var webView: WebView? = null
    private val titleState = mutableStateOf("Article")
    private var triedPrefetch = false
    private var loadUrlFailed = false
    private val io = Executors.newFixedThreadPool(2)
    private val fetchIo = Executors.newCachedThreadPool()

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val url = intent.getStringExtra(EXTRA_URL).orEmpty().trim()
        val titleExtra = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        if (url.isBlank() || !(url.startsWith("https://") || url.startsWith("http://"))) {
            finish()
            return
        }
        // Never open HN comments as the article — caller should pass the story URL.
        if (isHnCommentsUrl(url)) {
            Toast.makeText(this, "No story link for this item", Toast.LENGTH_LONG).show()
            finish()
            return
        }
        if (titleExtra.isNotBlank()) {
            titleState.value = titleExtra
        }

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    val wv = webView
                    if (wv != null && wv.canGoBack()) {
                        wv.goBack()
                    } else {
                        finish()
                    }
                }
            },
        )

        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(modifier = Modifier.fillMaxSize()) {
                    val pageTitle by titleState
                    var progress by remember { mutableFloatStateOf(0f) }

                    Column(Modifier.fillMaxSize().background(Color(0xFF0E1014))) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(Color(0xFF1A1F2A))
                                .padding(horizontal = 16.dp, vertical = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                text = "← Back",
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier
                                    .clickable { finish() }
                                    .padding(end = 16.dp),
                            )
                            Text(
                                text = pageTitle,
                                style = MaterialTheme.typography.titleMedium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f),
                            )
                        }
                        if (progress in 0.01f..0.99f) {
                            LinearProgressIndicator(
                                progress = { progress },
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }
                        AndroidView(
                            factory = { ctx ->
                                WebView(ctx).apply {
                                    webView = this
                                    setBackgroundColor(AndroidColor.BLACK)
                                    layoutParams = FrameLayout.LayoutParams(
                                        ViewGroup.LayoutParams.MATCH_PARENT,
                                        ViewGroup.LayoutParams.MATCH_PARENT,
                                    )
                                    CookieManager.getInstance().setAcceptCookie(true)
                                    CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)
                                    settings.apply {
                                        javaScriptEnabled = true
                                        domStorageEnabled = true
                                        databaseEnabled = true
                                        loadsImagesAutomatically = true
                                        mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
                                        useWideViewPort = true
                                        loadWithOverviewMode = true
                                        builtInZoomControls = true
                                        displayZoomControls = false
                                        mediaPlaybackRequiresUserGesture = true
                                        // Desktop Chrome UA — many publishers block "TV" / bare WebView UAs.
                                        userAgentString = DESKTOP_UA
                                        if (android.os.Build.VERSION.SDK_INT >= 26) {
                                            safeBrowsingEnabled = false
                                        }
                                    }
                                    webViewClient = object : WebViewClient() {
                                        override fun shouldOverrideUrlLoading(
                                            view: WebView,
                                            request: WebResourceRequest,
                                        ): Boolean {
                                            val next = request.url?.toString().orEmpty()
                                            return !(next.startsWith("https://") || next.startsWith("http://"))
                                        }

                                        override fun onPageFinished(view: WebView?, finishedUrl: String?) {
                                            super.onPageFinished(view, finishedUrl)
                                            // If Chromium painted its error page, try prefetch once.
                                            val title = view?.title.orEmpty().lowercase()
                                            if (!triedPrefetch && (
                                                    title.contains("webpage not available") ||
                                                        title.contains("err_") ||
                                                        title.contains("not available")
                                                    )
                                            ) {
                                                prefetchAndInject(url)
                                            }
                                        }

                                        override fun onReceivedError(
                                            view: WebView?,
                                            request: WebResourceRequest?,
                                            error: WebResourceError?,
                                        ) {
                                            if (request?.isForMainFrame != true) return
                                            loadUrlFailed = true
                                            val code = error?.errorCode ?: 0
                                            val desc = error?.description?.toString().orEmpty()
                                            android.util.Log.w(
                                                "ChannelCast",
                                                "WebView main-frame error code=$code desc=$desc url=$url",
                                            )
                                            if (!triedPrefetch) {
                                                prefetchAndInject(url)
                                            } else {
                                                Toast.makeText(
                                                    this@ArticleActivity,
                                                    "Could not load article" +
                                                        if (desc.isNotBlank()) ": $desc" else " (timeout)",
                                                    Toast.LENGTH_LONG,
                                                ).show()
                                            }
                                        }
                                    }
                                    webChromeClient = object : WebChromeClient() {
                                        override fun onProgressChanged(view: WebView?, newProgress: Int) {
                                            progress = newProgress / 100f
                                        }

                                        override fun onReceivedTitle(view: WebView?, title: String?) {
                                            if (!title.isNullOrBlank() &&
                                                !title.equals("Webpage not available", ignoreCase = true) &&
                                                !title.equals("ChannelCast reader", ignoreCase = true)
                                            ) {
                                                titleState.value = title
                                            }
                                        }
                                    }
                                    // TV Chromium often ERR_TIMED_OUT on news hosts even when
                                    // app HTTP works — always prefetch the main document first.
                                    prefetchAndInject(url, alsoTryLoadUrlOnFail = true)
                                }
                            },
                            modifier = Modifier.fillMaxSize().weight(1f),
                        )
                    }
                }
            }
        }
    }

    private fun prefetchAndInject(url: String, alsoTryLoadUrlOnFail: Boolean = false) {
        if (triedPrefetch && !alsoTryLoadUrlOnFail) return
        triedPrefetch = true
        val loadingTitle = intent.getStringExtra(EXTRA_TITLE).orEmpty().ifBlank { "Article" }
        val earlySummary = intent.getStringExtra(EXTRA_SUMMARY).orEmpty().trim()
        val hasUsableSummary = earlySummary.length >= 40
        // Always paint feed blurb (or title) immediately — never leave a blank WebView.
        webView?.loadDataWithBaseURL(
            url,
            if (earlySummary.isNotBlank()) {
                summaryReaderHtml(loadingTitle, earlySummary, url)
            } else {
                """<!DOCTYPE html><html><head><meta charset="utf-8"/>
                <style>body{margin:0;background:#12141a;color:#e8eaed;font-family:sans-serif;padding:36px;line-height:1.5}
                h1{color:#fff;font-size:1.6rem}</style></head>
                <body><h1>${loadingTitle}</h1><p>Loading article…</p>
                <p style="color:#9aa0a6;word-break:break-all">$url</p></body></html>"""
            },
            "text/html",
            "UTF-8",
            null,
        )
        // When the feed already has a blurb (HN meta, Ars, Fightful, …), stay on the
        // summary reader — skip flaky publisher prefetch that times out on TV.
        if (hasUsableSummary) {
            android.util.Log.i("ChannelCast", "Summary-only reader (${earlySummary.length} chars); skip prefetch")
            return
        }
        io.execute {
            val fetchUrl = readerFetchUrl(url)
            android.util.Log.i("ChannelCast", "Prefetch fetchUrl=$fetchUrl")
            val result = runCatching {
                val fut = fetchIo.submit<Pair<String, String>> { fetchHtml(fetchUrl) }
                try {
                    fut.get(14, TimeUnit.SECONDS)
                } catch (te: TimeoutException) {
                    fut.cancel(true)
                    error("prefetch timed out after 14s")
                }
            }.onFailure { android.util.Log.w("ChannelCast", "Prefetch exception", it) }
            android.util.Log.i(
                "ChannelCast",
                "Prefetch done ok=${result.isSuccess} err=${result.exceptionOrNull()?.message}",
            )
            runOnUiThread {
                val wv = webView
                if (wv == null) {
                    android.util.Log.e("ChannelCast", "Prefetch: webView null on UI thread")
                    return@runOnUiThread
                }
                result.fold(
                    onSuccess = { (finalUrl, html) ->
                        android.util.Log.i(
                            "ChannelCast",
                            "Prefetch OK ${html.length} bytes base=$finalUrl",
                        )
                        val fallbackTitle = intent.getStringExtra(EXTRA_TITLE).orEmpty()
                        val reader = ArticleReader.toReaderHtml(html, finalUrl, fallbackTitle)
                        android.util.Log.i(
                            "ChannelCast",
                            "Reader bodyChars=${reader.bodyChars} title=${reader.title.take(80)}",
                        )
                        titleState.value = reader.title
                        // Always show extracted reader HTML on TV. Falling back to
                        // loadUrl() causes ERR_TIMED_OUT on many publishers even when
                        // our HTTP prefetch already succeeded.
                        if (!ArticleReader.isUseful(reader)) {
                            android.util.Log.w(
                                "ChannelCast",
                                "Reader thin (${reader.bodyChars} chars); still injecting reader HTML",
                            )
                        }
                        wv.loadDataWithBaseURL(
                            finalUrl,
                            reader.html,
                            "text/html",
                            "UTF-8",
                            finalUrl,
                        )
                    },
                    onFailure = { err ->
                        android.util.Log.w("ChannelCast", "Prefetch failed: ${err.message}")
                        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty().ifBlank { "Article" }
                        val summary = intent.getStringExtra(EXTRA_SUMMARY).orEmpty().trim()
                        if (summary.isNotBlank()) {
                            android.util.Log.i("ChannelCast", "Using feed summary fallback (${summary.length} chars)")
                            titleState.value = title
                            wv.loadDataWithBaseURL(
                                url,
                                summaryReaderHtml(title, summary, url),
                                "text/html",
                                "UTF-8",
                                null,
                            )
                        } else {
                            // Keep the title page already painted; no error toast loop.
                            android.util.Log.w("ChannelCast", "No summary + prefetch failed: ${err.message}")
                        }
                    },
                )
            }
        }
    }

    override fun onDestroy() {
        io.shutdownNow()
        fetchIo.shutdownNow()
        webView?.apply {
            stopLoading()
            destroy()
        }
        webView = null
        super.onDestroy()
    }

    companion object {
        const val EXTRA_URL = "url"
        const val EXTRA_TITLE = "title"
        const val EXTRA_SUMMARY = "summary"

        fun summaryReaderHtml(title: String, summary: String, url: String): String {
            val safeTitle = title.ifBlank { "Article" }
            val paras = summary
                .split(Regex("""\n{2,}"""))
                .map { it.replace(Regex("""\s+"""), " ").trim() }
                .filter { it.length >= 20 }
            val body = if (paras.isEmpty()) {
                "<p>${summary.replace("<", "&lt;")}</p>"
            } else {
                paras.joinToString("\n") { "<p>${it.replace("&", "&amp;").replace("<", "&lt;")}</p>" }
            }
            return """
                <!DOCTYPE html><html><head><meta charset="utf-8"/>
                <style>
                  body{margin:0;background:#12141a;color:#e8eaed;font-family:Georgia,serif;line-height:1.65}
                  .wrap{max-width:820px;margin:0 auto;padding:28px 36px 64px}
                  .kicker{font-family:sans-serif;font-size:0.85rem;color:#9aa0a6}
                  h1{font-family:sans-serif;font-size:1.85rem;color:#fff;line-height:1.25}
                  p{font-size:1.15rem;margin:0 0 1em}
                  a{color:#8ab4f8}
                  .src{font-family:sans-serif;font-size:0.9rem;color:#9aa0a6;margin-top:2em;
                    padding-top:1em;border-top:1px solid #2a2f3a;word-break:break-all}
                </style></head><body><div class="wrap">
                <p class="kicker">ChannelCast reader</p>
                <h1>$safeTitle</h1>
                $body
                <p class="src">Original: <a href="$url">$url</a></p>
                </div></body></html>
            """.trimIndent()
        }

        private const val DESKTOP_UA =
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) " +
                "Chrome/120.0.0.0 Safari/537.36"

        fun isHnCommentsUrl(url: String): Boolean {
            val lower = url.lowercase()
            return (lower.contains("news.ycombinator.com/item") ||
                lower.contains("news.ycombinator.com/reply")) &&
                (lower.contains("id=") || lower.contains("/item?"))
        }

        fun shouldPrefetchFirst(url: String): Boolean {
            val host = runCatching { URL(url).host.lowercase() }.getOrDefault("")
            return host.contains("bbc.") ||
                host.contains("bbci.") ||
                host.endsWith("npr.org") ||
                host.contains("usatoday.com") ||
                host.contains("makeuseof.com") ||
                host.contains("nytimes.com") ||
                host.contains("washingtonpost.com") ||
                host.contains("theguardian.com") ||
                host.contains("cnn.com") ||
                host.contains("reuters.com")
        }


        /**
         * Prefer a lean HTML source when the publisher's full site is JS/CSS-heavy.
         * NPR's text.npr.org is a complete article body without the broken desktop shell.
         */
        fun readerFetchUrl(url: String): String {
            val host = runCatching { URL(url).host.lowercase() }.getOrDefault("")
            if (host == "text.npr.org") return url
            if (host == "www.npr.org" || host == "npr.org") {
                // /YYYY/MM/DD/{id}/slug  or  /nx-s1-… /g-s1-…
                val path = runCatching { URL(url).path }.getOrDefault("")
                val idMatch = Regex("""/(?:(\d{4})/(\d{2})/(\d{2})/)?(\d{5,}|[a-z]+-s\d+-\d+)""", RegexOption.IGNORE_CASE)
                    .find(path)
                val id = idMatch?.groupValues?.getOrNull(4)
                if (!id.isNullOrBlank()) {
                    return "https://text.npr.org/$id"
                }
            }
            return url
        }

        /** App-HTTP-stack fetch (bypasses Chromium net for the main document). */
        fun fetchHtml(url: String, redirectLimit: Int = 8): Pair<String, String> {
            var current = url
            var hops = 0
            while (hops < redirectLimit) {
                hops++
                val conn = (URL(current).openConnection() as HttpURLConnection).apply {
                    instanceFollowRedirects = false
                    connectTimeout = 8_000
                    readTimeout = 12_000
                    requestMethod = "GET"
                    setRequestProperty("User-Agent", DESKTOP_UA)
                    setRequestProperty(
                        "Accept",
                        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                    )
                    setRequestProperty("Accept-Language", "en-US,en;q=0.9")
                    setRequestProperty("Accept-Encoding", "gzip")
                    setRequestProperty("Connection", "close")
                }
                val code = try {
                    conn.responseCode
                } catch (e: Exception) {
                    conn.disconnect()
                    throw e
                }
                if (code in 300..399) {
                    val loc = conn.getHeaderField("Location")
                        ?: run {
                            conn.disconnect()
                            error("Redirect $code without Location")
                        }
                    current = URL(URL(current), loc).toString()
                    conn.disconnect()
                    continue
                }
                try {
                    if (code !in 200..299) {
                        error("HTTP $code")
                    }
                    var stream = (conn.errorStream ?: conn.inputStream) ?: error("empty body")
                    val enc = conn.contentEncoding?.lowercase().orEmpty()
                    if (enc.contains("gzip")) {
                        stream = GZIPInputStream(stream)
                    }
                    val charset = charsetFromContentType(conn.contentType)
                    val html = BufferedReader(InputStreamReader(stream, charset)).use { it.readText() }
                    if (html.isBlank()) error("empty HTML")
                    return current to html
                } finally {
                    conn.disconnect()
                }
            }
            error("Too many redirects")
        }

        private fun charsetFromContentType(ct: String?): Charset {
            if (ct == null) return Charsets.UTF_8
            val m = Regex("charset=([\\w\\-]+)", RegexOption.IGNORE_CASE).find(ct)
            return try {
                Charset.forName(m?.groupValues?.get(1) ?: "UTF-8")
            } catch (_: Exception) {
                Charsets.UTF_8
            }
        }
    }
}
