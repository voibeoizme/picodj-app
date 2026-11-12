package __PACKAGE__

// PicoDJ for Android.
// The UI (ui/index.html) runs in the WebView like on Windows. This activity adds what the WebView cannot do:
//  - USB MIDI from the DJ controller over OTG (android.media.midi), forwarded to the page as window.__pdjMidi([status, d1, d2]),
//    and MIDI out for LED feedback (PicoMidi.send from JavaScript);
//  - asks for permission to read music files, then tells the page to rescan (window.__pdjRescan);
//  - full-screen landscape with the screen kept on.

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.midi.MidiDevice
import android.media.midi.MidiDeviceInfo
import android.media.midi.MidiInputPort
import android.media.midi.MidiManager
import android.media.midi.MidiOutputPort
import android.media.midi.MidiReceiver
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.WindowManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import java.util.concurrent.ConcurrentHashMap

class MainActivity : TauriActivity() {
  private var web: WebView? = null
  private var midi: PicoMidi? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    hideBars()
    val perm = if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE
    if (checkSelfPermission(perm) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(perm)) { js("window.__pdjRescan&&window.__pdjRescan()") }
    }
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) hideBars()
  }

  private fun hideBars() {
    WindowCompat.setDecorFitsSystemWindows(window, false)
    val c = WindowInsetsControllerCompat(window, window.decorView)
    c.hide(WindowInsetsCompat.Type.systemBars())
    c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
  }

  override fun onWebViewCreate(webView: WebView) {
    web = webView
    // let Web Audio start without waiting for a tap
    webView.settings.mediaPlaybackRequiresUserGesture = false
    val m = midi ?: PicoMidi(this) { js(it) }
    midi = m
    webView.addJavascriptInterface(m, "PicoMidi")
  }

  fun js(code: String) {
    val w = web ?: return
    w.post { w.evaluateJavascript(code, null) }
  }
}

class PicoMidi(ctx: Context, private val js: (String) -> Unit) {
  private val mm = ctx.getSystemService(Context.MIDI_SERVICE) as MidiManager?
  private val handler = Handler(Looper.getMainLooper())
  private val open = ConcurrentHashMap<Int, Conn>()

  class Conn(val dev: MidiDevice, val name: String) {
    val outs = mutableListOf<MidiOutputPort>()
    val ins = mutableListOf<MidiInputPort>()
  }

  init {
    mm?.let { m ->
      @Suppress("DEPRECATION")
      m.devices.forEach { openDev(it) }
      @Suppress("DEPRECATION")
      m.registerDeviceCallback(object : MidiManager.DeviceCallback() {
        override fun onDeviceAdded(info: MidiDeviceInfo) { openDev(info) }
        override fun onDeviceRemoved(info: MidiDeviceInfo) { close(info.id) }
      }, handler)
    }
  }

  private fun nameOf(info: MidiDeviceInfo): String {
    val b = info.properties
    return b.getString(MidiDeviceInfo.PROPERTY_PRODUCT) ?: b.getString(MidiDeviceInfo.PROPERTY_NAME) ?: "MIDI ${info.id}"
  }

  private fun openDev(info: MidiDeviceInfo) {
    if (open.containsKey(info.id) || mm == null) return
    val name = nameOf(info)
    mm.openDevice(info, { dev ->
      if (dev == null) return@openDevice
      val c = Conn(dev, name)
      for (p in info.ports) {
        if (p.type == MidiDeviceInfo.PortInfo.TYPE_OUTPUT) {   // the controller's output = what we receive
          dev.openOutputPort(p.portNumber)?.let { op -> op.connect(Rx()); c.outs.add(op) }
        } else {
          dev.openInputPort(p.portNumber)?.let { c.ins.add(it) }
        }
      }
      open[info.id] = c
    }, handler)
  }

  private fun close(id: Int) {
    val c = open.remove(id) ?: return
    c.outs.forEach { try { it.close() } catch (_: Exception) {} }
    c.ins.forEach { try { it.close() } catch (_: Exception) {} }
    try { c.dev.close() } catch (_: Exception) {}
  }

  // splits the byte stream into short messages (running status supported, sysex and realtime ignored)
  inner class Rx : MidiReceiver() {
    private var status = 0
    private val buf = IntArray(3)
    private var n = 0
    private var need = 3
    override fun onSend(msg: ByteArray, offset: Int, count: Int, timestamp: Long) {
      for (i in offset until offset + count) {
        val b = msg[i].toInt() and 0xFF
        if (b >= 0xF8) continue
        if (b >= 0x80) {
          if (b >= 0xF0) { status = 0; n = 0; continue }
          status = b; buf[0] = b; n = 1
          need = if ((b and 0xF0) == 0xC0 || (b and 0xF0) == 0xD0) 2 else 3
          continue
        }
        if (status == 0) continue
        if (n == 0) { buf[0] = status; n = 1 }
        buf[n++] = b
        if (n >= need) {
          js(if (need == 2) "window.__pdjMidi&&window.__pdjMidi([${buf[0]},${buf[1]}])" else "window.__pdjMidi&&window.__pdjMidi([${buf[0]},${buf[1]},${buf[2]}])")
          n = 0
        }
      }
    }
  }

  @JavascriptInterface
  fun devices(): String = open.values.filter { it.outs.isNotEmpty() }.joinToString(",", "[", "]") { "\"" + it.name.replace("\"", "'").replace("\\", "/") + "\"" }

  @JavascriptInterface
  fun send(a: Int, b: Int, c: Int) {
    val d = byteArrayOf(a.toByte(), b.toByte(), c.toByte())
    for (cn in open.values) for (p in cn.ins) try { p.send(d, 0, 3) } catch (_: Exception) {}
  }
}
