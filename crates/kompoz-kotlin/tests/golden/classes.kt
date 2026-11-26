// Stage 14 WP0: data classes, plain classes, objects, companions, enum classes, nested declarations, type aliases.

data class Note(val id: Int, val title: String, val pinned: Boolean = false, val tags: List<String> = listOf())

@Composable
fun NoteRow(note: Note) {
    Text("${note.id}: ${note.title}${if (note.pinned) " *" else ""}")
}

@Composable
fun Notes() {
    val a = Note(title = "alışveriş", id = 1)
    val b = a.copy(pinned = true, title = "iş")
    Column {
        NoteRow(a)
        NoteRow(note = b)
        Text("$b", modifier = Modifier.testTag("str"))
        Text(if (a == b.copy(pinned = false, title = "alışveriş")) "eşit" else "farklı")
    }
}

object Tally {
    const val Step = 2
    var total = 0
    private val label = "toplam"
    fun add(n: Int = Step): Int {
        total += n
        return total
    }
    fun describe() = label + " " + total
    init {
        total = 10
    }
}

class Ledger(val id: Int) {
    fun label() = "$PREFIX$id"

    companion object {
        const val PREFIX = "hesap-"
        var made = 0
        fun create(): Ledger {
            made++
            return Ledger(made)
        }
    }
}

enum class Rack { Top, Bottom }

enum class Tier(val price: Int, val label: String = "tier") { FREE(0), PAID(price = 5, label = "paid") }

typealias Names = List<String>
typealias Cells<T> = List<List<T>>

fun total(grid: Cells<Int>) = grid.sumOf { it.sum() }

fun countNames(names: Names) = names.size

class Catalog {
    data class Entry(val title: String, val kind: Kind = Kind.BOOK)
    enum class Kind { BOOK, FILM }
    object Defaults {
        const val limit = 2
    }

    private val items = listOf(Entry("a"), Entry("b", Kind.FILM), Entry("c"))
    fun films(): List<Entry> = items.filter { it.kind == Kind.FILM }
}

fun uses(): String {
    val e = Catalog.Entry("d", Catalog.Kind.FILM)
    return "" + Tally.add() + Tally.describe() + Ledger.create().label() + Ledger.made + Rack.Bottom + Tier.PAID.price +
        Catalog.Defaults.limit + e + Catalog().films().size + Rack.valueOf("Top").ordinal + Tier.entries.size
}

// (stage 14 WPD) local classes: beside their function, of its name (two functions' of one name apart)
@Composable
fun LocalItems() {
    data class Item(val id: Int, val label: String = "")
    class Counter {
        var count = 0
    }

    val items = listOf(Item(0, "a"), Item(1))
    val counter = remember { Counter() }
    Text("${items.first()} ${items.last().label} ${counter.count}")
}

fun localSizes(): Int {
    data class Item(val id: Int, val size: Int)

    val (id, size) = Item(2, 3)
    return id + size
}

// (stage 14 WPD) an annotation class (nothing written) and a value class (a data class's struct of its one property)
annotation class Marker

@JvmInline
value class Meters(val value: Int)

@Marker
fun twice(m: Meters): Meters = Meters(m.value * 2)
