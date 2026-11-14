package com.pocketterm.android

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import java.io.File
import java.util.concurrent.Executors

class MainActivity : Activity() {
    companion object {
        init { System.loadLibrary("pocketterm_jni") }
        @JvmStatic external fun executeCommand(filesDir: String, currentDir: String, command: String): Array<String>
        @JvmStatic external fun cancelCommand(): Boolean
    }

    private val worker = Executors.newSingleThreadExecutor()
    private val history = mutableListOf<String>()
    private var historyIndex = 0
    private lateinit var output: TextView
    private lateinit var input: EditText
    private lateinit var runButton: Button
    private var currentDirectory = ""
    private var isRunning = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        currentDirectory = filesDir.absolutePath
        preparePrivateDirectories()
        output = TextView(this).apply {
            setTextColor(Color.BLACK)
            setBackgroundColor(Color.WHITE)
            textSize = 14f
            typeface = android.graphics.Typeface.MONOSPACE
            text = "PocketTerm\n"
            setPadding(12, 12, 12, 12)
        }
        val scroll = ScrollView(this).apply {
            setBackgroundColor(Color.WHITE)
            addView(output)
        }
        input = EditText(this).apply {
            setTextColor(Color.BLACK)
            setHintTextColor(Color.DKGRAY)
            setSingleLine(true)
            imeOptions = EditorInfo.IME_ACTION_DONE
            typeface = android.graphics.Typeface.MONOSPACE
            setOnEditorActionListener { _, actionId, event ->
                if (actionId == EditorInfo.IME_ACTION_DONE || event?.keyCode == KeyEvent.KEYCODE_ENTER) {
                    submitCommand()
                    true
                } else false
            }
            setOnKeyListener { _, keyCode, event ->
                if (event.action != KeyEvent.ACTION_DOWN) return@setOnKeyListener false
                when (keyCode) {
                    KeyEvent.KEYCODE_DPAD_UP -> { showHistory(-1); true }
                    KeyEvent.KEYCODE_DPAD_DOWN -> { showHistory(1); true }
                    else -> false
                }
            }
        }
        runButton = Button(this).apply {
            text = "Run"
            setTextColor(Color.BLACK)
            setBackgroundColor(Color.WHITE)
            setOnClickListener {
                if (isRunning) {
                    text = "Stopping..."
                    cancelCommand()
                } else submitCommand()
            }
        }
        val prompt = TextView(this).apply {
            text = "pocket@android:~$ "
            setTextColor(Color.BLACK)
            setBackgroundColor(Color.WHITE)
            typeface = android.graphics.Typeface.MONOSPACE
            gravity = Gravity.CENTER_VERTICAL
        }
        val commandBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(Color.WHITE)
            addView(prompt)
            addView(input, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            addView(runButton)
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.WHITE)
            addView(scroll, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(commandBar)
        }
        setContentView(root)
        input.requestFocus()
    }

    private fun preparePrivateDirectories() {
        val prefix = File(filesDir, "usr")
        listOf("bin", "lib", "etc", "share", "var", "tmp").forEach { File(prefix, it).mkdirs() }
        val bundledShim = File(applicationInfo.nativeLibraryDir, "libpt-exec.so")
        if (bundledShim.isFile) {
            runCatching { bundledShim.copyTo(File(prefix, "lib/libpt-exec.so"), overwrite = true) }
        }
    }

    private fun submitCommand() {
        val command = input.text.toString().trim()
        if (command.isEmpty()) return
        input.text.clear()
        history.add(command)
        historyIndex = history.size
        append("pocket@android:~$ $command\n")
        input.isEnabled = false
        isRunning = true
        runButton.text = "Stop"
        runButton.isEnabled = true
        append("Running...\n")
        worker.execute {
            val result = runCatching { executeCommand(filesDir.absolutePath, currentDirectory, command) }
            runOnUiThread {
                removeRunningMarker()
                result.onSuccess { fields ->
                    if (fields.size == 4) {
                        currentDirectory = fields[1]
                        append(fields[2])
                        append(fields[3])
                        if (fields[0] != "0") append("[exit ${fields[0]}]\n")
                    } else append("terminal: invalid response from native core\n")
                }.onFailure { error -> append("terminal: ${error.message ?: "command failed"}\n") }
                isRunning = false
                input.isEnabled = true
                runButton.text = "Run"
                runButton.isEnabled = true
                input.requestFocus()
            }
        }
    }

    private fun append(text: String) {
        output.append(text)
        val parent = output.parent
        if (parent is ScrollView) parent.post { parent.fullScroll(View.FOCUS_DOWN) }
    }

    private fun removeRunningMarker() {
        val text = output.text.toString()
        val marker = "Running...\n"
        if (text.endsWith(marker)) output.text = text.dropLast(marker.length)
    }

    private fun showHistory(offset: Int) {
        if (history.isEmpty()) return
        historyIndex = (historyIndex + offset).coerceIn(0, history.size)
        input.setText(history.getOrNull(historyIndex).orEmpty())
        input.setSelection(input.text.length)
    }

    override fun onDestroy() {
        if (isRunning) cancelCommand()
        worker.shutdownNow()
        super.onDestroy()
    }
}
