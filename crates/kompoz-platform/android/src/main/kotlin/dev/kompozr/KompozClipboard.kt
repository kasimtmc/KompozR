package dev.kompozr

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.PersistableBundle
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The clipboard for a KompozR app (C++'s KompozActivity clipboard, Compose's AndroidClipboard): what it holds
 * described without being read ([description]: its ClipDescription — its MIME types, whether it is sensitive, its
 * time stamp — which Android doesn't tell the user about), read as the app asks for a type ([read]: that is a paste,
 * which Android tells the user of, as it should), and the app's entries put there ([set]): text, HTML and URIs as
 * ClipData's items have them, the app's own types in the item's Intent (an item has no place for other bytes),
 * a sensitive one marked so (`ClipDescription.EXTRA_IS_SENSITIVE`: keyboards and the clipboard's preview hide it).
 *
 * Clips cross as one byte array each (kompoz-platform's android::clip, little-endian); the reading of a clip and
 * the making of one are KompozDragAndDrop's too.
 *
 * Its methods `description`, `read` and `set` are the app's to call (by name, from the native side, on this thread).
 */
internal class KompozClipboard(private val context: Context) {
    private val clipboard get() = context.getSystemService(ClipboardManager::class.java)

    /** What the clipboard holds, described (null: nothing). */
    @Suppress("unused") // (the app's)
    fun description(): ByteArray? {
        val c = clipboard ?: return null
        if (!c.hasPrimaryClip()) return null
        return describe(c.primaryClipDescription ?: return null)
    }

    /** Its content as `mimeType` if it is still the clip described at `timestamp` (null: not that type, or gone). */
    @Suppress("unused") // (the app's)
    fun read(mimeType: ByteArray, timestamp: Long): ByteArray? {
        val c = clipboard ?: return null
        val description = c.primaryClipDescription ?: return null
        if (description.timestamp != timestamp) return null
        return read(context, c.primaryClip ?: return null, mimeType.toString(Charsets.UTF_8))
    }

    /** The app's entry put there (null or no items: the clipboard emptied). */
    @Suppress("unused") // (the app's)
    fun set(entry: ByteArray?) {
        val c = clipboard ?: return
        val clip = entry?.let { clipData(it) }
        if (clip != null) c.setPrimaryClip(clip) else c.clearPrimaryClip()
    }

    companion object {
        /** The action of an item's Intent carrying the app's own types (each a byte array extra, named by its type). */
        const val TYPES_ACTION = "dev.kompozr.CLIP_TYPES"

        private const val URI_LIST = "text/uri-list"

        /** `ClipDescription.EXTRA_IS_SENSITIVE` (named in Android 13; keyboards and previews honour it before). */
        private const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"

        /** A ClipDescription as the app reads it: sensitive, its MIME types, its time stamp. */
        fun describe(description: ClipDescription): ByteArray {
            val out = Out()
            val sensitive = description.extras?.getBoolean(EXTRA_IS_SENSITIVE, false) ?: false
            out.byte(if (sensitive) 1 else 0)
            out.int(description.mimeTypeCount)
            for (i in 0 until description.mimeTypeCount) out.bytes(description.getMimeType(i).toByteArray())
            out.long(description.timestamp)
            return out.toByteArray()
        }

        /**
         * A clip's content as `mimeType`: text (its first item's, coerced as Android does), HTML, the URIs of all its
         * items (a uri-list), one of the app's own types (from an item's Intent), else the first item whose URI's
         * content is of that type, read from its provider. Null if none.
         */
        fun read(context: Context, clip: ClipData, mimeType: String): ByteArray? {
            if (clip.itemCount == 0) return null
            val first = clip.getItemAt(0)
            val essence = mimeType.substringBefore(';').trim().lowercase()
            when {
                essence == ClipDescription.MIMETYPE_TEXT_PLAIN || essence == "text/*" ->
                    return first.coerceToText(context)?.toString()?.toByteArray()
                essence == ClipDescription.MIMETYPE_TEXT_HTML ->
                    return first.htmlText?.toByteArray()
                essence == URI_LIST -> {
                    val uris = (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri?.toString() }
                    return if (uris.isEmpty()) null else uris.joinToString("\r\n", postfix = "\r\n").toByteArray()
                }
            }
            for (i in 0 until clip.itemCount) {
                val intent = clip.getItemAt(i).intent
                if (intent?.action == TYPES_ACTION) intent.getByteArrayExtra(mimeType)?.let { return it }
            }
            for (i in 0 until clip.itemCount) {
                val uri = clip.getItemAt(i).uri ?: continue
                KompozContent.readTyped(context, uri, mimeType)?.let { return it }
            }
            return null
        }

        /**
         * The app's entry as a ClipData (null: no items): each item's text, HTML and first URI an Android item's,
         * its other URIs items of their own, its other types in its Intent; described by the entry's types, marked
         * sensitive if it is.
         */
        fun clipData(entry: ByteArray): ClipData? {
            val input = In(entry)
            val sensitive = input.byte() != 0
            val types = Array(input.int()) { input.string() }
            val items = ArrayList<ClipData.Item>()
            repeat(input.int()) {
                var text: String? = null
                var html: String? = null
                val uris = ArrayList<Uri>()
                val own = HashMap<String, ByteArray>()
                repeat(input.int()) {
                    val mime = input.string()
                    val bytes = input.bytes()
                    when (mime.substringBefore(';').trim().lowercase()) {
                        ClipDescription.MIMETYPE_TEXT_PLAIN -> text = bytes.toString(Charsets.UTF_8)
                        ClipDescription.MIMETYPE_TEXT_HTML -> html = bytes.toString(Charsets.UTF_8)
                        URI_LIST -> bytes.toString(Charsets.UTF_8).lineSequence().map { it.trim() }
                            .filter { it.isNotEmpty() && !it.startsWith("#") }.forEach { uris += Uri.parse(it) }
                        else -> own[mime] = bytes
                    }
                }
                val intent = if (own.isEmpty()) null else Intent(TYPES_ACTION).also { i ->
                    own.forEach { (mime, bytes) -> i.putExtra(mime, bytes) }
                }
                items += ClipData.Item(text ?: html?.let { stripTags(it) }, html, intent, uris.firstOrNull())
                for (uri in uris.drop(1)) items += ClipData.Item(uri)
            }
            if (items.isEmpty()) return null
            val description = ClipDescription(null, types)
            if (sensitive) {
                description.extras = PersistableBundle().apply {
                    putBoolean(EXTRA_IS_SENSITIVE, true)
                }
            }
            val clip = ClipData(description, items[0])
            for (item in items.drop(1)) clip.addItem(item)
            return clip
        }

        /** HTML's text (an item with HTML must have text too). */
        private fun stripTags(html: String): String =
            android.text.Html.fromHtml(html, android.text.Html.FROM_HTML_MODE_LEGACY).toString()
    }

    /** Writes the wire's values (little-endian). */
    private class Out {
        private val out = ByteArrayOutputStream()
        fun byte(v: Int) = out.write(v)
        fun int(v: Int) = out.write(ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(v).array())
        fun long(v: Long) = out.write(ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(v).array())
        fun bytes(b: ByteArray) {
            int(b.size)
            out.write(b)
        }
        fun toByteArray(): ByteArray = out.toByteArray()
    }

    /** Reads them. */
    private class In(bytes: ByteArray) {
        private val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        fun byte(): Int = buffer.get().toInt()
        fun int(): Int = buffer.int
        fun bytes(): ByteArray = ByteArray(buffer.int).also { buffer.get(it) }
        fun string(): String = bytes().toString(Charsets.UTF_8)
    }
}
