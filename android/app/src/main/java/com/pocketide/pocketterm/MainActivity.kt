package com.pocketide.pocketterm

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import java.io.FileNotFoundException
import android.webkit.WebView
import android.webkit.WebViewClient

class MainActivity : Activity() {
    private lateinit var webView: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.allowFileAccess = false
            settings.allowContentAccess = false
            settings.allowFileAccessFromFileURLs = false
            settings.allowUniversalAccessFromFileURLs = false
            webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(view: WebView?, request: WebResourceRequest?): WebResourceResponse? {
                    val uri = request?.url ?: return null
                    if (uri.scheme != "https" || uri.host != "appassets.androidplatform.net" || !uri.path.orEmpty().startsWith("/assets/")) return null
                    val asset = uri.path!!.removePrefix("/assets/")
                    val mime = when (asset.substringAfterLast('.', "")) {
                        "html" -> "text/html"
                        "css" -> "text/css"
                        "js" -> "text/javascript"
                        else -> "application/octet-stream"
                    }
                    return try {
                        WebResourceResponse(mime, "UTF-8", assets.open(asset))
                    } catch (_: FileNotFoundException) {
                        WebResourceResponse("text/plain", "UTF-8", 404, "Not Found", emptyMap(), "")
                    }
                }
                override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
                    val uri = request?.url ?: return true
                    return uri.scheme != "https" || uri.host != "appassets.androidplatform.net"
                }
            }
            webChromeClient = WebChromeClient()
            addJavascriptInterface(TerminalBridge(), "PocketTerm")
            loadUrl("https://appassets.androidplatform.net/assets/index.html")
        }
        setContentView(webView)
    }

    inner class TerminalBridge {
        @JavascriptInterface fun startServer() {
            startForegroundService(Intent(this@MainActivity, TerminalService::class.java).setAction(TerminalService.ACTION_START))
        }
        @JavascriptInterface fun stopServer() {
            startService(Intent(this@MainActivity, TerminalService::class.java).setAction(TerminalService.ACTION_STOP))
        }
    }
}
