//! (stage 14 WPA) The C++ Kompoz's Kotlin-syntax tests of class hierarchies (tests/kompozable_tests.cpp
//! `sealed_classes_and_is`, `interfaces_as_kotlin_writes_them`, `data_class_bodies`, `abstract_and_open_classes`,
//! `generic_classes_and_interfaces`, `enum_classes_with_bodies`, `inner_classes_hold_their_outer_object`,
//! `enum_entries_with_bodies_of_their_own`, `sealed_classes_with_constructors_and_members`,
//! `nested_sealed_and_generic_declarations`, `inner_classes_of_values`, `fun_interfaces_with_suspend_funs`,
//! `companion_objects_implementing_interfaces`, `sealed_classes_with_class_cases`,
//! `inner_classes_of_generic_data_classes_and_with_cpp_bases` — its C++ base a Rust one here — and the class parts of
//! `classes_as_kotlin_writes_them`), ported by name: the same Kotlin through `kompoz!`, the same lines expected (one
//! following Compose's state where the C++ Kompoz's differs: `interfaces_as_kotlin_writes_them`'s after `Seç`).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

/// The node whose text starts with `prefix`, its whole text checked.
fn line(rule: &KompozTestRule, prefix: &str, expected: &str) {
    rule.on_node_with(kompoz::test::has_text(prefix, true, false), false).assert_text_equals(&[expected]);
}

// ---- sealed_classes_and_is ---------------------------------------------------------------------------------------

kompoz! {
    // Kotlin's sealed classes and interfaces: nested subclasses (a data object, a data class) and the file's own; when
    // with is (the subject seen as the T: smart casts), if (x is T && …), as?, == and toString.
    sealed class LoadState {
        data object Loading : LoadState()
        data class Ready(val items: List<String>) : LoadState()
    }

    data class Broken(val message: String) : LoadState()

    // (another with subclasses of the same names: each its own)
    sealed class SaveState {
        data object Loading : SaveState()
        data class Ready(val count: Int) : SaveState()
    }

    sealed interface Figure

    data class Disc(val r: Float) : Figure

    object Speck : Figure

    @Kompozable
    fun LoadView(state: LoadState) {
        when (state) {
            is LoadState.Loading -> Text("yükleniyor")
            is LoadState.Ready -> Text("${state.items.size} öğe")
            is Broken -> Text("hata: " + state.message)
        }
    }

    fun describe(figure: Figure): String = when (figure) {
        is Disc -> "disk " + figure.r
        is Speck -> "nokta"
    }

    fun area(figure: Figure): Float = if (figure is Disc && figure.r > 0f) figure.r * figure.r else 0f

    @Kompozable
    fun SealedForms() {
        var state by remember { mutableStateOf<LoadState>(LoadState.Loading) }
        Column {
            LoadView(state)
            Text(describe(Disc(2f)) + " " + area(Speck) + " " + area(Disc(3f)) + " " + (state == LoadState.Loading) + " " + state)
            Text("kayıt " + SaveState.Ready(count = 3) + " " + SaveState.Loading)
            val broken = state as? Broken
            if (broken != null) Text("ayrıntı: " + broken.message)
            Button(onClick = { state = LoadState.Ready(listOf("a", "b")) }) { Text("Yükle") }
            Button(onClick = { state = Broken("yok") }) { Text("Boz") }
        }
    }
}

fn sealed_classes_and_is() {
    let rule = KompozTestRule::new(SealedForms);
    rule.wait_for_idle();
    rule.on_node_with_text("yükleniyor").assert_exists();
    line(&rule, "disk ", "disk 2.0 0.0 9.0 true Loading");
    rule.on_node_with_text("kayıt Ready(count=3) Loading").assert_exists();
    rule.on_node_with_text("Yükle").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("2 öğe").assert_exists();
    rule.on_node_with_text("disk 2.0 0.0 9.0 false Ready(items=[a, b])").assert_exists();
    rule.on_node_with_text("Boz").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("hata: yok").assert_exists();
    rule.on_node_with_text("ayrıntı: yok").assert_exists();
}

// ---- interfaces_as_kotlin_writes_them ----------------------------------------------------------------------------

kompoz! {
    // (kompozable_tests_other.cpp's: an interface of another file, implemented in this one by an object and a data
    // class)
    interface Emblem {
        val text: String
        fun shown() = "<$text>"
    }
}

kompoz! {
    // Kotlin's interfaces: abstract and default properties and functions, one extending another, implemented by
    // a class (a constructor's override val), a ViewModel, a data class, an object and an object expression; a
    // fun interface made from a lambda; is / as on them, == (a data class's: equal values).
    interface Titled {
        val name: String
        fun title(): String = "[$name]"
    }

    interface Shelf : Titled {
        val items: List<String>
        var picks: Int
        fun count(): Int
        fun describe() = title() + " " + count()
    }

    class FakeShelf(override val name: String = "sahte") : Shelf {
        override val items = listOf("a", "b")
        override var picks = 0
        override fun count() = items.size
    }

    object EmptyShelf : Shelf {
        override val name = "boş"
        override val items: List<String> = emptyList()
        override var picks = 0
        override fun count() = 0
    }

    data class Tag(override val name: String) : Titled

    class ShelfViewModel : ViewModel(), Titled {
        override val name: String get() = "raf"
        override fun title() = "<$name>"
    }

    object NewEmblem : Emblem {
        override val text = "yeni"
    }

    data class CountEmblem(override val text: String) : Emblem

    fun interface Check {
        fun test(text: String): Boolean
    }

    fun titleOf(named: Titled): String = named.title()

    fun kindOf(t: Titled): String = when (t) {
        is Tag -> "etiket " + t.name
        is FakeShelf -> "raf " + t.items.size + " " + (t as? Shelf)?.count()
        else -> "başka"
    }

    @Kompozable
    fun InterfaceForms() {
        val vm = viewModel<ShelfViewModel>()
        val nonEmpty = Check { it.isNotEmpty() }
        val anonymous = object : Titled { override val name = "adsız" }
        var shelf by remember { mutableStateOf<Shelf>(FakeShelf()) }
        Column {
            Text(shelf.describe())
            Text(titleOf(vm) + " " + titleOf(anonymous) + " " + titleOf(EmptyShelf) + " " + titleOf(Tag("etiket")) + " " + EmptyShelf.title())
            Text("geçerli " + nonEmpty.test("x") + " " + nonEmpty.test(""))
            Text(NewEmblem.shown() + CountEmblem("3").shown())
            Text(kindOf(Tag("x")) + "; " + kindOf(FakeShelf()) + "; " + kindOf(vm))
            Text(if (shelf is FakeShelf) "sahte: " + (shelf as FakeShelf).items + " " + shelf.picks else "başka " + (shelf == EmptyShelf))
            val tags: List<Titled> = listOf(Tag("x"), Tag("x"))
            Text("eşit " + (tags[0] == tags[1]) + " " + (tags[0] is Tag) + " " + (tags[0] as Tag) + " " + (tags[0] is FakeShelf))
            Button(onClick = { shelf.picks++ ; shelf = shelf }) { Text("Seç") }
            Button(onClick = { shelf = EmptyShelf }) { Text("Boşalt") }
        }
    }
}

fn interfaces_as_kotlin_writes_them() {
    let rule = KompozTestRule::new(InterfaceForms);
    rule.wait_for_idle();
    rule.on_node_with_text("[sahte] 2").assert_exists();
    rule.on_node_with_text("<raf> [adsız] [boş] [etiket] [boş]").assert_exists();
    rule.on_node_with_text("geçerli true false").assert_exists();
    rule.on_node_with_text("<yeni><3>").assert_exists();
    rule.on_node_with_text("etiket x; raf 2 2; başka").assert_exists();
    rule.on_node_with_text("sahte: [a, b] 0").assert_exists();
    rule.on_node_with_text("eşit true true Tag(name=x) false").assert_exists();
    // (Compose's rule, not the C++ Kompoz's: `shelf = shelf` sets a value equal to the state's — the same object,
    // a class's `equals` its identity —, which `mutableStateOf`'s structural policy takes as no change, so nothing
    // recomposes and the line keeps its `0`; the C++ state of an interface's handle is `neverEqual`, its line `1`)
    rule.on_node_with_text("Seç").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("sahte: [a, b] 0").assert_exists();
    rule.on_node_with_text("Boşalt").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("[boş] 0").assert_exists();
    rule.on_node_with_text("başka true").assert_exists();
}

// ---- data_class_bodies -------------------------------------------------------------------------------------------

kompoz! {
    // A data class's body: functions (one implementing an interface's), a property computed as it is made, a getter, a
    // lazy one, an init block; copy() makes it again (its body's values anew), == and toString() over its fields only.
    data class Receipt(val item: String, val price: Int, val count: Int = 1) : Titled {
        val total = price * count
        val isBig get() = total >= 100
        val label by lazy { "$item x$count" }
        override val name: String get() = item

        init {
            require(count > 0)
        }

        fun withTax(rate: Int): Int = total + total * rate / 100
        fun describe() = "$label = $total" + (if (isBig) " (büyük)" else "")
    }

    @Kompozable
    fun ReceiptView() {
        val r = Receipt("kalem", 30)
        val more = r.copy(count = 4)
        Column {
            Text(r.describe() + " | " + more.describe() + " | " + more.withTax(10))
            Text("" + r + " " + (r == Receipt("kalem", 30)) + " " + titleOf(more))
        }
    }
}

fn data_class_bodies() {
    let rule = KompozTestRule::new(ReceiptView);
    rule.wait_for_idle();
    rule.on_node_with_text("kalem x1 = 30 | kalem x4 = 120 (büyük) | 132").assert_exists();
    line(&rule, "Receipt(", "Receipt(item=kalem, price=30, count=1) true [kalem]");
}

// ---- abstract_and_open_classes -----------------------------------------------------------------------------------

kompoz! {
    // Abstract and open classes: abstract properties and functions, open ones overridden (super.describe()), a final
    // one inherited; subclasses as their base (a List<Plate>), is / as on them.
    abstract class Plate(val label: String) {
        abstract val corners: Int
        abstract fun area(): Float
        open fun describe() = "$label: $corners köşe, alan ${area()}"
        fun shout() = label.uppercase()
    }

    class Square(val side: Float) : Plate("kare") {
        override val corners = 4
        override fun area() = side * side
    }

    class Round(val r: Float) : Plate("daire") {
        override val corners: Int get() = 0
        override fun area() = 3f * r * r
        override fun describe() = "yuvarlak " + super.describe()
    }

    open class Stepper {
        var total = 0
        open fun step() = 1
        fun tick() { total += step() }
    }

    class DoubleStepper : Stepper() {
        override fun step() = 2
    }

    abstract class PageViewModel : ViewModel() {
        abstract fun heading(): String
        fun banner() = "<" + heading() + ">"
    }

    class StartViewModel : PageViewModel() {
        override fun heading() = "başlangıç"
    }

    @Kompozable
    fun PlateForms() {
        val plates: List<Plate> = listOf(Square(2f), Round(1f))
        val stepper: Stepper = DoubleStepper()
        val page = viewModel<StartViewModel>()
        stepper.tick()
        stepper.tick()
        Column {
            for (p in plates) Text(p.describe())
            Text(page.banner())
            Text(plates[0].shout() + " " + (plates[1] is Round) + " " + (plates[0] is Round) + " " + (plates[0] as Square).side + " " + stepper.total)
        }
    }
}

fn abstract_and_open_classes() {
    let rule = KompozTestRule::new(PlateForms);
    rule.wait_for_idle();
    rule.on_node_with_text("kare: 4 köşe, alan 4.0").assert_exists();
    rule.on_node_with_text("yuvarlak daire: 0 köşe, alan 3.0").assert_exists();
    rule.on_node_with_text("<başlangıç>").assert_exists();
    line(&rule, "KARE", "KARE true false 2.0 4");
}

// ---- generic_classes_and_interfaces ------------------------------------------------------------------------------

kompoz! {
    // Generic classes, data classes and interfaces: their type arguments written or taken from the values, an
    // interface's default function over its T, a class deriving from a generic one (super), a generic fun interface
    // from a lambda.
    interface Feed<T> {
        fun next(): T
        fun two(): List<T> = listOf(next(), next())  // (Kotlin's order: the first next() first)
    }

    class Countdown(var from: Int) : Feed<Int> {
        override fun next(): Int {
            from -= 1
            return from + 1
        }
    }

    class Repeater<T>(private val value: T) : Feed<T> {
        override fun next() = value
    }

    class Crate<T>(val value: T) {
        fun get(): T = value
        fun describe() = "Crate($value)"
        fun <R> convert(f: (T) -> R): Crate<R> = Crate(f(value))
    }

    fun <T> firstOr(items: List<T>, fallback: T): T = if (items.isEmpty()) fallback else items.first()

    fun <T, R> mapAll(items: List<T>, transform: (T) -> R): List<R> = items.map { transform(it) }

    fun <T> List<T>.second(): T = this[1]

    fun <T> emptyBag(): List<T> = emptyList()

    @Kompozable
    fun <T> LabeledRow(items: List<T>, label: (T) -> String) {
        Row {
            for (x in items) Text(label(x))
        }
    }

    open class Holder<T>(val item: T) {
        open fun show() = "tutar: $item"
    }

    class NamedHolder(item: String) : Holder<String>(item) {
        override fun show() = "ad " + super.show()
    }

    data class Duo<A, B>(val first: A, val second: B) {
        fun swap() = Duo(second, first)
    }

    fun interface Mapper<T> {
        fun map(x: T): String
    }

    @Kompozable
    fun GenericForms() {
        val box = Crate("kutu")
        val ints: Crate<Int> = Crate(3)
        val duo = Duo(1, "bir")
        val src: Feed<Int> = Countdown(3)
        val rep: Feed<String> = Repeater("a")
        val mapper = Mapper<Int> { "#$it" }
        Column {
            Text(box.get() + " " + ints.get() + " " + box.describe() + " " + Crate<String>("b").value)
            Text("" + duo + " " + duo.swap() + " " + (duo == Duo(1, "bir")))
            Text("" + src.two() + " " + rep.two() + " " + mapper.map(5) + " " + NamedHolder("x").show())
            Text("işlev " + firstOr(listOf(4, 5), 0) + " " + firstOr(emptyBag<Int>(), 7) + " " + mapAll(listOf(1, 2)) { it * 10 } + " " +
                listOf("a", "b").second() + " " + ints.convert { "#$it" }.get())
            LabeledRow(listOf(1, 2)) { "etiket $it" }
        }
    }
}

fn generic_classes_and_interfaces() {
    let rule = KompozTestRule::new(GenericForms);
    rule.wait_for_idle();
    line(&rule, "kutu", "kutu 3 Crate(kutu) b");
    line(&rule, "Duo(", "Duo(first=1, second=bir) Duo(first=bir, second=1) true");
    line(&rule, "[3, 2]", "[3, 2] [a, a] #5 ad tutar: x");
    line(&rule, "işlev", "işlev 4 7 [10, 20] b #3");
    rule.on_node_with_text("etiket 2").assert_exists();
}

// ---- enum_classes_with_bodies ------------------------------------------------------------------------------------

kompoz! {
    // Enum classes: one with a constructor (defaults, named arguments) and a body (a property, functions, a
    // companion), one with a body only, one of entries alone; name, ordinal, entries, values(), valueOf(), "$x", when
    // over them.
    enum class Rank(val n: Int, val label: String = "seviye") {
        LOW(1), MID(n = 5, label = "orta"), HIGH(9);

        val doubled = n * 2
        fun next(): Rank = if (this == LOW) MID else HIGH
        fun describe() = "$name/$ordinal/$label/$doubled"

        companion object {
            fun of(n: Int) = Rank.entries.first { it.n >= n }
        }
    }

    enum class Turn { LEFT, RIGHT; fun flip() = if (this == LEFT) RIGHT else LEFT }

    enum class Suit { HEART, SPADE }

    @Kompozable
    fun EnumForms() {
        val low = Rank.LOW
        Column {
            Text("rütbe " + low.describe() + " " + low.next() + " " + Rank.MID.describe() + " " + Rank.valueOf("HIGH").n + " " + Rank.entries.size + " " + Rank.of(4))
            Text("renk " + Turn.LEFT.flip() + " " + Suit.SPADE + " " + Suit.SPADE.name + " " + Suit.SPADE.ordinal + " " + Suit.values().size + " " +
                Suit.valueOf("HEART") + " " + when (low) {
                    Rank.LOW -> "düşük"
                    else -> "başka"
                })
            val nums = listOf(1, 2, 3, 4)
            Text("koşul " + nums.first { it > 1 } + " " + nums.last { it % 2 == 1 } + " " + nums.firstOrNull { it > 9 } + " " + nums.lastOrNull { it < 3 })
        }
    }
}

fn enum_classes_with_bodies() {
    let rule = KompozTestRule::new(EnumForms);
    rule.wait_for_idle();
    line(&rule, "rütbe", "rütbe LOW/0/seviye/2 MID MID/1/orta/10 9 3 MID");
    line(&rule, "renk", "renk RIGHT SPADE SPADE 1 2 HEART düşük");
    rule.on_node_with_text("koşul 2 3 null 2").assert_exists();
}

// ---- inner_classes_hold_their_outer_object -----------------------------------------------------------------------

kompoz! {
    // inner class: an object holding its outer one, made as outer.Counter(…) or Counter(…) in it; it reaches the outer
    // one's members by their names (its own first, private ones too), the outer object as this@Scoreboard; an inner
    // class's inner class reaches both.
    class Scoreboard(val name: String) {
        var count = 0
        private var bumps = 0
        fun note(by: Int) {
            count += by
            bumps++
        }
        fun counter() = Counter(2)

        inner class Counter(val step: Int) {
            val name = "sayaç"
            fun bump() {
                note(step)
                count++
            }
            fun label() = "$name/${this@Scoreboard.name}:$count:$bumps"
            fun owner() = this@Scoreboard
            fun twin() = Counter(step * 10)
            fun take(n: Int) = step * n  // (named as Kotlin's take: its own, as Kotlin's member comes first)

            inner class Mark(val tag: String) {
                fun text() = "$tag$step$count"
            }
        }
    }

    fun innerReport(): String {
        val tally = Scoreboard("t")
        val c = tally.counter()
        c.bump()
        val d = tally.Counter(5)
        d.bump()
        val mark = d.Mark("m")
        return "iç sınıf " + c.label() + " " + d.twin().step + " " + (d.owner() == tally) + " " + mark.text() + " " + tally.count + " " + d.take(2)
    }

    @Kompozable
    fun InnerForms() {
        Text(innerReport())
    }
}

fn inner_classes_hold_their_outer_object() {
    let rule = KompozTestRule::new(InnerForms);
    rule.wait_for_idle();
    line(&rule, "iç sınıf", "iç sınıf sayaç/t:9:2 50 true m59 9 10");
}

// ---- enum_entries_with_bodies_of_their_own ----------------------------------------------------------------------

kompoz! {
    // An enum entry with a body of its own: what it overrides of its class's (an abstract fun, an open fun, an open
    // val, toString), its parameters named its own way too; the class's own for the entries without one.
    enum class Calc(val symbol: String) {
        PLUS("+") {
            override fun apply(a: Int, b: Int) = a + b
            override val loud = true
        },
        TIMES("*") {
            override fun apply(x: Int, y: Int): Int = x * y
            override fun describe() = "çarpı $symbol"
            override fun toString() = "kez"
        },
        MINUS("-") {
            override fun apply(a: Int, b: Int) = a - b
        };

        abstract fun apply(a: Int, b: Int): Int
        open fun describe() = "işlem $symbol"
        open val loud: Boolean = false
        fun shown() = "$this=${apply(6, 3)}${if (loud) "!" else ""}"
    }

    enum class Sign {
        POS { override fun of(n: Int) = n },
        NEG { override fun of(n: Int) = -n };
        abstract fun of(n: Int): Int
    }

    @Kompozable
    fun EntryBodyForms() {
        Text("girdi " + Calc.entries.joinToString(" ") { it.shown() } + " " + Calc.TIMES.describe() + " " + Calc.MINUS.describe() + " " + Calc.PLUS.loud + " " +
             listOf(1, 2, 3).joinToString(", ", "[", "]", 2) + " " + "a/b/c.txt".substringAfterLast('/') + "a:b".substringBefore(":") +
             "x".substringAfter("-", "yok") + " " + Sign.NEG.of(4))
    }
}

fn enum_entries_with_bodies_of_their_own() {
    let rule = KompozTestRule::new(EntryBodyForms);
    rule.wait_for_idle();
    line(&rule, "girdi", "girdi PLUS=9! kez=18 MINUS=3 çarpı * işlem - true [1, 2, ...] c.txtayok -4");
}

// ---- sealed_classes_with_constructors_and_members ----------------------------------------------------------------

kompoz! {
    // A sealed class with a constructor, members and an interface: each case gives its constructor's values
    // (Screen("home"), by name too, defaults for the rest) and has its members (a value made from them, a function, an
    // open one it overrides, the interface's property it implements); the sealed value reads and calls them through
    // the case it holds, and is the interface too.
    interface Headed {
        val title: String
    }

    sealed class Screen(val route: String, val depth: Int = 0) : Headed {
        object Home : Screen("home") {
            override val title = "Ana"
            override fun badge() = "!"
        }
        data class Detail(val id: Int) : Screen("detail/$id", depth = 1) {
            override val title get() = "Ayrıntı $id"
            override fun badge() = "#$id"
        }
        data object Options : Screen(route = "options") {
            override val title = "Ayarlar"
        }

        val upper = route.uppercase()
        open fun badge() = ""
        fun label() = "$title@$route${badge()}"
        fun isDeep() = depth > 0
        companion object {
            val start = "home"
        }
    }

    data class Lost(val path: String) : Screen("lost/$path") {
        override val title = "Kayıp"
    }

    fun screenReport(): String {
        val all = listOf<Screen>(Screen.Home, Screen.Detail(7), Screen.Options, Lost("x"))
        val named: Headed = Screen.Detail(2)
        val first: Headed = all[0]
        return all.joinToString(" ") { it.label() + ":" + it.depth + ":" + it.upper + ":" + it.isDeep() } + " " + named.title + " " +
            first.title + " " + Screen.Detail(3).route + " " + Screen.start
    }

    @Kompozable
    fun ScreenForms() {
        Text("ekran " + screenReport())
    }
}

fn sealed_classes_with_constructors_and_members() {
    let rule = KompozTestRule::new(ScreenForms);
    rule.wait_for_idle();
    line(
        &rule,
        "ekran",
        "ekran Ana@home!:0:HOME:false Ayrıntı 7@detail/7#7:1:DETAIL/7:true Ayarlar@options:0:OPTIONS:false \
         Kayıp@lost/x:0:LOST/X:false Ayrıntı 2 Ana detail/3 home",
    );
}

// ---- nested_sealed_and_generic_declarations ----------------------------------------------------------------------

kompoz! {
    // Declarations nested in a class that are a sealed class (its cases in it and beside it) or generic (a data
    // class, a class, an interface).
    class Wizard {
        sealed class Step(val index: Int) {
            object Start : Step(0)
            data class Middle(val note: String) : Step(1)
        }
        data class Finish(val total: Int) : Step(2)
        data class Slot<T>(val value: T)
        class Holder<T>(val item: T) {
            fun twice(): List<T> = listOf(item, item)
        }
        interface Source<T> {
            fun next(): T
        }
        fun steps(): List<Step> = listOf(Step.Start, Step.Middle("orta"), Finish(3))
        fun slot() = Slot(2.5)
    }

    fun wizardReport(): String {
        val kinds = Wizard().steps().joinToString(",") { s ->
            when (s) {
                is Wizard.Step.Start -> "baş${s.index}"
                is Wizard.Step.Middle -> "${s.note}${s.index}"
                is Wizard.Finish -> "son${s.total}${s.index}"
            }
        }
        val slot = Wizard.Slot("k")
        val holder = Wizard.Holder(4)
        val source = object : Wizard.Source<Int> {
            override fun next() = 9
        }
        return "sihirbaz $kinds ${slot.value} ${Wizard().slot().value} ${holder.twice()} ${source.next()}"
    }

    @Kompozable
    fun WizardForms() {
        Text(wizardReport())
    }
}

fn nested_sealed_and_generic_declarations() {
    let rule = KompozTestRule::new(WizardForms);
    rule.wait_for_idle();
    line(&rule, "sihirbaz", "sihirbaz baş0,orta1,son32 k 2.5 [4, 4] 9");
}

// ---- inner_classes_of_values -------------------------------------------------------------------------------------

kompoz! {
    // An inner class of a data class or an enum (values here: its object holds a copy of the outer value), and
    // declarations nested in an enum.
    data class Pad(val width: Int, val label: String) {
        inner class Cell(val index: Int) {
            fun text() = "$label[$index/$width]"
        }
        fun first() = Cell(0)
    }

    enum class Mood(val emoji: String) {
        HAPPY(":)"), SAD(":(");

        data class Note(val text: String)
        inner class Line(val text: String) {
            fun shown() = "$emoji $text ${this@Mood.name}"
        }
        fun line(text: String) = Line(text)
    }

    fun valueInners(): String {
        val pad = Pad(3, "p")
        return "değer " + pad.first().text() + " " + pad.Cell(2).text() + " " + Mood.SAD.line("eh").shown() + " " + Mood.HAPPY.Line("ya").shown() +
            " " + Mood.Note("n").text
    }

    @Kompozable
    fun ValueInnerForms() {
        Text(valueInners())
    }
}

fn inner_classes_of_values() {
    let rule = KompozTestRule::new(ValueInnerForms);
    rule.wait_for_idle();
    line(&rule, "değer", "değer p[0/3] p[2/3] :( eh SAD :) ya HAPPY n");
}

// ---- fun_interfaces_with_suspend_funs ----------------------------------------------------------------------------

kompoz! {
    // A fun interface's suspend fun: made from a lambda that is a coroutine (Fetcher { key -> delay(…); "…" }), giving
    // its value or none.
    fun interface Fetcher {
        suspend fun load(key: String): String
    }

    fun interface Pinger {
        suspend fun ping()
    }

    @Kompozable
    fun SuspendLoaderForms() {
        var text by remember { mutableStateOf("…") }
        var pinged by remember { mutableStateOf(0) }
        val loader = remember {
            Fetcher { key ->
                delay(100)
                "yüklendi $key"
            }
        }
        val pinger = remember { Pinger { delay(10); pinged++ } }
        LaunchedEffect(Unit) {
            text = loader.load("a")
            pinger.ping()
        }
        Text("yükleyici $text $pinged")
    }
}

fn fun_interfaces_with_suspend_funs() {
    let rule = KompozTestRule::new(SuspendLoaderForms);
    rule.on_node_with_text("yükleyici … 0").assert_exists();
    rule.advance_time_by(300);
    rule.wait_for_idle();
    rule.on_node_with_text("yükleyici yüklendi a 1").assert_exists();
}

// ---- companion_objects_implementing_interfaces -------------------------------------------------------------------

kompoz! {
    // A companion object implementing an interface: its members the class's (Widget.make(3)), the interface's with a
    // body too (Widget.pair()), and the class's name as a value is the companion (sizes(Widget)).
    interface Maker<T> {
        fun make(n: Int): T
        fun pair(): List<T> = listOf(make(1), make(2))
    }

    class Widget(val size: Int) {
        companion object : Maker<Widget> {
            override fun make(n: Int) = Widget(n * 10)
        }
    }

    fun sizes(maker: Maker<Widget>): List<Int> = maker.pair().map { it.size }

    @Kompozable
    fun CompanionInterfaceForms() {
        Text("eş ${Widget.make(3).size} ${Widget.pair().size} ${sizes(Widget)}")
    }
}

fn companion_objects_implementing_interfaces() {
    let rule = KompozTestRule::new(CompanionInterfaceForms);
    rule.wait_for_idle();
    rule.on_node_with_text("eş 30 2 [10, 20]").assert_exists();
}

// ---- sealed_classes_with_class_cases -----------------------------------------------------------------------------

kompoz! {
    // A sealed class's cases that are classes (in it, and at the top level): handles made with its constructor's
    // values (by name too) and members, an open one overridden, reached through `is` and `when` as the others are.
    sealed class Outcome(val code: Int) {
        class Done(val value: String) : Outcome(200) {
            fun shout() = value.uppercase()
        }
        data class Waiting(val percent: Int) : Outcome(102)
        open fun summary() = "kod $code"
    }

    class Failed(val reason: String) : Outcome(code = 500) {
        override fun summary() = "hata $reason $code"
    }

    object Idle : Outcome(0) {
        override fun summary() = "boş"
    }

    fun outcomeReport(): String {
        val all = listOf<Outcome>(Outcome.Done("tamam"), Outcome.Waiting(40), Failed("bozuk"), Idle)
        return "sonuç " + all.joinToString(" ") { o ->
            when (o) {
                is Outcome.Done -> o.shout() + "/" + o.summary()
                is Outcome.Waiting -> "%${o.percent}/" + o.summary()
                is Failed -> o.summary()
                is Idle -> o.summary()
            }
        } + " " + all.map { it.code }
    }

    @Kompozable
    fun OutcomeForms() {
        Text(outcomeReport())
    }
}

fn sealed_classes_with_class_cases() {
    let rule = KompozTestRule::new(OutcomeForms);
    rule.wait_for_idle();
    line(&rule, "sonuç", "sonuç TAMAM/kod 200 %40/kod 102 hata bozuk 500 boş [200, 102, 500, 0]");
}

// ---- inner_classes_of_generic_data_classes_and_with_cpp_bases ----------------------------------------------------

/// The C++ test's `struct Greeting { virtual std::string hello() { return "merhaba"; } }`, a base written in the
/// host language that a Kotlin class derives from: here Rust's, an open class's trait (its members the class's
/// open ones, their defaults its bodies).
///
/// Kotlin: `open class Greeting`
pub trait Greeting {
    /// Kotlin: `hello(): String`
    fn hello(&self) -> String {
        "merhaba".to_string()
    }
}

kompoz! {
    // An inner class of a generic data class (made as box.Label(…): its type the box's own, Box<Int>'s), and an inner
    // class deriving from a Rust base (Greeting, written in Rust above: it holds its outer object itself).
    data class Parcel<T>(val content: T) {
        inner class Label(val n: Int) {
            fun text() = "$content#$n"
        }
        fun first() = Label(0)
    }

    class Host(val name: String) {
        inner class Polite : Greeting() {
            override fun hello(): String = "sayın $name"
        }
        fun polite() = Polite()
    }

    fun moreInners(): String {
        val parcel = Parcel(5)
        val host = Host("ada")
        val polite = host.Polite()
        return "iç " + parcel.first().text() + " " + parcel.Label(2).text() + " " + Parcel("k").Label(3).text() + " " + polite.hello() + " " + host.polite().hello()
    }

    @Kompozable
    fun MoreInnerForms() {
        Text(moreInners())
    }
}

fn inner_classes_of_generic_data_classes_and_with_cpp_bases() {
    let rule = KompozTestRule::new(MoreInnerForms);
    rule.wait_for_idle();
    line(&rule, "iç ", "iç 5#0 5#2 k#3 sayın ada sayın ada");
}

// ---- classes_as_kotlin_writes_them (its classes) -----------------------------------------------------------------

kompoz! {
    // Classes as Kotlin writes them: a ViewModel with a state flow and a function, a class with a primary
    // constructor (val and plain parameters, a default), a property typed by its value, an init block, a
    // flow built in a function typed Flow<T>, a saved state handle set by index, a lifecycle effect.
    data class Todo(val id: Int, val title: String, val done: Boolean = false)

    class TodoViewModel : ViewModel() {
        val items = MutableStateFlow(listOf(Todo(1, "Yaz"), Todo(2, "Gönder")))
        var toggles = 0

        fun toggle(id: Int, done: Boolean) {
            toggles++
            items.update { list -> list.map { if (it.id == id) it.copy(done = done) else it } }
        }

        override fun onCleared() {}
    }

    class Greeter(private val name: String, suffix: String, val punctuation: String = "!") {
        val greeting: String = "Merhaba, $name$suffix"   // (typed: it reads a plain parameter)
        var greeted = 0
        init { greeted = -1 }
        fun greet(): String {
            greeted++
            return greeting + punctuation
        }
        private fun hidden() = 1
    }

    fun ticks(): Flow<Int> = flow {
        for (i in 0 until 3) {
            emit(i)
            delay(10)
        }
    }

    class QueryViewModel(private val state: SavedStateHandle) : ViewModel() {
        val query: StateFlow<String> = state.getStateFlow("query", "")
        fun setQuery(q: String) { state["query"] = q }
    }

    class Player { var playing = false; fun play() { playing = true } fun pause() { playing = false } }

    @Kompozable
    fun TodoScreen(player: Player) {
        val vm = viewModel<TodoViewModel>()
        val items by vm.items.collectAsState()
        LazyColumn {
            items(items, key = { it.id }) { t ->
                Text(if (t.done) "✓ ${t.title}" else t.title, modifier = Modifier.clickable { vm.toggle(t.id, !t.done) })
            }
        }
        LifecycleResumeEffect(Unit) {
            player.play()
            onPauseOrDispose { player.pause() }
        }
    }
}

fn classes_as_kotlin_writes_them() {
    let player = Player();
    let p = player.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || TodoScreen(p.clone())));
    rule.on_node_with_text("Yaz").assert_exists();
    assert!(player.playing()); // (the composable copied the handle: the same object)
    rule.on_node_with_text("Yaz").perform_click();
    rule.on_node_with_text("✓ Yaz").assert_exists();
    let vm: std::rc::Rc<TodoViewModel> =
        ViewModelProvider::create(&*rule.host().view_model_store_owner(), None, None).get::<TodoViewModel>();
    assert_eq!(vm.toggles(), 1);
    let g = named_args!(Greeter("Ada", " Lovelace"));
    assert!(g.greeting() == "Merhaba, Ada Lovelace" && g.greeted() == -1);
    assert!(g.greet() == "Merhaba, Ada Lovelace!" && g.greeted() == 0);
    let same = g.clone(); // (a copy is the same object, as Kotlin's)
    same.greet();
    assert!(g.greeted() == 1 && same == g);
    assert_eq!(
        Greeter::__kz_builder().name("Bo").suffix("").punctuation("?".to_string()).__call().greet(),
        "Merhaba, Bo?"
    );
    let q = QueryViewModel(SavedStateHandle::new());
    q.set_query("kompoz");
    assert_eq!(q.query().value(), "kompoz");
    let _ = ticks(); // (it compiles: a Flow<i32> from flow { emit(i) })
}

// ---- what class hierarchies have besides the C++ tests' ----------------------------------------------------------

kompoz! {
    // A data class deriving from an abstract class (the base's initializer first: Kotlin's order), its body's `var`
    // with a setter and a property with a getter reading `field`; a class implementing an interface by delegation;
    // a composable member function; an open class's secondary constructor, a subclass's super call going through
    // it; an enum entry's own function; an interface's generic function; object expressions extending an abstract
    // class and a ViewModel.
    abstract class Logged(val tag: String) {
        var log = "base:$tag"
        abstract fun kind(): String
    }

    data class Entry(val n: Int) : Logged("e$n") {
        val seen = log + "/entry"
        var count = n
            set(value) { field = if (value < 0) 0 else value }
        val doubled: Int = n * 2
            get() = field + 1
        override fun kind() = "entry"
    }

    interface Speaker {
        fun speak(): String
        fun twice() = speak() + speak()
    }

    class Loud(val word: String) : Speaker {
        override fun speak() = word.uppercase()
    }

    class Echo(inner: Speaker) : Speaker by inner

    class Panel(val title: String) {
        @Kompozable
        fun Content(suffix: String = "!") {
            Text("panel $title$suffix")
        }
    }

    open class Tile(val w: Int, val h: Int) {
        constructor(side: Int) : this(side, side)
        fun area() = w * h
    }

    class Square2(s: Int) : Tile(s)

    enum class Op {
        DOUBLE {
            fun factor() = 2
            override fun apply(x: Int) = x * factor()
        };
        abstract fun apply(x: Int): Int
    }

    interface Boxed {
        val label: String
        fun <R> mapLabel(f: (String) -> R): R = f(label)
    }

    data class Labeled(override val label: String) : Boxed

    fun extras(): String {
        val e = Entry(3)
        e.count = -5
        val echo = Echo(Loud("ho"))
        val anon = object : Logged("o") {
            override fun kind() = "anon " + tag
        }
        val vm = object : ViewModel() {
            override fun onCleared() {}
        }
        val boxed: Boxed = Labeled("kutu")
        return "ek " + e.seen + " " + e.count + " " + e.doubled + " " + e.kind() + " " + echo.twice() + " " + Square2(3).area() + " " +
            Tile(2).area() + " " + Op.DOUBLE.apply(5) + " " + boxed.mapLabel { it.length } + " " + anon.kind() + " " + anon.log + " " + (vm is ViewModel)
    }

    @Kompozable
    fun ExtraForms() {
        val panel = remember { Panel("p") }
        Column {
            Text(extras())
            panel.Content()
        }
    }
}

fn hierarchy_forms_beyond_the_cpp_tests() {
    let rule = KompozTestRule::new(ExtraForms);
    rule.wait_for_idle();
    line(&rule, "ek ", "ek base:e3/entry 0 7 entry HOHO 9 4 10 4 anon o base:o true");
    rule.on_node_with_text("panel p!").assert_exists();
}

// ---- generic sealed classes (WPA-rest) ---------------------------------------------------------------------------

kompoz! {
    // Generic sealed interfaces and classes as Compose's samples write them (JetNews' `Result<out R>`, a screen's UI
    // state): a case of the class's own argument (`Success<out T>(val data: T) : Result<T>`) and cases of `Nothing`
    // (`Error`, `Loading`: one type in every `Result<T>`, Kotlin's `out` variance); a case given where a
    // `Result<String>` is taken (a `val`'s declared type, a function's result — each branch's, a `return`'s —, an
    // assignment, a state's value); `when` over them (the subject smart-cast, its case's type argument the value's),
    // `is` / `as?`, and the class's own members reading `this` as one of its cases.
    sealed interface Result<out R> {
        data class Success<out T>(val data: T) : Result<T>
        data class Error(val exception: Throwable) : Result<Nothing>
    }

    fun <T> Result<T>.successOr(fallback: T): T = when (this) {
        is Result.Success -> data
        is Result.Error -> fallback
    }

    fun fetch(ok: Boolean): Result<String> {
        if (!ok) return Result.Error(IllegalStateException("bağlantı yok"))
        return Result.Success("veri")
    }

    sealed class Loadable<out T> {
        data class Done<out T>(val value: T) : Loadable<T>()
        data class Failed(val reason: String) : Loadable<Nothing>()
        object Loading : Loadable<Nothing>()

        val isLoading get() = this is Loading
        fun valueOrNull(): T? = if (this is Done) value else null
        fun describe(): String = when (this) {
            is Done -> "değer $value"
            is Failed -> "hata $reason"
            Loading -> "yükleniyor"
        }
    }

    fun parse(text: String): Loadable<Int> = if (text.isEmpty()) Loadable.Failed("boş") else Loadable.Done(text.length)

    fun shown(o: Loadable<Int>): String = when (o) {
        is Loadable.Done -> "#" + (o.value + 1)
        is Loadable.Failed, Loadable.Loading -> "-"
    }

    @Kompozable
    fun GenericSealedForms() {
        var outcome: Loadable<Int> by remember { mutableStateOf(Loadable.Loading) }
        val first: Result<String> = fetch(true)
        var second: Result<String> = Result.Error(IllegalArgumentException("yok"))
        val before = second.successOr("önce")
        second = fetch(false)
        val failed = second as? Result.Error
        Column {
            Text("sonuç " + outcome.describe() + " " + outcome.isLoading + " " + outcome.valueOrNull() + " " + shown(outcome))
            Text("veri " + first.successOr("?") + " " + before + " " + second.successOr("?") + " " + (first is Result.Success) + " " + failed?.exception?.message +
                " " + parse("") + " " + parse("abc") + " " + parse("ab").valueOrNull() + " " + listOf(parse("a"), Loadable.Loading).map { it.isLoading })
            Button(onClick = { outcome = parse("kedi") }) { Text("Ayrıştır") }
        }
    }
}

fn generic_sealed_classes() {
    let rule = KompozTestRule::new(GenericSealedForms);
    rule.wait_for_idle();
    rule.on_node_with_text("sonuç yükleniyor true null -").assert_exists();
    line(&rule, "veri ", "veri veri önce ? true bağlantı yok Failed(reason=boş) Done(value=3) 2 [false, true]");
    rule.on_node_with_text("Ayrıştır").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("sonuç değer 4 false 4 #5").assert_exists();
}

// ---- API bases of the second kind (WPA-rest) -----------------------------------------------------------------------

kompoz! {
    // The API's abstract classes that are structs of callbacks, as their samples extend them in object expressions:
    // navigationevent's `NavigationEventHandler<T>` (its constructor's arguments by name; its overrides' `this` the
    // handler: `transitionState`, `isBackEnabled`), navigation's `NavType<T>` (`isNullableAllowed` its method, `name`
    // an `override val`, the methods' Rust parameters given as Kotlin's: `key: String`, `value: Point`).
    data class Point(val x: Int, val y: Int)

    fun backHandler(log: MutableState<String>): NavigationEventHandler<NavigationEventInfo.None> =
        object : NavigationEventHandler<NavigationEventInfo.None>(initialInfo = NavigationEventInfo.None, isBackEnabled = true) {
            override fun onBackStarted(event: NavigationEvent) {
                log.value += "başla "
            }
            override fun onBackProgressed(event: NavigationEvent) {
                log.value += "${event.progress} "
            }
            override fun onBackCompleted() {
                log.value += "bitti " + (transitionState == NavigationEventTransitionState.Idle) + " " + isBackEnabled
            }
        }

    fun pointNavType(): NavType<Point> = object : NavType<Point>(isNullableAllowed = false) {
        override val name = "nokta"
        override fun put(bundle: SavedState, key: String, value: Point) {
            bundle.write { putString(key, "${value.x},${value.y}") }
        }
        override fun get(bundle: SavedState, key: String): Point? {
            val text = bundle.read { getString(key) }
            return Point(text.substringBefore(",").toInt(), text.substringAfter(",").toInt())
        }
        override fun parseValue(value: String): Point = Point(value.substringBefore(",").toInt(), value.substringAfter(",").toInt())
        override fun serializeAsValue(value: Point): String = "${value.x},${value.y}"
    }

    object NullablePointType : NavType<Point>(isNullableAllowed = true) {
        override fun put(bundle: SavedState, key: String, value: Point?) {
            if (value != null) bundle.write { putString(key, "${value.x}:${value.y}") }
        }
        override fun get(bundle: SavedState, key: String): Point? = null
        override fun parseValue(value: String): Point? =
            if (value == "null") null else Point(value.substringBefore(":").toInt(), value.substringAfter(":").toInt())
        override fun serializeAsValue(value: Point?): String = if (value == null) "null" else "${value.x}:${value.y}"
    }

    fun nullablePoints(): NavType<Point> = NullablePointType

    // navigation3-ui's AnimatedBottomSheetSample: a scene of its own, `object : OverlayScene<T>` (its supertype
    // `Scene<T>` implemented too, its objects compared and shown as Kotlin's are, by identity), its `content` a
    // composable lambda, its suspending `onRemove`.
    fun sheetScene(entry: NavEntry<String>, previous: List<NavEntry<String>>, log: MutableState<String>): OverlayScene<String> =
        object : OverlayScene<String> {
            override val key = entry.contentKey
            override val entries = listOf(entry)
            override val previousEntries = previous
            override val overlaidEntries = previous
            override val content: @Composable () -> Unit = { Text("sayfa") }
            override suspend fun onRemove() {
                log.value += "kaldırıldı"
            }
        }

    // lifecycle-viewmodel's CustomFactory sample: `object : ViewModelProvider.Factory` overriding the generic
    // `create` (`T` its bound, a ViewModel; `as T` the value, made the factory's shared ViewModel).
    class NoteViewModel(val note: String) : ViewModel()

    fun noteFactory(note: String): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: KClass<T>, extras: CreationExtras): T {
            @Suppress("UNCHECKED_CAST")
            return NoteViewModel(note) as T
        }
    }

    // foundation's dragAndDropTarget sample: `object : DragAndDropTarget` (its other callbacks the defaults).
    fun dropTarget(log: MutableState<String>): DragAndDropTarget = object : DragAndDropTarget {
        override fun onDrop(event: DragAndDropEvent): Boolean {
            log.value += "bırakıldı "
            return true
        }
        override fun onEntered(event: DragAndDropEvent) {
            log.value += "girdi "
        }
    }
}

fn api_bases_of_the_second_kind() {
    use kompoz::navigationevent::{DirectNavigationEventInput, NavigationEvent, NavigationEventDispatcher};
    let log = mutable_state_of(String::new());
    let handler = back_handler(log.clone());
    let dispatcher = NavigationEventDispatcher::new();
    dispatcher.add_handler(&handler);
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    input.dispatch_on_back_started(NavigationEvent::default());
    input.dispatch_on_back_progressed(NavigationEvent { progress: 0.5, ..NavigationEvent::default() });
    input.dispatch_on_back_completed();
    assert_eq!(log.get(), "başla 0.5 bitti true true");
    let t = point_nav_type();
    assert_eq!(t.name(), "nokta");
    assert!(!t.is_nullable_allowed());
    let bundle = SavedState::new();
    t.parse_and_put(&bundle, "p", "3,4").unwrap();
    assert_eq!(t.get::<Point>(&bundle, "p"), Some(Point(3, 4)));
    assert_eq!(t.parse_value::<Point>("1,2").unwrap(), Some(Point(1, 2)));
    let n = nullable_points();
    assert!(n.is_nullable_allowed() && n.parse_value::<Point>("null").unwrap().is_none());
    assert_eq!(n.parse_value::<Point>("5:6").unwrap(), Some(Point(5, 6)));
    assert_eq!(n.serialize_as_value(Some(&Point(1, 2))), "1:2");
    // The overlay scene: its entries, itself as an overlay scene, `==` its identity, its `onRemove` a coroutine.
    use kompoz::navigation3::{Metadata, NavEntry, Scene};
    let entry = |k: &str| {
        NavEntry::new(k.to_string(), kompoz::ui::layout::SlotKey::new(k.to_string()), Metadata::new(), |_| {})
    };
    let log = mutable_state_of(String::new());
    let scene = sheet_scene(entry("üst"), vec![entry("alt")], log.clone());
    assert_eq!(scene.entries().len(), 1);
    assert_eq!(scene.overlaid_entries().len(), 1);
    let same = scene.clone();
    assert!(*scene == *same);
    let other = sheet_scene(entry("üst"), vec![], log.clone());
    assert!(*scene != *other);
    let as_scene: std::rc::Rc<dyn Scene<String>> = scene.clone();
    assert!(as_scene.as_overlay_scene().is_some());
    let mut removing = scene.on_remove();
    let poll = removing.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
    assert!(poll.is_ready() && log.get() == "kaldırıldı");
    // The factory: its generic `create` the trait's.
    let factory = note_factory("not".to_string());
    let made = factory.create(&KClass::of::<NoteViewModel>(), &CreationExtras::default());
    let any: std::rc::Rc<dyn std::any::Any> = made;
    assert_eq!(any.downcast_ref::<NoteViewModel>().map(|v| v.note()), Some("not".to_string()));
    // The drop target: its overrides called, the others the trait's defaults.
    let log = mutable_state_of(String::new());
    let target = drop_target(log.clone());
    let event = kompoz::ui::draganddrop::DragAndDropEvent::default();
    target.on_started(&event);
    target.on_entered(&event);
    assert!(target.on_drop(&event));
    assert_eq!(log.get(), "girdi bırakıldı ");
}

// ---- object expressions' members and secondary constructors (WPA-rest) ------------------------------------------

kompoz! {
    // An object expression extending an abstract class: an `override var` (its setter the object's), a suspending
    // override (a coroutine borrowing the object, called by the class's own suspending function); an open class's
    // secondary constructor's body run when a subclass's super call goes through it (after the primary one's
    // initializer, as Kotlin's), an abstract one's too.
    abstract class Fetcher2 {
        abstract var calls: Int
        abstract suspend fun fetch(key: String): String
        suspend fun twice(key: String): String = "${fetch(key)}${fetch(key)}"
    }

    fun counting(prefix: String): Fetcher2 = object : Fetcher2() {
        override var calls = 0
        override suspend fun fetch(key: String): String {
            delay(10)
            calls++
            return "$prefix$key"
        }
    }

    open class Panel2(val w: Int, val h: Int) {
        var note = "birincil"
        constructor(side: Int) : this(side, side) {
            note = "kare $side"
        }
    }

    class SquarePanel(s: Int) : Panel2(s)

    abstract class Labeled2(val text: String) {
        var shown = text
        constructor(n: Int) : this("#$n") {
            shown = "sayı " + text
        }
    }

    class Numbered(n: Int) : Labeled2(n)

    @Kompozable
    fun ObjectMemberForms() {
        val fetcher = remember { counting("ö:") }
        var text by remember { mutableStateOf("…") }
        LaunchedEffect(Unit) {
            text = fetcher.twice("a") + " " + fetcher.calls
            fetcher.calls = 10
            text += " " + fetcher.calls
        }
        Column {
            Text("nesne $text")
            Text("ikincil " + SquarePanel(3).note + " " + SquarePanel(3).w + " " + Panel2(2, 5).note + " " + Numbered(7).shown)
        }
    }
}

fn object_members_and_secondary_constructors() {
    let rule = KompozTestRule::new(ObjectMemberForms);
    rule.wait_for_idle();
    rule.on_node_with_text("ikincil kare 3 3 birincil sayı #7").assert_exists();
    rule.advance_time_by(100);
    rule.wait_for_idle();
    rule.on_node_with_text("nesne ö:aö:a 2 10").assert_exists();
}

// ---- the closing pass's classes and types (stage 14 FINAL-T) --------------------------------------------------

kompoz! {
    // Exceptions of the user's open and sealed classes: `e is IllegalStateException` of an open exception class's
    // value (its shared object a Kotlin exception), a sealed one's cases thrown and caught as the class or a case.
    open class AppFault(message: String) : IllegalStateException(message)
    class DiskFault : AppFault("disk")

    fun faultKind(e: AppFault): String = if (e is IllegalStateException) "ise " + e.message else "başka"
    fun diskKind(): String = faultKind(DiskFault())

    sealed class Trouble(val code: Int) : IllegalStateException("kod $code") {
        class NotFound : Trouble(404)
        class Snapped(val why: String) : Trouble(500)
    }

    // (one of no cases)
    sealed class Mishap(val code: Int) : IllegalStateException("kod $code")

    fun risky(n: Int): Int {
        if (n == 1) throw Trouble.NotFound()
        if (n == 2) throw Trouble.Snapped("bozuk")
        return n
    }

    fun tryRisky(n: Int): String = try {
        "ok " + risky(n)
    } catch (e: Trouble.Snapped) {
        "broken " + e.why + " " + e.code + " " + e.message
    } catch (e: Trouble) {
        "trouble " + e.code + " " + (e is IllegalStateException) + " " + e
    }

    // A data class compared with a value of `Any?`; hashed as Kotlin's `hashCode()` (a `Float` by its bits:
    // `NaN == NaN`, `0f != -0f`), in sets; a class named as a Rust standard type is one of the crate's.
    data class Coin(val amount: Int)
    fun sameCoin(m: Any?): Boolean = Coin(5) == m
    fun otherCoin(m: Any): Boolean = m != Coin(5)

    data class Speckle(val x: Int, val y: Float)
    fun speckles(): Int = setOf(Speckle(1, 2f), Speckle(1, 2f), Speckle(1, -0f), Speckle(1, 0f)).size
    fun speckleHashes(): Boolean = Speckle(1, 2f).hashCode() == Speckle(1, 2f).hashCode() && "ab".hashCode() == 3105 &&
        listOf(1, 2).hashCode() == 994

    class Vec(val x: Int)
    fun vec() = Vec(1)
    fun vecs() = listOf(Vec(2), Vec(3))

    // `out` variance of the user's generic classes: a `Lookup<Hound>` a `Lookup<Creature>`, given where one is
    // taken (a result, an argument, a case of `Nothing`), a data class's too.
    open class Creature(val name: String)
    class Hound(name: String) : Creature(name)

    sealed class Lookup<out T> {
        data class Found<out T>(val value: T, val all: List<T>) : Lookup<T>()
        data class Pairs<out T>(val p: Pair<T, Int>?) : Lookup<T>()
        object Missing : Lookup<Nothing>()
    }
    data class Satchel<out T>(val item: T, val more: List<T>?)

    fun hounds(): Lookup<Hound> = Lookup.Found(Hound("karabaş"), listOf(Hound("a")))
    fun creatures(): Lookup<Creature> = hounds()
    fun satchel(): Satchel<Creature> = Satchel(Hound("x"), null)
    fun describeLookup(o: Lookup<Creature>): String = when (o) {
        is Lookup.Found -> "bulundu " + o.value.name + " " + o.all.size
        is Lookup.Pairs -> "çift"
        Lookup.Missing -> "yok"
    }
    fun lookups(): String = describeLookup(hounds()) + " " + describeLookup(Lookup.Missing) + " " + satchel().item.name

    // A generic sealed class holding its type parameter's values (its cases giving their own), a sealed class's
    // secondary constructor with a body its data and plain cases' super calls go through, a sealed class of a type
    // parameter no case takes (its hidden variant).
    sealed class Keeping<out T>(val first: T) {
        data class Value<T>(val value: T) : Keeping<T>(value)
        class Both<T>(val a: T, val b: T) : Keeping<T>(a)
        fun firstOr(): T = first
    }
    fun keepings(): String {
        val k: Keeping<Int> = Keeping.Value(3)
        val p: Keeping<String> = Keeping.Both("a", "b")
        return "" + k.first + " " + k.firstOr() + " " + p.first + " " + k + " " + (k == Keeping.Value(3))
    }

    sealed class Polygon(val name: String) {
        var note = "birincil"
        constructor(sides: Int) : this("kenar $sides") {
            note = "çokgen $sides"
        }
        data class Regular(val n: Int) : Polygon(n)
        data class Named(val label: String) : Polygon(label)
        class Plain(val k: Int) : Polygon(k)
    }
    fun polygons(): String =
        Polygon.Regular(3).note + " " + Polygon.Regular(4).name + " " + Polygon.Named("x").note + " " + Polygon.Plain(5).note

    sealed class Quiet<out T> {
        object Off : Quiet<Nothing>()
        data class Note(val text: String) : Quiet<Nothing>()
    }
    fun quiets(q: Quiet<Int>): String = when (q) {
        Quiet.Off -> "kapalı"
        is Quiet.Note -> "not " + q.text
    }
    fun quietLine(): String {
        val all: List<Quiet<Int>> = listOf(Quiet.Off, Quiet.Note("a"))
        return all.map { quiets(it) }.toString() + " " + (all[1] == Quiet.Note("a")) + " " + all[1]
    }

    // Object expressions: members of their own (a `var`, a `lateinit var`, a function) its overrides share, an
    // override another calls; an object declaration of a base whose callbacks are given its struct (Kotlin's one
    // object); generic functions of an open or abstract class overridden (by a class, an object expression).
    abstract class Tocker {
        abstract fun next(): Int
        fun twice(): String = "${next()} ${next()}"
    }
    fun tocker(start: Int): Tocker = object : Tocker() {
        var n = start
        fun bump() = n++
        override fun next(): Int = bump()
    }

    interface Drip {
        fun next(): Int
        fun both(): String
    }
    fun drip(start: Int): Drip = object : Drip {
        lateinit var last: String
        var n = start
        override fun next(): Int { n++; last = "n$n"; return n }
        override fun both(): String = "${next()} $last"
    }
    fun objectsLine(): String = tocker(5).twice() + " " + drip(1).both() + " " + drip(7).both()

    data class Spot2(val x: Int, val y: Int)
    fun spotType(sep: String): NavType<Spot2> = object : NavType<Spot2>(isNullableAllowed = false) {
        override fun put(bundle: SavedState, key: String, value: Spot2) {
            bundle.write { putString(key, serializeAsValue(value)) }
        }
        override fun get(bundle: SavedState, key: String): Spot2? = parseValue(bundle.read { getString(key) })
        override fun parseValue(value: String): Spot2 =
            Spot2(value.substringBefore(sep).toInt(), value.substringAfter(sep).toInt())
        override fun serializeAsValue(value: Spot2): String = "${value.x}$sep${value.y}"
    }

    object BackLog : NavigationEventHandler<NavigationEventInfo.None>(NavigationEventInfo.None, isBackEnabled = true) {
        var log = ""
        override fun onBackStarted(event: NavigationEvent) {
            log += "başla "
        }
        override fun onBackCompleted() {
            log += "bitti " + isBackEnabled + " " + (transitionState == NavigationEventTransitionState.Idle)
            isBackEnabled = false
        }
    }
    fun backLog(): NavigationEventHandler<NavigationEventInfo.None> = BackLog
    fun backLogText(): String = BackLog.log
    fun sameHandler(): Boolean = backLog() == backLog()

    open class Wrapper {
        open fun <T> id(x: T): T = x
        open fun <T> wrap(x: T): List<T> = listOf(x)
        fun both(): String = "" + id(1) + wrap("a")
    }
    class TwinWrapper : Wrapper() {
        override fun <T> wrap(x: T): List<T> = listOf(x, x)
    }
    fun wrapper(): Wrapper = object : Wrapper() {
        override fun <T> wrap(x: T): List<T> = listOf(x, x, x)
    }
    abstract class Doubler {
        abstract fun <T> wrap(x: T): List<T>
        fun twice(): Int = wrap(2).size
    }
    fun doubler(): Doubler = object : Doubler() {
        override fun <T> wrap(x: T): List<T> = listOf(x, x)
    }
    fun genericsLine(): String =
        Wrapper().both() + " " + TwinWrapper().both() + " " + wrapper().both() + " " + doubler().twice() + " " + wrapper().wrap(true)

    // A plain class's `equals` of `Any?` (`this === other`, an early `!is` return: the smart cast after it) and its
    // `hashCode()` (Kotlin's wrapping arithmetic).
    class Brick(val side: Int) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Brick) return false
            return side == other.side
        }
        override fun hashCode(): Int = side * 1000003 * 1000003 + listOf(side).hashCode()
    }
    fun squares(): String = "" + (Brick(2) == Brick(2)) + " " + (Brick(2) == Brick(3)) + " " +
        (Brick(7).hashCode() == Brick(7).hashCode())
}

fn closing_pass_classes_and_types() {
    use std::rc::Rc;
    assert_eq!(disk_kind(), "ise disk");
    assert_eq!(try_risky(3), "ok 3");
    assert_eq!(try_risky(1), "trouble 404 true NotFound: kod 404");
    assert_eq!(try_risky(2), "broken bozuk 500 kod 500");
    let any = |v: Rc<dyn kompoz::runtime::kt::KtObject>| v;
    assert!(same_coin(Some(any(Rc::new(Coin(5))))) && !same_coin(Some(any(Rc::new(Coin(6))))));
    assert!(!same_coin(None::<Rc<dyn kompoz::runtime::kt::KtObject>>) && !other_coin(any(Rc::new(Coin(5)))));
    assert_eq!(speckles(), 3);
    assert!(speckle_hashes() && Speckle(1, f32::NAN) == Speckle(1, f32::NAN) && Speckle(1, 0.0) != Speckle(1, -0.0));
    let mut set = std::collections::HashSet::new();
    set.insert(Coin(1));
    set.insert(Coin(1));
    assert_eq!(set.len(), 1);
    assert_eq!(vec().x() + vecs().len() as i32, 3);
    assert_eq!(lookups(), "bulundu karabaş 1 yok x");
    assert_eq!(keepings(), "3 3 a Value(value=3) true");
    assert_eq!(polygons(), "çokgen 3 kenar 4 birincil çokgen 5");
    assert_eq!(quiet_line(), "[kapalı, not a] true Note(text=a)");
    assert_eq!(objects_line(), "5 6 2 n2 8 n8");
    let t = spot_type(";".to_string());
    let bundle = SavedState::new();
    t.parse_and_put(&bundle, "p", "3;4").unwrap();
    assert_eq!(t.get::<Spot2>(&bundle, "p"), Some(Spot2(3, 4)));
    {
        use kompoz::navigationevent::{DirectNavigationEventInput, NavigationEvent, NavigationEventDispatcher};
        let dispatcher = NavigationEventDispatcher::new();
        dispatcher.add_handler(&back_log());
        let input = DirectNavigationEventInput::new();
        dispatcher.add_input(&input);
        input.dispatch_on_back_started(NavigationEvent::default());
        input.dispatch_on_back_completed();
        assert_eq!(back_log_text(), "başla bitti true true");
        assert!(same_handler() && !back_log().is_back_enabled());
    }
    assert_eq!(generics_line(), "1[a] 1[a, a] 1[a, a, a] 2 [true, true, true]");
    assert_eq!(squares(), "true false true");
}

fn main() {
    common::run(&[
        ("sealed_classes_and_is", sealed_classes_and_is),
        ("interfaces_as_kotlin_writes_them", interfaces_as_kotlin_writes_them),
        ("data_class_bodies", data_class_bodies),
        ("abstract_and_open_classes", abstract_and_open_classes),
        ("generic_classes_and_interfaces", generic_classes_and_interfaces),
        ("enum_classes_with_bodies", enum_classes_with_bodies),
        ("inner_classes_hold_their_outer_object", inner_classes_hold_their_outer_object),
        ("enum_entries_with_bodies_of_their_own", enum_entries_with_bodies_of_their_own),
        ("sealed_classes_with_constructors_and_members", sealed_classes_with_constructors_and_members),
        ("nested_sealed_and_generic_declarations", nested_sealed_and_generic_declarations),
        ("inner_classes_of_values", inner_classes_of_values),
        ("fun_interfaces_with_suspend_funs", fun_interfaces_with_suspend_funs),
        ("companion_objects_implementing_interfaces", companion_objects_implementing_interfaces),
        ("sealed_classes_with_class_cases", sealed_classes_with_class_cases),
        (
            "inner_classes_of_generic_data_classes_and_with_cpp_bases",
            inner_classes_of_generic_data_classes_and_with_cpp_bases,
        ),
        ("classes_as_kotlin_writes_them", classes_as_kotlin_writes_them),
        ("hierarchy_forms_beyond_the_cpp_tests", hierarchy_forms_beyond_the_cpp_tests),
        ("generic_sealed_classes", generic_sealed_classes),
        ("api_bases_of_the_second_kind", api_bases_of_the_second_kind),
        ("object_members_and_secondary_constructors", object_members_and_secondary_constructors),
        ("closing_pass_classes_and_types", closing_pass_classes_and_types),
    ]);
}
