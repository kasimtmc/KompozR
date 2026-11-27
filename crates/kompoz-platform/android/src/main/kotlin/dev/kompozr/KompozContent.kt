package dev.kompozr

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL

/**
 * What a KompozR app reads from the network and other apps (C++'s KompozInput images): an http(s) URL's body
 * through HttpURLConnection, a `content://` URI's bytes through the ContentResolver, and the pictures our decoders
 * lack (HEIF, AVIF, GIF's first frame) decoded by BitmapFactory. Called from the app's worker threads (the image
 * loader's), attached to the VM; the app's context is the application's, kept as the first activity is created.
 */
internal object KompozContent {
    @Volatile
    @JvmStatic
    var context: Context? = null

    /** The body of an http(s) GET (redirects followed, 15 s to connect, 20 s between reads), or null. */
    @JvmStatic
    fun httpGet(url: ByteArray): ByteArray? {
        val address = url.toString(Charsets.UTF_8)
        var connection: HttpURLConnection? = null
        return try {
            connection = URL(address).openConnection() as HttpURLConnection
            connection.connectTimeout = 15_000
            connection.readTimeout = 20_000
            connection.instanceFollowRedirects = true
            if (connection.responseCode / 100 != 2) {
                Log.w("kompoz", "$address: HTTP ${connection.responseCode}")
                null
            } else {
                connection.inputStream.use { readAll(it) }
            }
        } catch (e: Exception) { // (plain http:// needs the app's usesCleartextTraffic)
            Log.w("kompoz", "can't load $address: $e")
            null
        } finally {
            connection?.disconnect()
        }
    }

    /** A `content://` URI's bytes from its provider, or null. */
    @JvmStatic
    fun readContent(uri: ByteArray): ByteArray? {
        val c = context ?: return null
        val text = uri.toString(Charsets.UTF_8)
        return try {
            c.contentResolver.openInputStream(Uri.parse(text))?.use { readAll(it) }
        } catch (e: Exception) {
            Log.w("kompoz", "can't read $text: $e")
            null
        }
    }

    /** A URI's content as `mimeType` (its provider's own type, or one it can turn it into), or null. */
    fun readTyped(context: Context, uri: Uri, mimeType: String): ByteArray? = try {
        val resolver = context.contentResolver
        val type = resolver.getType(uri)
        if (type != null && android.content.ClipDescription.compareMimeTypes(type, mimeType)) {
            resolver.openInputStream(uri)?.use { readAll(it) }
        } else {
            resolver.openTypedAssetFileDescriptor(uri, mimeType, null)?.use { d -> d.createInputStream().use { readAll(it) } }
        }
    } catch (e: Exception) {
        null
    }

    /**
     * A picture's bytes decoded by BitmapFactory, scaled down by a power of two as far as the size asked for
     * allows (`width` by `height` px, 0 for the whole; `fill`: covering that box rather than fitting in it):
     * `[width, height]` then the ARGB pixels, or null.
     */
    @JvmStatic
    fun decodeImage(data: ByteArray, width: Int, height: Int, fill: Boolean): IntArray? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        var sample = 1
        if (width > 0 && height > 0) {
            // How much larger the picture is than it needs to be: to cover the box, the lesser; to fit, the greater.
            val x = bounds.outWidth.toDouble() / width
            val y = bounds.outHeight.toDouble() / height
            val larger = if (fill) minOf(x, y) else maxOf(x, y)
            while (sample * 2 <= larger) sample *= 2
        }
        val options = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }
        val bitmap = BitmapFactory.decodeByteArray(data, 0, data.size, options) ?: return null
        val w = bitmap.width
        val h = bitmap.height
        val out = IntArray(2 + w * h)
        out[0] = w
        out[1] = h
        bitmap.getPixels(out, 2, w, 0, 0, w, h)
        bitmap.recycle()
        return out
    }

    private fun readAll(input: InputStream): ByteArray {
        val out = ByteArrayOutputStream()
        val buffer = ByteArray(16384)
        while (true) {
            val n = input.read(buffer)
            if (n <= 0) break
            out.write(buffer, 0, n)
        }
        return out.toByteArray()
    }
}
