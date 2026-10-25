package com.cruiseliner.channelcast

import java.net.URI
import java.util.regex.Pattern

/**
 * Turn a raw publisher HTML document into a dark, leanback-friendly reader page.
 *
 * TV WebView often cannot load news-site CSS/JS (ERR_TIMED_OUT on subresources),
 * so injecting the full document yields a black/malformed shell. Extracting the
 * article body and wrapping it in our own CSS keeps Open article readable.
 */
object ArticleReader {

    private val SCRIPT_STYLE: Pattern = Pattern.compile(
        "(?is)<(script|style|noscript|iframe|svg|template)[^>]*>.*?</\\1>",
    )
    private val COMMENT: Pattern = Pattern.compile("(?is)<!--.*?-->")
    private val TAG: Pattern = Pattern.compile("(?is)<[^>]+>")

    data class ReaderDoc(val title: String, val html: String, val bodyChars: Int)

    fun toReaderHtml(rawHtml: String, pageUrl: String, fallbackTitle: String): ReaderDoc {
        val title = extractTitle(rawHtml)?.takeIf { it.isNotBlank() }
            ?: fallbackTitle.ifBlank { "Article" }
        val bodyHtml = extractBodyHtml(rawHtml, pageUrl)
        val plainLen = stripTags(bodyHtml).replace(Regex("\\s+"), " ").trim().length
        val doc = """
            <!DOCTYPE html>
            <html><head>
            <meta charset="utf-8"/>
            <meta name="viewport" content="width=device-width, initial-scale=1"/>
            <style>
              html,body{margin:0;padding:0;background:#12141a;color:#e8eaed;
                font-family:Georgia,"Times New Roman",serif;line-height:1.65;}
              .wrap{max-width:820px;margin:0 auto;padding:28px 36px 64px;}
              .kicker{font-family:sans-serif;font-size:0.85rem;color:#9aa0a6;margin:0 0 8px;}
              h1{font-family:sans-serif;font-size:1.85rem;line-height:1.25;margin:0 0 20px;color:#fff;}
              h2,h3{font-family:sans-serif;color:#fff;margin:1.4em 0 0.5em;}
              p,li{font-size:1.15rem;margin:0 0 1em;color:#e8eaed;}
              a{color:#8ab4f8;}
              img{max-width:100%;height:auto;border-radius:6px;margin:0.5em 0 1em;background:#1a1f2a;}
              blockquote{border-left:4px solid #5f6368;margin:1em 0;padding:0.2em 0 0.2em 1em;color:#c5c7ca;}
              figcaption,.caption{font-family:sans-serif;font-size:0.9rem;color:#9aa0a6;margin:-0.5em 0 1.2em;}
              .src{font-family:sans-serif;font-size:0.9rem;color:#9aa0a6;margin-top:2.5em;
                padding-top:1em;border-top:1px solid #2a2f3a;word-break:break-all;}
            </style>
            </head><body><div class="wrap">
            <p class="kicker">ChannelCast reader</p>
            <h1>${escape(title)}</h1>
            $bodyHtml
            <p class="src">Original: <a href="${escape(pageUrl)}">${escape(pageUrl)}</a></p>
            </div></body></html>
        """.trimIndent()
        return ReaderDoc(title = title, html = doc, bodyChars = plainLen)
    }

    fun isUseful(doc: ReaderDoc): Boolean = doc.bodyChars >= 180

    private fun extractTitle(html: String): String? {
        ogMeta(html, "og:title")?.let { return decode(it) }
        matchGroup(html, "(?is)<title[^>]*>(.*?)</title>")?.let {
            return decode(stripTags(it).trim())
        }
        matchGroup(html, "(?is)<h1[^>]*>(.*?)</h1>")?.let {
            return decode(stripTags(it).trim())
        }
        return null
    }

    private fun extractBodyHtml(html: String, pageUrl: String): String {
        val cleaned = SCRIPT_STYLE.matcher(COMMENT.matcher(html).replaceAll("")).replaceAll("")
        val candidates = ArrayList<String>()
        // Balanced containers first (nested </div> breaks naive regex).
        balancedInner(cleaned, "id", "storytext")?.let { candidates.add(it) }
        balancedInnerByClass(cleaned, "paragraphs-container")?.let { candidates.add(it) }
        balancedInnerByClass(cleaned, "storytext")?.let { candidates.add(it) }
        balancedInnerByClass(cleaned, "gnt_ar_b")?.let { candidates.add(it) }
        balancedInnerByClass(cleaned, "story-body")?.let { candidates.add(it) }

        val patterns = listOf(
            "(?is)<[^>]+itemprop=[\"']articleBody[\"'][^>]*>(.*?)</(?:div|section|article)>",
            "(?is)<article[^>]*>(.*?)</article>",
            "(?is)<main[^>]*>(.*?)</main>",
        )
        for (pat in patterns) {
            val m = Pattern.compile(pat).matcher(cleaned)
            var best: String? = null
            var bestLen = 0
            while (m.find()) {
                val chunk = m.group(1) ?: continue
                val len = stripTags(chunk).length
                if (len > bestLen) {
                    bestLen = len
                    best = chunk
                }
            }
            if (best != null && bestLen > 120) candidates.add(best)
        }
        // Last resort: harvest all substantial paragraphs from the whole doc.
        if (candidates.none { stripTags(it).length > 400 }) {
            val allPs = harvestParagraphs(cleaned)
            if (allPs.length > 200) candidates.add(allPs)
        }
        val best = candidates.maxByOrNull { stripTags(it).length } ?: cleaned
        return sanitizeFragment(best, pageUrl)
    }

    private fun sanitizeFragment(fragment: String, pageUrl: String): String {
        var s = fragment
        for (tag in listOf("nav", "aside", "footer", "header", "form", "button")) {
            s = Pattern.compile("(?is)<$tag\\b[^>]*>.*?</$tag>").matcher(s).replaceAll("")
        }
        val out = StringBuilder()
        val block = Pattern.compile(
            "(?is)<(p|h2|h3|h4|li|blockquote|figcaption)(\\s[^>]*)?>(.*?)</\\1>|<(img)\\b([^>]*?)/?>",
        )
        val m = block.matcher(s)
        var found = false
        while (m.find()) {
            found = true
            val tag = (m.group(1) ?: m.group(4) ?: "").lowercase()
            if (tag == "img") {
                val attrs = m.group(5).orEmpty()
                val src = attr(attrs, "src") ?: attr(attrs, "data-src") ?: continue
                val abs = absolutize(pageUrl, src) ?: continue
                if (abs.startsWith("data:")) continue
                val alt = attr(attrs, "alt").orEmpty()
                out.append("<p><img src=\"").append(escape(abs)).append("\" alt=\"")
                    .append(escape(alt)).append("\"/></p>\n")
            } else {
                val inner = m.group(3).orEmpty()
                val texty = decode(stripTags(inner)).trim()
                if (texty.length < 2) continue
                if (texty.equals("skip to main content", ignoreCase = true) ||
                    texty.contains("keyboard shortcuts", ignoreCase = true) ||
                    texty.equals("open navigation menu", ignoreCase = true) ||
                    texty.equals("npr logo", ignoreCase = true) ||
                    texty.equals("newsletters", ignoreCase = true)
                ) {
                    continue
                }
                val safeInner = rewriteInline(inner, pageUrl)
                when (tag) {
                    "li" -> out.append("<ul><li>").append(safeInner).append("</li></ul>\n")
                    else -> out.append("<").append(tag).append(">").append(safeInner)
                        .append("</").append(tag).append(">\n")
                }
            }
        }
        if (!found || stripTags(out.toString()).length < 120) {
            val plain = decode(stripTags(s)).replace("\r", "")
            val paras = plain.split(Regex("\n{2,}"))
                .map { it.replace(Regex("\\s+"), " ").trim() }
                .filter { it.length >= 40 }
                .filterNot {
                    it.contains("skip to main content", ignoreCase = true) ||
                        it.contains("keyboard shortcuts", ignoreCase = true)
                }
            out.setLength(0)
            for (p in paras.take(40)) {
                out.append("<p>").append(escape(p)).append("</p>\n")
            }
        }
        return out.toString()
    }

    private fun rewriteInline(inner: String, pageUrl: String): String {
        val sb = StringBuilder()
        val re = Pattern.compile("(?is)<a\\b([^>]*)>(.*?)</a>|([^<]+)|(<[^>]+>)")
        val m = re.matcher(inner)
        while (m.find()) {
            when {
                m.group(1) != null -> {
                    val href = attr(m.group(1), "href")
                    val text = escape(decode(stripTags(m.group(2))).trim())
                    if (href.isNullOrBlank()) {
                        sb.append(text)
                    } else {
                        val abs = absolutize(pageUrl, href) ?: href
                        sb.append("<a href=\"").append(escape(abs)).append("\">")
                            .append(text).append("</a>")
                    }
                }
                m.group(3) != null -> sb.append(escape(decode(m.group(3))))
                else -> {
                    val t = m.group(4).orEmpty().lowercase()
                    if (t.startsWith("<br")) sb.append(" ")
                    // drop other tags
                }
            }
        }
        return sb.toString().replace(Regex("\\s+"), " ").trim()
    }


    /** Find inner HTML of a balanced <div id="…">…</div>. */
    private fun balancedInner(html: String, attr: String, value: String): String? {
        val open = Pattern.compile(
            """(?is)<div\b[^>]*\b""" + attr + """\s*=\s*["']""" + value + """["'][^>]*>""",
        ).matcher(html)
        if (!open.find()) return null
        return balanceFrom(html, open.end())
    }

    private fun balancedInnerByClass(html: String, className: String): String? {
        val open = Pattern.compile(
            """(?is)<div\b[^>]*\bclass\s*=\s*["'][^"']*\b""" +
                Pattern.quote(className) +
                """\b[^"']*["'][^>]*>""",
        ).matcher(html)
        if (!open.find()) return null
        return balanceFrom(html, open.end())
    }

    private fun balanceFrom(html: String, start: Int): String? {
        var depth = 1
        val either = Pattern.compile("""(?is)<div\b[^>]*>|</div\s*>""")
        val m = either.matcher(html)
        m.region(start, html.length)
        while (m.find()) {
            val tok = m.group()
            if (tok.startsWith("</")) {
                depth--
                if (depth == 0) {
                    val inner = html.substring(start, m.start())
                    return if (stripTags(inner).length > 120) inner else null
                }
            } else if (!tok.endsWith("/>")) {
                depth++
            }
        }
        return null
    }

    private fun harvestParagraphs(html: String): String {
        val sb = StringBuilder()
        val m = Pattern.compile("""(?is)<p\b[^>]*>(.*?)</p>""").matcher(html)
        while (m.find()) {
            val inner = m.group(1) ?: continue
            val text = decode(stripTags(inner)).trim()
            if (text.length < 40) continue
            if (text.contains("skip to main content", ignoreCase = true)) continue
            sb.append("<p>").append(inner).append("</p>\n")
        }
        return sb.toString()
    }

    private fun attr(attrs: String, name: String): String? {
        val m = Pattern.compile(
            "(?is)\\b$name\\s*=\\s*([\"'])(.*?)\\1|\\b$name\\s*=\\s*([^\\s>]+)",
        ).matcher(attrs)
        if (!m.find()) return null
        return (m.group(2) ?: m.group(3))?.trim()
    }

    private fun absolutize(base: String, ref: String): String? {
        val r = ref.trim()
        if (r.isEmpty() || r.startsWith("#") || r.startsWith("javascript:", ignoreCase = true)) {
            return null
        }
        return try {
            URI(base).resolve(r).toString()
        } catch (_: Exception) {
            null
        }
    }

    private fun ogMeta(html: String, prop: String): String? {
        matchGroup(
            html,
            "(?is)<meta[^>]+property=[\"']$prop[\"'][^>]+content=[\"'](.*?)[\"']",
        )?.let { return it }
        return matchGroup(
            html,
            "(?is)<meta[^>]+content=[\"'](.*?)[\"'][^>]+property=[\"']$prop[\"']",
        )
    }

    private fun matchGroup(html: String, pattern: String): String? {
        val m = Pattern.compile(pattern).matcher(html)
        return if (m.find()) m.group(1) else null
    }

    private fun stripTags(s: String): String = TAG.matcher(s).replaceAll(" ")

    private fun decode(s: String): String =
        s.replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&#x27;", "'")
            .replace("&mdash;", "—")
            .replace("&ndash;", "–")
            .replace("&rsquo;", "’")
            .replace("&lsquo;", "‘")
            .replace("&rdquo;", "”")
            .replace("&ldquo;", "“")

    private fun escape(s: String): String =
        s.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
}
