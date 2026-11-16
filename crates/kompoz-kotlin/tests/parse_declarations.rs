//! Declarations: the file's package, imports and annotations; functions, properties and their accessors, classes
//! of every kind with their members, objects, type aliases; annotations everywhere with use-site targets; types.

mod common;

use common::*;
use kompoz_kotlin::ast::*;

#[test]
fn package_imports_and_file_annotations() {
    let f = parse_ok(
        "#!/usr/bin/env kotlin\n@file:Suppress(\"UNUSED\")\n@file:[JvmName(\"Ui\") OptIn(X::class)]\npackage com.example.ui\n\n\
         import androidx.compose.runtime.*\nimport a.b.C\nimport a.b.D as E\n\nfun f() {}",
    );
    let anns: Vec<String> = f.annotations.iter().map(annotation).collect();
    assert_eq!(anns, ["@file:Suppress(\"UNUSED\")", "@file:JvmName(\"Ui\")", "@file:OptIn((:: X class))"]);
    assert_eq!(path(f.package.as_ref().unwrap()), "com.example.ui");
    let imports: Vec<(String, bool, Option<String>)> =
        f.imports.iter().map(|i| (path(&i.path), i.star, i.alias.as_ref().map(ident))).collect();
    assert_eq!(
        imports,
        [
            ("androidx.compose.runtime".into(), true, None),
            ("a.b.C".into(), false, None),
            ("a.b.D".into(), false, Some("E".into()))
        ]
    );
    assert_eq!(f.decls.len(), 1);
    // annotations without `file:` are the first declaration's
    let f = parse_ok("@Kompozable\nfun A() {}");
    assert!(f.annotations.is_empty());
}

#[test]
fn functions() {
    let cases = [
        ("fun f() {}", "(fun f() {})"),
        ("fun f(): Int = 1", "(fun f(): Int = 1)"),
        ("fun f()\n: Int\n= 1", "(fun f(): Int = 1)"),
        ("fun f()\n{\n}", "(fun f() {})"),
        (
            "@Kompozable\nprivate inline fun Labeled(label: String, value: Int = 7, content: @Kompozable () -> Unit,) {}",
            "(fun @Kompozable private inline Labeled(label: String, value: Int = 7, content: @Kompozable () -> Unit) {})",
        ),
        ("fun sum(vararg xs: Int): Int = xs.sum()", "(fun sum(vararg xs: Int): Int = (call (. xs sum)))"),
        (
            "inline fun f(noinline a: () -> Unit, crossinline b: () -> Unit) {}",
            "(fun inline f(noinline a: () -> Unit, crossinline b: () -> Unit) {})",
        ),
        ("fun <T> List<T>.second(): T = this[1]", "(fun <T> List<T>.second(): T = ([] this 1))"),
        (
            "fun Modifier.framed(color: Color): Modifier =\n  border(1.dp, color)",
            "(fun Modifier.framed(color: Color): Modifier = (call border (. 1 dp) color))",
        ),
        ("fun String?.orEmpty() = this ?: \"\"", "(fun String?.orEmpty() = (?: this \"\"))"),
        ("fun String? .x() = 1", "(fun String?.x() = 1)"),
        ("fun a.b.C.d() {}", "(fun a.b.C.d() {})"),
        ("fun Map<K, V>.Entry<K, V>.k() = key", "(fun Map<K, V>.Entry<K, V>.k() = key)"),
        ("fun (() -> Unit).twice() { this(); this() }", "(fun (() -> Unit).twice() {(call this); (call this)})"),
        ("fun <T : Comparable<T>, reified R, out O> f() {}", "(fun <T: Comparable<T>, reified R, out O> f() {})"),
        ("fun <T> f(x: T) where T : A, T : B {}", "(fun <T> f(x: T) where T: A, T: B {})"),
        ("suspend fun load(): String", "(fun suspend load(): String)"),
        ("operator fun Vec.plus(o: Vec) = Vec(x + o.x)", "(fun operator Vec.plus(o: Vec) = (call Vec (+ x (. o x))))"),
        ("infix fun Int.pow(e: Int): Int = 1", "(fun infix Int.pow(e: Int): Int = 1)"),
        (
            "tailrec fun f(n: Int): Int = if (n == 0) 0 else f(n - 1)",
            "(fun tailrec f(n: Int): Int = (if (== n 0) 0 else (call f (- n 1))))",
        ),
        (
            "expect fun platform(): String\nactual fun other() = 1",
            "(fun expect platform(): String)\n(fun actual other() = 1)",
        ),
        ("fun `my test`() {}", "(fun `my test`() {})"),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
}

#[test]
fn properties_and_accessors() {
    let cases = [
        ("val x = 1", "(val x = 1)"),
        ("const val MAX: Int = 10", "(val const MAX: Int = 10)"),
        ("private var visits = 0", "(var private visits = 0)"),
        ("val greeting by lazy { \"hi\" }", "(val greeting by (call lazy {\"hi\"}))"),
        ("var shared by mutableStateOf(1)", "(var shared by (call mutableStateOf 1))"),
        ("val String.shout: String get() = this + \"!\"", "(val String.shout: String get() = (+ this \"!\"))"),
        ("val <T> List<T>.second: T get() = this[1]", "(val <T> List<T>.second: T get() = ([] this 1))"),
        ("val Vec2.norm get() = x * x", "(val Vec2.norm get() = (* x x))"),
        (
            "var Meter.doubled: Int\n    get() = value * 2\n    set(v) { value = v / 2 }",
            "(var Meter.doubled: Int get() = (* value 2) set(v) {(= value (/ v 2))})",
        ),
        ("var count = 0\n    private set", "(var count = 0 private set)"),
        (
            "var name: String = \"\"\n    set(value) {\n        field = value.trim()\n    }\n    get",
            "(var name: String = \"\" get set(value) {(= field (call (. value trim)))})",
        ),
        ("val size: Int\n    get(): Int {\n        return 1\n    }", "(val size: Int get(): Int {(return 1)})"),
        ("var x = 0\n    @Inject set", "(var x = 0 @Inject set)"),
        ("var y: Int = 0; private set", "(var y: Int = 0 private set)"),
        ("var x = 1\n    private set val y = 2", "(var x = 1 private set)\n(val y = 2)"),
        (
            "var s: String\n    set(value: String) = log(value)\n    get() = \"\"",
            "(var s: String get() = \"\" set(value: String) = (call log value))",
        ),
        ("lateinit var session: Session", "(var lateinit session: Session)"),
        ("val (a, b) = pair", "(val (a, b) = pair)"),
        ("val f: () -> Unit = {}", "(val f: () -> Unit = {})"),
        ("@get:JvmName(\"x\") @field:Volatile var x = 1", "(var @get:JvmName(\"x\") @field:Volatile x = 1)"),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
}

#[test]
fn classes() {
    let cases = [
        ("class A", "(class A)"),
        ("class A {}", "(class A {})"),
        (
            "data class Note(val id: Int, var title: String = \"\", val tags: List<String> = listOf(),)",
            "(class data Note(val id: Int, var title: String = \"\", val tags: List<String> = (call listOf)))",
        ),
        ("class Repeater<T>(private val value: T) : Feed<T>", "(class Repeater<T>(private val value: T) : Feed<T>)"),
        ("class A private constructor(x: Int)", "(class A private constructor(x: Int))"),
        ("class A @Inject constructor(val x: Int)", "(class A @Inject constructor(val x: Int))"),
        ("class B(x: Int) : Base(x), I, J by j", "(class B(x: Int) : Base(x), I, J by j)"),
        ("class C : I by impl {\n  fun f() = 1\n}", "(class C : I by impl {(fun f() = 1)})"),
        (
            "class F : (Int) -> Int { override fun invoke(x: Int) = x }",
            "(class F : (Int) -> Int {(fun override invoke(x: Int) = x)})",
        ),
        ("class S : suspend () -> Unit", "(class S : suspend () -> Unit)"),
        (
            "abstract class Plate(val label: String) {\n  abstract fun area(): Float\n  open val sides = 0\n}",
            "(class abstract Plate(val label: String) {(fun abstract area(): Float); (val open sides = 0)})",
        ),
        ("open class Holder<T>(val item: T)", "(class open Holder<T>(val item: T))"),
        ("class Box<T> where T : Any", "(class Box<T> where T: Any)"),
        (
            "class Outer {\n  inner class In {\n    fun f() = this@Outer\n  }\n  class Nested\n}",
            "(class Outer {(class inner In {(fun f() = this@Outer)}); (class Nested)})",
        ),
        ("@JvmInline value class Id(val raw: Int)", "(class @JvmInline value Id(val raw: Int))"),
        (
            "sealed class LoadState {\n  data object Loading : LoadState()\n  data class Done(val v: Int) : LoadState()\n}",
            "(class sealed LoadState {(object data Loading : LoadState()); (class data Done(val v: Int) : LoadState())})",
        ),
        ("sealed interface Figure", "(interface sealed Figure)"),
        (
            "interface Shelf : Titled {\n  val name: String\n  fun title(): String = name\n  fun count(): Int\n}",
            "(interface Shelf : Titled {(val name: String); (fun title(): String = name); (fun count(): Int)})",
        ),
        (
            "fun interface Check {\n  fun test(x: Int): Boolean\n}",
            "(fun interface Check {(fun test(x: Int): Boolean)})",
        ),
        ("fun interface Mapper<T> { fun map(x: T): T }", "(fun interface Mapper<T> {(fun map(x: T): T)})"),
        ("annotation class Marker(val why: String)", "(annotation class Marker(val why: String))"),
        (
            "class Counter(start: Int) {\n  var count = start\n  init {\n    count += 1\n  }\n  constructor() : this(0)\n  constructor(s: String) : this(s.length) {\n    log(s)\n  }\n}",
            "(class Counter(start: Int) {(var count = start); (init {(+= count 1)}); (constructor () : this(0)); (constructor (s: String) : this((. s length)) {(call log s)})})",
        ),
        (
            "class V : View {\n  constructor(c: Context) : super(c)\n}",
            "(class V : View {(constructor (c: Context) : super(c))})",
        ),
        (
            "class Ledger(val id: Int) {\n  companion object {\n    const val START = 0\n    fun open() = Ledger(START)\n  }\n}",
            "(class Ledger(val id: Int) {(object companion {(val const START = 0); (fun open() = (call Ledger START))})})",
        ),
        (
            "class W {\n  companion object Factory : Maker<W> { }\n}",
            "(class W {(object companion Factory : Maker<W> {})})",
        ),
        ("class X {\n  companion object\n  init {}\n}", "(class X {(object companion); (init {})})"),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
}

#[test]
fn enum_classes() {
    let cases = [
        ("enum class Suit { HEART, SPADE }", "(enum class Suit [HEART, SPADE] {})"),
        ("enum class Suit { HEART, SPADE, }", "(enum class Suit [HEART, SPADE] {})"),
        ("enum class Side {\n  Left,\n  Right\n}", "(enum class Side [Left, Right] {})"),
        (
            "enum class Rank(val n: Int, val label: String = \"seviye\") {\n  LOW(1), HIGH(9, label = \"yüksek\");\n  fun up() = n + 1\n}",
            "(enum class Rank(val n: Int, val label: String = \"seviye\") [LOW(1), HIGH(9 label=\"yüksek\")] {(fun up() = (+ n 1))})",
        ),
        (
            "enum class Turn { LEFT, RIGHT; fun flip() = if (this == LEFT) RIGHT else LEFT }",
            "(enum class Turn [LEFT, RIGHT] {(fun flip() = (if (== this LEFT) RIGHT else LEFT))})",
        ),
        (
            "enum class Calc(val symbol: String) {\n  PLUS(\"+\") {\n    override fun apply(a: Int, b: Int) = a + b\n  },\n  @Deprecated(\"x\") TIMES(\"*\") {\n    override fun apply(a: Int, b: Int) = a * b\n  };\n  abstract fun apply(a: Int, b: Int): Int\n}",
            "(enum class Calc(val symbol: String) [PLUS(\"+\") {(fun override apply(a: Int, b: Int) = (+ a b))}, @Deprecated(\"x\") TIMES(\"*\") {(fun override apply(a: Int, b: Int) = (* a b))}] {(fun abstract apply(a: Int, b: Int): Int)})",
        ),
        ("enum class E { ; fun f() = 1 }", "(enum class E {(fun f() = 1)})"),
        ("enum class Choice { System, Light, Dark }", "(enum class Choice [System, Light, Dark] {})"),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
}

#[test]
fn objects_and_type_aliases() {
    let cases = [
        (
            "object AppIcons { val Add = Icons.Default.Add }",
            "(object AppIcons {(val Add = (. (. Icons Default) Add))})",
        ),
        ("object Speck : Figure", "(object Speck : Figure)"),
        (
            "private data object Idle : Outcome(0) { override fun toString() = \"idle\" }",
            "(object private data Idle : Outcome(0) {(fun override toString() = \"idle\")})",
        ),
        ("typealias Names = List<String>", "(typealias Names = List<String>)"),
        ("typealias Pick = (String) -> Unit", "(typealias Pick = (String) -> Unit)"),
        ("typealias Cells<T> = List<List<T>>", "(typealias Cells<T> = List<List<T>>)"),
        (
            "private typealias Handler = suspend CoroutineScope.(Int) -> Unit",
            "(typealias private Handler = suspend CoroutineScope.(Int) -> Unit)",
        ),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
}

#[test]
fn annotations_everywhere() {
    let cases = [
        (
            "@Preview(widthDp = 400, showBackground = true) @Kompozable fun P() {}",
            "(fun @Preview(widthDp=400 showBackground=true) @Kompozable P() {})",
        ),
        ("@androidx.compose.runtime.Composable fun C() {}", "(fun @androidx.compose.runtime.Composable C() {})"),
        (
            "@OptIn(ExperimentalFoundationApi::class)\n@Composable\nfun L() {}",
            "(fun @OptIn((:: ExperimentalFoundationApi class)) @Composable L() {})",
        ),
        ("@Suppress(\"A\", \"B\") private @Volatile var v = 0", "(var @Suppress(\"A\" \"B\") @Volatile private v = 0)"),
        (
            "class A(@param:Named(\"x\") val x: Int, @Ann y: Int)",
            "(class A(@param:Named(\"x\") val x: Int, @Ann y: Int))",
        ),
        (
            "fun f(@Ann x: Int, content: @Kompozable RowScope.() -> Unit) {}",
            "(fun f(@Ann x: Int, content: @Kompozable RowScope.() -> Unit) {})",
        ),
        (
            "@Target(AnnotationTarget.FUNCTION) annotation class Fast",
            "(annotation class @Target((. AnnotationTarget FUNCTION)) Fast)",
        ),
        ("@set:[Inject Named(\"n\")] var n = 0", "(var @set:Inject @set:Named(\"n\") n = 0)"),
        ("@Ann<Int> fun g() {}", "(fun @Ann<Int> g() {})"),
        ("fun <@Ann T> h() {}", "(fun <@Ann T> h() {})"),
    ];
    for (text, tree) in cases {
        assert_eq!(decls(text), tree, "{text}");
    }
    // use-site targets are kept as written
    let f = parse_ok("@get:JvmName(\"x\") val x = 1");
    let Decl::Property(p) = &f.decls[0] else { panic!() };
    assert_eq!(p.annotations[0].use_site.as_ref().map(ident), Some("get".into()));
    assert_eq!(path(&p.annotations[0].name), "JvmName");
}

#[test]
fn types() {
    let ty_of = |text: &str| {
        let f = parse_ok(&format!("val x: {text}"));
        let Decl::Property(p) = &f.decls[0] else { panic!() };
        ty(p.ty.as_ref().unwrap())
    };
    let cases = [
        ("Int", "Int"),
        ("String?", "String?"),
        ("String??", "String?"),
        ("List<Item>", "List<Item>"),
        ("Map<String, List<Int?>>", "Map<String, List<Int?>>"),
        ("Map.Entry<K, V>", "Map.Entry<K, V>"),
        ("Outer<A>.Inner<B>", "Outer<A>.Inner<B>"),
        ("kotlin.collections.List<*>", "kotlin.collections.List<*>"),
        ("Array<out Number>", "Array<out Number>"),
        ("Comparator<in T>", "Comparator<in T>"),
        ("() -> Unit", "() -> Unit"),
        ("(String) -> Unit", "(String) -> Unit"),
        ("(index: Int, item: T) -> Boolean", "(index: Int, item: T) -> Boolean"),
        ("(Int) -> (Int) -> Int", "(Int) -> (Int) -> Int"),
        ("(() -> Unit)?", "(() -> Unit)?"),
        ("((Int) -> Unit)?", "((Int) -> Unit)?"),
        ("(Int)", "Int"),
        ("@Kompozable () -> Unit", "@Kompozable () -> Unit"),
        ("RowScope.() -> Unit", "RowScope.() -> Unit"),
        ("@Kompozable BoxScope.(Int) -> Unit", "@Kompozable BoxScope.(Int) -> Unit"),
        ("String?.() -> Int", "String?.() -> Int"),
        ("suspend () -> Unit", "suspend () -> Unit"),
        ("suspend CoroutineScope.() -> Unit", "suspend CoroutineScope.() -> Unit"),
        ("suspend PointerInputScope.(Offset) -> Unit", "suspend PointerInputScope.(Offset) -> Unit"),
        ("List<@Kompozable () -> Unit>", "List<@Kompozable () -> Unit>"),
        ("dynamic", "dynamic"),
    ];
    for (text, tree) in cases {
        assert_eq!(ty_of(text), tree, "{text}");
    }
}

#[test]
fn positions_of_declarations() {
    let text = "/** Doc. */\n@Kompozable\nfun Greeting(name: String) {\n    Text(\"Merhaba $name\")\n}\nval ğ = 1";
    let f = parse_ok(text);
    let Decl::Function(g) = &f.decls[0] else { panic!() };
    // a declaration from its first annotation to its end
    assert_eq!(g.loc.text(text), "@Kompozable\nfun Greeting(name: String) {\n    Text(\"Merhaba $name\")\n}");
    assert_eq!(g.name.loc.text(text), "Greeting");
    assert_eq!((g.name.loc.start.line, g.name.loc.start.column), (3, 5));
    assert_eq!(g.params[0].loc.text(text), "name: String");
    let Some(FunctionBody::Block(b)) = &g.body else { panic!() };
    assert_eq!(b.stmts[0].loc.text(text), "Text(\"Merhaba $name\")");
    let Decl::Property(p) = &f.decls[1] else { panic!() };
    assert_eq!((p.loc.start.line, p.loc.start.column, p.loc.end.column), (6, 1, 10));
}
