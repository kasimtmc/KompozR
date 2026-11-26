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
    ]);
}
