//! (stage 14 WP0) The C++ Kompoz's Kotlin-syntax tests of classes (tests/kompozable_tests.cpp `data_classes`,
//! `objects_with_members`, `companion_objects`, `type_aliases`, `nested_declarations`, `objects_and_spans`, and a
//! Rust-modules analogue of `kotlin_in_cpp_namespaces`), ported by name: the same Kotlin through `kompoz!` and
//! `kompoz_file!`, the same lines expected. Kotlin's declarations of another block or file of the crate are seen as
//! a package's (the module table), the classes Rust calls as Rust types.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

// ---- data_classes ------------------------------------------------------------------------------------------------

kompoz! {
    data class Note(val id: Int, val title: String, val pinned: Boolean = false, val tags: List<String> = listOf())

    @Kompozable
    fun NoteRow(note: Note) {
        Text("${note.id}: ${note.title}${if (note.pinned) " *" else ""}")
    }
}

kompoz! {
    // (another block's data class: the module table's)
    @Kompozable
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
}

fn data_classes() {
    let rule = KompozTestRule::new(Notes);
    rule.on_node_with_text("1: alışveriş").assert_exists();
    rule.on_node_with_text("1: iş *").assert_exists();
    rule.on_node_with_tag("str").assert_text_equals(&["Note(id=1, title=iş, pinned=true, tags=[])"]);
    rule.on_node_with_text("eşit").assert_exists();
    // (Rust calls the Kotlin data class: its constructor, fields, components, copy)
    let n = Note(3, "üç");
    assert_eq!((n.id, n.title.as_str(), n.pinned, n.component2()), (3, "üç", false, "üç".to_string()));
    assert_eq!(named_args!(n.copy(pinned = true)).to_string(), "Note(id=3, title=üç, pinned=true, tags=[])");
}

// ---- objects_with_members ----------------------------------------------------------------------------------------

kompoz! {
    // Kotlin's objects with members (an app's singletons): vals (const too), a var, a private one, functions with a
    // default, an init block that runs first; Kotlin's String + any value.
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

    @Kompozable
    fun ObjectForms() {
        var shown by remember { mutableStateOf(Tally.describe()) }
        Column {
            Text(shown)
            Button(onClick = {
                Tally.add()
                shown = Tally.describe() + " / adım " + Tally.Step + " / " + 1.5f
            }) { Text("Ekle") }
        }
    }
}

fn objects_with_members() {
    let rule = KompozTestRule::new(ObjectForms);
    rule.wait_for_idle();
    rule.on_node_with_text("toplam 10").assert_exists();
    rule.on_node_with_text("Ekle").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("toplam 12 / adım 2 / 1.5").assert_exists();
}

// ---- companion_objects -------------------------------------------------------------------------------------------

kompoz! {
    // companion object: a class's, a data class's and an interface's static members: a const val, a var, a value of
    // the class itself, functions; read by their names inside the class, as Name.member outside.
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

    data class Money(val cents: Int) {
        companion object {
            val Zero = Money(0)
            fun of(lira: Int) = Money(lira * 100)
        }
    }

    interface Shade {
        val tone: String

        companion object {
            const val Default = "açık"
        }
    }

    @Kompozable
    fun CompanionForms() {
        val first = remember { Ledger.create() }
        val second = remember { Ledger.create() }
        Text(first.label() + " " + second.label() + " " + Ledger.made + " " + Ledger.PREFIX + " " + Money.of(3) + " " + Money.Zero + " " + Shade.Default)
    }
}

fn companion_objects() {
    let rule = KompozTestRule::new(CompanionForms);
    rule.wait_for_idle();
    rule.on_node_with(kompoz::test::has_text("hesap-1", true, false), false)
        .assert_text_equals(&["hesap-1 hesap-2 2 hesap- Money(cents=300) Money(cents=0) açık"]);
}

// ---- type_aliases ------------------------------------------------------------------------------------------------

kompoz! {
    data class Receipt(val item: String, val price: Int, val count: Int = 1)

    // typealias: of a list, of function types (a lambda given one that gives a value gives it), a generic one and one
    // of it, one naming a data class.
    typealias Names = List<String>
    typealias Pick = (String) -> Unit
    typealias Format = (Int) -> String
    typealias Cells<T> = List<List<T>>
    typealias Board = Cells<Int>
    typealias Stock = List<Receipt>

    fun countNames(names: Names) = names.size
    fun formatAll(xs: List<Int>, f: Format): Names = xs.map { f(it) }
    fun total(grid: Cells<Int>) = grid.sumOf { it.sum() }

    @Kompozable
    fun AliasForms(onPick: Pick = {}) {
        val names: Names = listOf("a", "b")
        val board: Board = listOf(listOf(1, 2), listOf(3))
        val stock: Stock = listOf(Receipt("kalem", 30))
        Text("ad " + countNames(names) + " " + formatAll(listOf(1, 2)) { "#$it" } + " " + total(board) + " " + stock.size)
    }
}

fn type_aliases() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| AliasForms()));
    rule.wait_for_idle();
    rule.on_node_with(kompoz::test::has_text("ad ", true, false), false).assert_text_equals(&["ad 2 [#1, #2] 6 1"]);
    // (Rust names the aliases)
    let names: Names = vec!["x".into()];
    assert_eq!(count_names(names), 1);
}

// ---- nested_declarations -----------------------------------------------------------------------------------------

kompoz! {
    // Nested declarations: a data class, enums (entries alone, and with a constructor), an object, an interface and
    // a class inside a class, named alone inside it and with it outside (Catalog.Entry, Catalog.Kind.FILM).
    class Catalog {
        data class Entry(val title: String, val kind: Kind = Kind.BOOK)
        enum class Kind { BOOK, FILM }
        enum class Tier(val price: Int) { FREE(0), PAID(5) }
        object Defaults {
            const val limit = 2
        }
        interface Filter {
            fun keep(e: Entry): Boolean
        }
        class Shelf(val entries: List<Entry>) {
            fun count() = entries.size
        }

        private val items = listOf(Entry("a"), Entry("b", Kind.FILM), Entry("c"))
        fun films(): List<Entry> = items.filter { it.kind == Kind.FILM }
        fun shelf() = Shelf(items.take(Defaults.limit))
    }

    @Kompozable
    fun NestedForms() {
        val catalog = remember { Catalog() }
        val entry = Catalog.Entry("d", Catalog.Kind.FILM)
        val books = object : Catalog.Filter {
            override fun keep(e: Catalog.Entry) = e.kind == Catalog.Kind.BOOK
        }
        Text("iç " + catalog.films().size + " " + catalog.shelf().count() + " " + entry + " " + Catalog.Tier.PAID.price + " " + Catalog.Kind.entries.size +
            " " + books.keep(entry) + " " + Catalog.Defaults.limit)
    }
}

fn nested_declarations() {
    let rule = KompozTestRule::new(NestedForms);
    rule.wait_for_idle();
    rule.on_node_with(kompoz::test::has_text("iç", true, false), false)
        .assert_text_equals(&["iç 1 2 Entry(title=d, kind=FILM) 5 2 false 2"]);
}

// ---- objects_and_spans -------------------------------------------------------------------------------------------

kompoz! {
    // An object expression, object : NestedScrollConnection { override fun … }: the locals its functions use are
    // held in it. A lazy grid's span = { GridItemSpan(maxLineSpan) }.
    fun counting(counter: MutableState<Float>) = object : NestedScrollConnection {
        override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
            counter.value += available.y
            return Offset(0f, available.y / 2)
        }
    }

    @Kompozable
    fun SpannedGrid() {
        LazyVerticalGrid(columns = GridCells.Fixed(3), modifier = Modifier.width(300.dp).height(100.dp)) {
            item(span = { GridItemSpan(maxLineSpan) }) { Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag("header")) }
            items(listOf("a", "b"), span = { name -> GridItemSpan(if (name == "a") 2 else 1) }) { name ->
                Box(modifier = Modifier.fillMaxWidth().height(20.dp).testTag(name))
            }
        }
    }
}

fn objects_and_spans() {
    let counter = mutable_state_of(0.0_f32);
    let connection = counting(counter.clone());
    let used = connection.on_pre_scroll(Offset::new(0.0, 10.0), NestedScrollSource::UserInput);
    assert!(counter.get() == 10.0 && used.y == 5.0, "{} {used:?}", counter.get());
    let rule = KompozTestRule::new(SpannedGrid);
    let header = rule.on_node_with_tag("header").fetch_semantics_node().size();
    let a = rule.on_node_with_tag("a").fetch_semantics_node().size();
    assert!((header.width - 300).abs() <= 1 && (a.width - 200).abs() <= 1, "{header:?} {a:?}");
}

// ---- kotlin_in_cpp_namespaces: Kotlin in Rust modules --------------------------------------------------------------

/// Kotlin and Rust in a Rust module, the module's Rust calling its Kotlin (C++'s namespaces).
mod shop {
    use kompoz::prelude::*;

    /// A Rust enum stays Rust (beside Kotlin's enum class).
    #[derive(Clone, Copy, PartialEq, Debug)]
    #[allow(dead_code)]
    pub enum Aisle {
        Fruit,
        Bread,
        Milk,
    }

    kompoz! {
        enum class Rack { Top, Bottom }

        data class Basket(val items: Int, val rack: Rack = Rack.Top)

        fun basketLabel(basket: Basket): String = "${basket.items} ürün, ${basket.rack}"
    }

    pub fn shop_label() -> String {
        basket_label(Basket { items: 2, rack: Rack::Bottom }) + " / " + &(Aisle::Milk as i32).to_string()
    }
}

/// Another module's Kotlin using the first's declarations (the module table's, brought in by a `use`).
mod shelf {
    use super::shop::*;
    use kompoz::prelude::*;

    kompoz! {
        fun shelfLabel(): String = basketLabel(Basket(3)) + " " + Rack.entries.size + " " + Rack.valueOf("Bottom").ordinal
    }
}

fn kotlin_in_cpp_namespaces() {
    assert_eq!(shop::shop_label(), "2 ürün, Bottom / 2");
    assert_eq!(shelf::shelf_label(), "3 ürün, Top 2 1");
}

// ---- (stage 14 WPD) value classes and annotation classes -------------------------------------------------------

kompoz! {
    // (an annotation of the app's own: declared, used, nothing to write)
    @Target(AnnotationTarget.FUNCTION)
    annotation class Measured(val unit: String = "m")

    @JvmInline
    value class Meters(val value: Int) {
        operator fun plus(other: Meters) = Meters(value + other.value)
        fun doubled() = Meters(value * 2)
    }

    @Measured
    fun walked(steps: List<Meters>): String {
        var total = Meters(0)
        for (s in steps) total = total + s
        return "$total ${total.doubled().value} ${total == Meters(5)}"
    }
}

fn value_and_annotation_classes() {
    assert_eq!(walked(vec![Meters(2), Meters(3)]), "Meters(value=5) 10 true");
}

fn main() {
    common::run(&[
        ("value_and_annotation_classes", value_and_annotation_classes),
        ("data_classes", data_classes),
        ("objects_with_members", objects_with_members),
        ("companion_objects", companion_objects),
        ("type_aliases", type_aliases),
        ("nested_declarations", nested_declarations),
        ("objects_and_spans", objects_and_spans),
        ("kotlin_in_cpp_namespaces", kotlin_in_cpp_namespaces),
    ]);
}
