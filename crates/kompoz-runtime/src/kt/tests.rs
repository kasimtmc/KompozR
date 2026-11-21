//! The kt shims against what Kotlin gives (each expectation is Kotlin/JVM's output).

use super::*;
use crate::{mutable_state_list_of, mutable_state_of};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The panic message of `f` (a Kotlin exception's text), the panic not printed (the hook swapped by one test at a
/// time: they run on threads of their own).
fn thrown(f: impl FnOnce()) -> String {
    static HOOK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = HOOK.lock().unwrap_or_else(|e| e.into_inner());
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    match r {
        Ok(()) => panic!("nothing was thrown"),
        Err(e) => e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    }
}

#[test]
fn numbers_as_kotlin_writes_them() {
    assert_eq!(to_string(&3.0f32), "3.0");
    assert_eq!(to_string(&2.5f32), "2.5");
    assert_eq!(to_string(&0.1f32), "0.1");
    assert_eq!(to_string(&0.1f64), "0.1");
    assert_eq!(to_string(&(0.1f64 + 0.2)), "0.30000000000000004");
    assert_eq!(to_string(&100.0f64), "100.0");
    assert_eq!(to_string(&1e7f64), "1.0E7");
    assert_eq!(to_string(&1.5e10f64), "1.5E10");
    assert_eq!(to_string(&9999999.0f64), "9999999.0");
    assert_eq!(to_string(&0.001f64), "0.001");
    assert_eq!(to_string(&0.0001f64), "1.0E-4");
    assert_eq!(to_string(&1.25e-5f32), "1.25E-5");
    assert_eq!(to_string(&-0.0f64), "-0.0");
    assert_eq!(to_string(&-2.0f32), "-2.0");
    assert_eq!(to_string(&f64::NAN), "NaN");
    assert_eq!(to_string(&f32::NEG_INFINITY), "-Infinity");
    assert_eq!(to_string(&f32::MAX), "3.4028235E38");
    assert_eq!(to_string(&42), "42");
    assert_eq!(to_string(&-7i64), "-7");
    assert_eq!(to_string(&'ğ'), "ğ");
    assert_eq!(to_string(&true), "true");
    assert_eq!(to_string(&()), "kotlin.Unit");
}

#[test]
fn templates_nulls_and_collections() {
    let ratio = 2.5f32;
    let whole = 3f32;
    let name = "Kompoz";
    assert_eq!(template!(name.length(), " harf, ", ratio, ", ", whole, ", ", ratio * 2.0), "6 harf, 2.5, 3.0, 5.0");
    let none: Option<i32> = None;
    assert_eq!(template!("x=", none, " y=", Some(1.0f64)), "x=null y=1.0");
    assert_eq!(template!(Some(None::<i32>)), "null");
    assert_eq!(template!(vec![1, 2, 3]), "[1, 2, 3]");
    assert_eq!(template!(vec!["a".to_string(), "b".to_string()]), "[a, b]");
    assert_eq!(template!(vec![Some(1.0f32), None]), "[1.0, null]");
    assert_eq!(template!(map_of([("b", 1), ("a", 2)])), "{b=1, a=2}");
    assert_eq!(template!(set_of([3, 1, 3])), "[3, 1]");
    assert_eq!(template!(("x", 1)), "(x, 1)");
    assert_eq!(template!((1, 2.0f32, 'c')), "(1, 2.0, c)");
    assert_eq!(template!(Vec::<i32>::new()), "[]");
    // (a Display type of another crate: its Display, in a template; in a list through KtDisplay)
    struct Other;
    impl std::fmt::Display for Other {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("other")
        }
    }
    assert_eq!(template!("<", Other, ">"), "<other>");
    assert_eq!(template!(vec![KtDisplay(Other)]), "[other]");
}

#[test]
fn ranges_and_progressions() {
    let v: Vec<i32> = until(0, 4i32).into_iter().collect();
    assert_eq!(v, [0, 1, 2, 3]);
    assert!(until(0, 0i32).is_empty());
    assert_eq!(to_string(&until(0, 0)), "0..-1");
    assert_eq!(to_string(&range_to(1, 10)), "1..10");
    let back: Vec<i32> = step(down_to(10, 8), 2).into_iter().collect();
    assert_eq!(back, [10, 8]);
    assert_eq!(to_string(&down_to(10, 1)), "10 downTo 1 step 1");
    assert_eq!(to_string(&step(range_to(1, 10), 4)), "1..9 step 4");
    assert_eq!(step(range_to(1, 10), 4).last(), 9);
    assert_eq!(range_to(1, 4).reversed().into_iter().collect::<Vec<_>>(), [4, 3, 2, 1]);
    assert!(is_in(&4, &range_to(3, 5)));
    assert!(!is_in(&6, &range_to(3, 5)));
    assert!(is_in(&5, &step(range_to(1, 10), 2)));
    assert!(!is_in(&4, &step(range_to(1, 10), 2)));
    assert!(is_in(&0.5, &range_to(0.0, 1.0)));
    assert!(!is_in(&1.0f32, &until(0.0f32, 1.0)));
    assert_eq!(range_to('a', 'e').into_iter().collect::<String>(), "abcde");
    assert_eq!(range_to(1i64, 3).count(), 3);
    assert_eq!(until(0, i32::MIN), KtProgression::range(1, 0));
    assert_eq!(range_to(1, 0), range_to(5, 2));
    assert!(
        thrown(|| {
            step(range_to(1, 3), 0);
        })
        .as_str()
        .starts_with("IllegalArgumentException: Step must be positive")
    );
    let indexed: Vec<(i32, &str)> =
        vec!["ayla", "ali"].with_index().into_iter().map(|iv| (iv.component1(), iv.component2())).collect();
    assert_eq!(indexed, [(0, "ayla"), (1, "ali")]);
    assert_eq!(template!(vec!["a"].with_index()), "[IndexedValue(index=0, value=a)]");
    assert_eq!(vec![1, 2, 3].indices().into_iter().collect::<Vec<_>>(), [0, 1, 2]);
    assert!(is_in(&"ab", "cabd"));
    assert!(is_in(&'x', &"xyz".to_string()));
    assert!(is_in(&2, &vec![1, 2]));
    assert!(is_in(&"k", &map_of([("k", 1)])));
}

#[test]
fn strings_in_utf16_units() {
    assert_eq!("Kompoz".length(), 6);
    assert_eq!("😀a".length(), 3);
    assert_eq!(KtStr::get(&"a😀b", 1), '😀');
    assert_eq!(KtStr::get(&"a😀b", 2), '\u{FFFD}');
    assert_eq!(KtStr::get(&"a😀b", 3), 'b');
    assert_eq!(KtStr::substring(&"a😀b", 1, Some(3)), "😀");
    assert_eq!(KtStr::substring(&"a😀b", 0, Some(2)), "a\u{FFFD}");
    assert_eq!(KtStr::substring(&"hello", 1, None), "ello");
    assert!(
        thrown(|| {
            KtStr::substring(&"abc", 2, Some(5));
        })
        .as_str()
        .starts_with("StringIndexOutOfBoundsException: begin 2, end 5, length 3")
    );
    assert_eq!("çağrı".uppercase(), "ÇAĞRI");
    assert_eq!("İ".lowercase(), "i\u{307}");
    assert_eq!(KtStr::trim(&"\u{1C} a \u{3000}"), "a");
    assert!(KtStr::is_blank(&" \t\u{A0}"));
    assert!("x".is_not_blank());
    let text = "\n            bir\n              iki\n        ";
    assert_eq!(text.trim_indent(), "bir\n  iki");
    assert_eq!("\n    |a\n    |  b\n    c\n".trim_margin("|"), "a\n  b\n    c");
    assert_eq!(KtStr::split(&"a,b,,c,", ",", false, 0), ["a", "b", "", "c", ""]);
    assert_eq!(KtStr::split(&"abc", "", false, 0), ["", "a", "b", "c", ""]);
    assert_eq!(KtStr::split(&"a-b_c", ["-", "_"], false, 0), ["a", "b", "c"]);
    assert_eq!(KtStr::split(&"a,b,c", ',', false, 2), ["a", "b,c"]);
    assert_eq!(KtStr::split(&"aXbxc", "x", true, 0), ["a", "b", "c"]);
    assert!(KtStr::starts_with(&"Kotlin", "kot", true));
    assert!(!KtStr::starts_with(&"Kotlin", "kot", false));
    assert!(KtStr::contains(&"Kotlin", 'l', false));
    assert_eq!(KtStr::replace(&"a.b.c", ".", "-", false), "a-b-c");
    assert_eq!(KtStr::replace(&"AbAb", "ab", "x", true), "xx");
    assert_eq!(KtStr::replace(&"ab", "", "-", false), "-a-b-");
    assert_eq!(KtStr::index_of(&"a😀b😀", "😀", 2, false), 4);
    assert_eq!(KtStr::last_index_of(&"abcabc", "b", None, false), 4);
    assert_eq!(KtStr::last_index_of(&"abcabc", "b", Some(3), false), 1);
    assert_eq!("7".pad_start(3, '0'), "007");
    assert_eq!("ab".pad_end(4, ' '), "ab  ");
    assert_eq!(KtStr::repeat(&"ab", 3), "ababab");
    assert_eq!("a😀b".reversed(), "b😀a");
    assert_eq!(KtStr::take(&"kotlin", 3), "kot");
    assert_eq!(KtStr::drop(&"kotlin", 4), "in");
    assert_eq!("k:v:w".substring_before(":", None), "k");
    assert_eq!("k:v:w".substring_after_last(":", None), "w");
    assert_eq!("kvw".substring_after(":", Some("yok")), "yok");
    assert_eq!(KtStr::filter(&"a1b2", KtChar::is_digit), "12");
    assert_eq!(KtStr::lines(&"a\r\nb\rc\n"), ["a", "b", "c", ""]);
    assert_eq!(None::<String>.or_empty(), "");
    assert!(Some("").is_null_or_empty());
    assert!(KtChar::is_whitespace('\u{1F}'));
    assert!(!KtChar::is_whitespace('\u{85}'));
    assert!(KtChar::is_digit('٣'));
    assert_eq!('٣'.digit_to_int(), 3);
    assert!(!KtChar::is_digit('½'));
    assert!('ş'.is_letter());
    assert!('A'.is_upper_case());
    assert_eq!('a'.code(), 97);
    assert_eq!('ß'.uppercase(), "SS");
    assert_eq!('ß'.uppercase_char(), 'ß');
}

#[test]
fn numbers_from_text() {
    assert_eq!("42".to_int(), 42);
    assert_eq!("+42".to_int(), 42);
    assert_eq!("-2147483648".to_int(), i32::MIN);
    assert_eq!("2147483648".to_int_or_null(), None);
    assert_eq!(" 1".to_int_or_null(), None);
    assert_eq!("".to_int_or_null(), None);
    assert_eq!("-".to_int_or_null(), None);
    assert_eq!("١٢".to_int(), 12);
    assert_eq!("ff".to_int_radix(16), 255);
    assert_eq!("9223372036854775807".to_long(), i64::MAX);
    assert_eq!(" 1.5 ".to_double(), 1.5);
    assert_eq!("1.5f".to_float(), 1.5);
    assert_eq!("1e3".to_double(), 1000.0);
    assert_eq!(".5".to_double(), 0.5);
    assert!("NaN".to_double().is_nan());
    assert_eq!("-Infinity".to_float(), f32::NEG_INFINITY);
    assert_eq!("inf".to_double_or_null(), None);
    assert_eq!("1.".to_double_or_null(), Some(1.0));
    assert_eq!(".".to_double_or_null(), None);
    assert_eq!("1e".to_double_or_null(), None);
    assert!("TRUE".to_boolean());
    assert_eq!(
        thrown(|| {
            let _ = "x1".to_int();
        }),
        "NumberFormatException: For input string: \"x1\""
    );
}

#[test]
fn string_format_as_java() {
    let args: [&dyn KtToString; 4] = [&1.25f32, &7, &"ab", &255];
    assert_eq!("%.1f km · %03d dk · %-4s| · %x".format(&args), "1.3 km · 007 dk · ab  | · ff");
    assert_eq!(format("%.1f", &[&0.15]), "0.2");
    assert_eq!(format("%.2f", &[&1.005]), "1.01");
    assert_eq!(format("%.0f", &[&0.5]), "1");
    assert_eq!(format("%.3f", &[&0.0005]), "0.001");
    assert_eq!(format("%f", &[&1.0f64]), "1.000000");
    assert_eq!(format("%5.1f|", &[&-2.25]), " -2.3|");
    assert_eq!(format("%,d", &[&1234567]), "1,234,567");
    assert_eq!(format("%,.2f", &[&1234567.891]), "1,234,567.89");
    assert_eq!(format("%+d %+d", &[&5, &-5]), "+5 -5");
    assert_eq!(format("%(d", &[&-5]), "(5)");
    assert_eq!(format("%x %X", &[&-1, &-1i64]), "ffffffff FFFFFFFFFFFFFFFF");
    assert_eq!(format("%#x %o", &[&255, &8]), "0xff 10");
    assert_eq!(format("%08.2f", &[&-3.25159]), "-0003.25");
    assert_eq!(format("%e", &[&12345.678]), "1.234568e+04");
    assert_eq!(format("%.2E", &[&0.000123]), "1.23E-04");
    assert_eq!(format("%g", &[&0.0001234]), "0.000123400");
    assert_eq!(format("%g", &[&123456789.0]), "1.23457e+08");
    assert_eq!(format("%s %S %b %b %c %c", &[&"a", &"b", &None::<i32>, &3, &'ğ', &65]), "a B false true ğ A");
    assert_eq!(format("%2$s %1$s %<s", &[&"x", &"y"]), "y x x");
    assert_eq!(format("100%% %n", &[]), "100% \n");
    assert_eq!(format("%.2s|%5s|", &[&"kotlin", &"ab"]), "ko|   ab|");
    assert_eq!(format("%s", &[&vec![1, 2]]), "[1, 2]");
    assert_eq!(
        thrown(|| {
            let _ = format("%d", &[&1.5]);
        }),
        "IllegalFormatConversionException: d != java.lang.Double"
    );
    assert_eq!(
        thrown(|| {
            let _ = format("%s %s", &[&1]);
        }),
        "MissingFormatArgumentException: Format specifier '%s'"
    );
}

#[test]
fn collection_functions() {
    let names = list_of(["ayla", "ali", "veli"]);
    let upper: Vec<String> = names.filter(|it| it.starts_with('a')).map(|it| it.uppercase());
    assert_eq!(upper, ["AYLA", "ALI"]);
    let lengths = names.map(|it| it.length());
    assert_eq!(lengths.join_to_string("+", "", "", -1, "...", None), "4+3+4");
    assert_eq!(names.join_to_string(", ", "[", "]", 2, "…", None), "[ayla, ali, …]");
    assert_eq!(names.join_to_string("/", "", "", -1, "...", Some(&|s: &str| s.take(1))), "a/a/v");
    assert_eq!(KtList::first(&names), "ayla");
    assert_eq!(names.last_that(|it| it.starts_with('a')), "ali");
    assert_eq!(names.first_or_null_that(|it| it.length() > 9), None);
    assert_eq!(
        thrown(|| {
            let _ = KtList::first(&Vec::<i32>::new());
        }),
        "NoSuchElementException: List is empty."
    );
    assert_eq!(names.size(), 3);
    assert_eq!(names.get_or_null(5), None);
    assert_eq!(names.get_or_else(5, |i| if i == 5 { "beş" } else { "" }), "beş");
    assert_eq!(
        thrown(|| {
            let _ = KtList::get(&names, 3);
        }),
        "IndexOutOfBoundsException: Index 3 out of bounds for length 3"
    );
    let words = list_of(["elma", "armut", "erik", "ayva", "kiraz"]);
    let (a, rest) = words.partition(|it| it.starts_with('a'));
    assert_eq!((a.join_to_string(", ", "", "", -1, "...", None), rest.size()), ("armut, ayva".to_string(), 3));
    let by_letter = words.group_by(|it| it.first());
    assert_eq!(template!(by_letter), "{e=[elma, erik], a=[armut, ayva], k=[kiraz]}");
    assert_eq!(words.max_by_or_null(|it| it.length()), Some("armut"));
    assert_eq!(words.index_of_first(|it| it.starts_with('e')), 0);
    assert_eq!(words.index_of_last(|it| it.starts_with('e')), 2);
    assert_eq!(KtList::first(&words.sorted_by_descending(|it| it.length())), "armut");
    assert_eq!(words.chunked(2).size(), 3);
    assert_eq!(words.distinct_by(|it| it.first()).size(), 3);
    assert_eq!(words.associate_with(|it| it.length()).size(), 5);
    assert_eq!(list_of([list_of([1, 2]), list_of([3])]).flat_map(|it| it).sum(), 6);
    assert_eq!(vec![3, 1, 2].sorted(), [1, 2, 3]);
    assert_eq!(vec![1.0, f64::NAN, 0.5].sorted().iter().map(to_string).collect::<Vec<_>>(), ["0.5", "1.0", "NaN"]);
    assert_eq!(vec![2.0f32, 5.0].max_of(|it| it), 5.0);
    assert_eq!(vec![1, 2, 3].sum_of(|it| it * 10), 60);
    assert_eq!(vec![i32::MAX, 1].sum(), i32::MIN);
    assert!(vec![1, 2].any_that(|it| it > 1) && vec![1, 2].all(|it| it > 0) && vec![1, 2].none_that(|it| it > 2));
    assert_eq!(vec![1, 2, 3, 4].count_that(|it| it % 2 == 0), 2);
    assert_eq!(vec![1, 2, 3, 4, 5].take(2), [1, 2]);
    assert_eq!(vec![1, 2, 3].drop(5), Vec::<i32>::new());
    assert_eq!(vec![1, 2].zip(&vec!["a", "b", "c"]), [(1, "a"), (2, "b")]);
    assert_eq!(vec![1, 2, 1].distinct(), [1, 2]);
    assert_eq!(vec![1, 2, 3].reversed(), [3, 2, 1]);
    assert_eq!(vec![Some(1), None, Some(3)].filter_not_null(), [1, 3]);
    assert_eq!(vec![vec![1], vec![2, 3]].flatten(), [1, 2, 3]);
    assert_eq!(vec![1, 2].plus(&vec![3]), [1, 2, 3]);
    assert_eq!(vec![1, 2, 1].minus_element(1), [2, 1]);
    assert_eq!(vec![1, 2, 3].windowed(2, 1, false), [vec![1, 2], vec![2, 3]]);
    assert_eq!(vec![1, 2, 3].fold(10, |acc, it| acc + it), 16);
    assert!(KtList::contains(&vec![1, 2], 2));
    let mut list = mutable_list_of([1, 2, 3, 4]);
    list.add(5);
    list.add_at(0, 0);
    assert!(KtMutableList::remove(&mut list, 3));
    assert_eq!(list.remove_at(0), 0);
    assert!(list.remove_all(|it| it % 2 == 0));
    assert_eq!(list, [1, 5]);
    assert_eq!(KtMutableList::set(&mut list, 1, 9), 5);
    assert_eq!(list, [1, 9]);
    KtMutableList::reverse(&mut list);
    assert_eq!(list, [9, 1]);
    let state = crate::mutable_state_list_of([1, 2, 3]);
    state.reverse();
    assert_eq!(state.to_vec(), [3, 2, 1]);
    assert_eq!(List(3, |i| i * i), [0, 1, 4]);
}

#[test]
fn maps_and_sets_keep_their_order() {
    let mut m = mutable_map_of([("b", 1), ("a", 2)]);
    m.set("c", 3);
    m.put("b", 10);
    assert_eq!(template!(m), "{b=10, a=2, c=3}");
    assert_eq!(m.get("a"), Some(2));
    assert_eq!(KtMapExt::get(&m, "z"), None);
    assert_eq!(template!(m.keys()), "[b, a, c]");
    assert_eq!(m.values(), [10, 2, 3]);
    assert_eq!(m.map(|(k, v)| template!(k, v)), ["b10", "a2", "c3"]);
    assert_eq!(template!(m.filter(|(_, v)| v > 2)), "{b=10, c=3}");
    assert_eq!(m, map_of([("c", 3), ("a", 2), ("b", 10)]));
    assert_eq!(
        thrown(|| {
            let _ = m.get_value("q");
        }),
        "NoSuchElementException: Key q is missing in the map."
    );
    let mut s = set_of([2, 1]);
    assert!(!s.add(2) && s.add(0));
    assert_eq!(template!(s), "[2, 1, 0]");
    assert_eq!(s, set_of([0, 1, 2]));
    assert_eq!(("x", 1).component2(), 1);
    assert_eq!(Pair(1, 'a').second(), 'a');
    assert_eq!(Triple(1, 2, 3).third(), 3);
    assert_eq!(template!(to("k", 1.5f32)), "(k, 1.5)");
}

#[test]
fn nullability_and_scope_functions() {
    let maybe: Option<String> = None;
    let some: Option<String> = Some("dört".to_string());
    assert_eq!(elvis(safe!(safe(maybe.clone(), |s| s.length()), |n| to_string(&n)), || "yok".to_string()), "yok");
    assert_eq!(template!("var: ", elvis(safe(some.clone(), |s| s.length()), || 0)), "var: 4");
    // (a nullable result flattened: Option<Option<i32>> is one nullable)
    let nested: Option<i32> = safe!(some.clone(), |s| s.to_int_or_null());
    assert_eq!(nested, None);
    let plain: Option<i32> = safe!(Some("12"), |s| s.length());
    assert_eq!(plain, Some(2));
    assert_eq!(flatten!(Some(Some(3))), Some(3));
    // (a ?: b with b nullable: nullable)
    let either: Option<i32> = elvis(None::<i32>, || Some(5));
    assert_eq!(either, Some(5));
    assert_eq!(not_null(Some(1)), 1);
    assert!(
        thrown(|| {
            let _ = not_null(None::<i32>);
        })
        .as_str()
        .starts_with("NullPointerException")
    );
    assert_eq!(5.r#let(|it| it * 2), 10);
    let mut seen = 0;
    assert_eq!(5.also(|it| seen = it), 5);
    assert_eq!(seen, 5);
    assert_eq!(vec![1].apply(|v| v.push(2)), [1, 2]);
    assert_eq!(3.take_if(|it| it > 2), Some(3));
    assert_eq!(3.take_unless(|it| it > 2), None);
    assert_eq!(with(String::from("a"), |s| s.len()), 1);
    assert_eq!(run(|| 7), 7);
}

#[test]
fn errors_as_kotlin_throws_them() {
    assert_eq!(thrown(|| require(false, None)), "IllegalArgumentException: Failed requirement.");
    assert_eq!(thrown(|| check(1 > 2, Some(&|| template!("n=", 2)))), "IllegalStateException: n=2");
    assert_eq!(
        thrown(|| error("LocalAccount was not provided")),
        "IllegalStateException: LocalAccount was not provided"
    );
    assert_eq!(thrown(|| TODO(None)), "NotImplementedError: An operation is not implemented.");
    assert_eq!(require_not_null(Some(1), None), 1);
    require(true, None);
    let mut n = 0;
    repeat(3, |i| n += i);
    assert_eq!(n, 3);
}

#[test]
fn delegates_lazy_and_increments() {
    let count = mutable_state_of(0);
    assert_eq!(post_inc(&count), 0);
    assert_eq!(pre_inc(&count), 2);
    assert_eq!(count.get_value(), 2);
    KtMutableDelegate::set_value(&count, 7);
    assert_eq!(count.value(), 7);
    count.set_value(8);
    assert_eq!(post_dec(&count), 8);
    assert_eq!(count.component1(), 7);
    (count.component2())(1);
    assert_eq!(count.get(), 1);
    let made = std::rc::Rc::new(std::cell::Cell::new(0));
    let m = made.clone();
    let lazy_value = lazy(move || {
        m.set(m.get() + 1);
        "x".to_string()
    });
    assert!(!lazy_value.is_initialized());
    assert_eq!(lazy_value.get_value(), "x");
    assert_eq!(lazy_value.clone().get_value(), "x");
    assert_eq!(made.get(), 1);
    let late: KtLateinit<i32> = KtLateinit::new("name");
    assert_eq!(
        thrown(|| {
            let _ = late.get_value();
        }),
        "UninitializedPropertyAccessException: lateinit property name has not been initialized"
    );
    late.set_value(3);
    assert_eq!(late.get_value(), 3);
    assert_eq!('a'.inc(), 'b');
    assert_eq!(i32::MAX.inc(), i32::MIN);
    assert!(template!(count).as_str().starts_with("MutableState(value=1)@"));
    let tags = mutable_state_list_of(["a", "b"]);
    assert!(tags.remove_if(|t| t == "a"));
    assert_eq!(tags.size(), 1);
    assert!(template!(tags).as_str().starts_with("SnapshotStateList(value=[b])@"));
}

#[test]
fn numbers_and_math() {
    assert_eq!(2.5f32.round_to_int(), 3);
    assert_eq!((-2.5f32).round_to_int(), -2);
    assert_eq!(1e20f64.round_to_int(), i32::MAX);
    assert!(
        thrown(|| {
            let _ = f32::NAN.round_to_int();
        })
        .as_str()
        .starts_with("IllegalArgumentException: Cannot round NaN")
    );
    assert_eq!(round(2.5f64), 2.0);
    assert_eq!(round(3.5f32), 4.0);
    assert_eq!(5.coerce_in(0, 3), 3);
    assert_eq!((-1.5f32).coerce_at_least(0.0), 0.0);
    assert_eq!(3.9f32.to_int(), 3);
    assert_eq!(f32::NAN.to_int(), 0);
    assert!(max_of(1.0, f64::NAN).is_nan());
    assert_eq!(min_of(2, 1), 1);
    assert_eq!(abs(i32::MIN), i32::MIN);
    assert_eq!(sqrt(16f32), 4.0f32);
    assert!((sin(PI / 2.0) - 1.0).abs() < 1e-12);
    assert_eq!(sign(-3.0f32), -1.0);
    assert!((2f32.pow(3.0) - 8.0).abs() < 1e-5); // (Miri adds its noise to powf)
    assert_eq!(to_char(65), 'A');
}

thread_local! {
    static GREETING: String = String::from("selam");
}

const STEPS: i32 = 3;

#[test]
fn top_level_values_read_by_name() {
    // (a thread-local's value cloned out of it, any other value cloned)
    assert_eq!(top_level!(GREETING), "selam");
    assert_eq!(top_level!(STEPS) * 2, 6);
    let names = vec!["a".to_string()];
    let copy: Vec<String> = top_level!(names);
    assert_eq!((copy, names.len()), (vec!["a".to_string()], 1));
    // (lent where it is: a thread-local's inside it, a value's in place, whether it clones or not)
    assert_eq!(top_level_with!(GREETING, |g| g.len()), 5);
    struct NotClone(i32);
    static PLACE: NotClone = NotClone(7);
    assert_eq!(top_level_with!(PLACE, |p| p.0), 7);
}

/// `f` run to its end (a no-op waker: what it waits on is a [`Yield`], ready when polled again).
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// A suspension point: pending once, then ready (each lambda below suspends there, as a `delay` would).
struct Yield(bool);
impl std::future::Future for Yield {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.0 {
            std::task::Poll::Ready(())
        } else {
            self.0 = true;
            std::task::Poll::Pending
        }
    }
}
fn suspend() -> Yield {
    Yield(false)
}

#[test]
fn suspending_twins_do_what_their_shims_do() {
    let xs = vec![3, 1, 4, 1, 5];
    block_on(async {
        // (the elements each lambda sees, in order: a twin calls it as its shim does — as often, as far)
        let mut seen = Vec::new();
        assert_eq!(
            xs.map_async(async |x| {
                suspend().await;
                x * 2
            })
            .await,
            KtList::map(&xs, |x| x * 2)
        );
        assert_eq!(
            xs.filter_async(async |x| {
                suspend().await;
                x > 1
            })
            .await,
            KtList::filter(&xs, |x| x > 1)
        );
        assert_eq!(
            xs.fold_async(0, async |a, x| {
                suspend().await;
                a * 10 + x
            })
            .await,
            xs.fold(0, |a, x| a * 10 + x)
        );
        assert_eq!(
            xs.reduce_async(async |a, x| {
                suspend().await;
                a - x
            })
            .await,
            KtList::reduce(&xs, |a, x| a - x)
        );
        assert_eq!(
            xs.sum_of_async(async |x| {
                suspend().await;
                x
            })
            .await,
            14
        );
        assert_eq!(
            xs.max_by_async(async |x| {
                suspend().await;
                -x
            })
            .await,
            KtList::max_by(&xs, |x| -x)
        );
        assert_eq!(
            xs.min_of_async(async |x| {
                suspend().await;
                x
            })
            .await,
            1
        );
        assert_eq!(
            xs.group_by_async(async |x| {
                suspend().await;
                x % 2
            })
            .await,
            xs.group_by(|x| x % 2)
        );
        assert_eq!(
            xs.partition_async(async |x| {
                suspend().await;
                x > 2
            })
            .await,
            KtList::partition(&xs, |x| x > 2)
        );
        assert_eq!(
            xs.distinct_by_async(async |x| {
                suspend().await;
                x
            })
            .await,
            xs.distinct_by(|x| x)
        );
        assert_eq!(
            xs.map_not_null_async(async |x| {
                suspend().await;
                (x > 1).then_some(x)
            })
            .await,
            [3, 4, 5]
        );
        assert_eq!(
            xs.index_of_last_async(async |x| {
                suspend().await;
                x == 1
            })
            .await,
            3
        );
        assert_eq!(
            xs.associate_by_async(async |x| {
                suspend().await;
                x
            })
            .await,
            xs.associate_by(|x| x)
        );
        // (short-circuits where the shim does)
        assert!(
            xs.any_that_async(async |x| {
                seen.push(x);
                suspend().await;
                x == 4
            })
            .await
        );
        assert_eq!(std::mem::take(&mut seen), [3, 1, 4]);
        assert!(
            !xs.all_async(async |x| {
                seen.push(x);
                suspend().await;
                x > 2
            })
            .await
        );
        assert_eq!(std::mem::take(&mut seen), [3, 1]);
        assert_eq!(
            xs.take_while_async(async |x| {
                seen.push(x);
                suspend().await;
                x > 2
            })
            .await,
            [3]
        );
        assert_eq!(
            xs.drop_while_async(async |x| {
                seen.push(x);
                suspend().await;
                x > 2
            })
            .await,
            [1, 4, 1, 5]
        );
        assert_eq!(std::mem::take(&mut seen), [3, 1, 3, 1]);
        assert_eq!(
            xs.first_or_null_that_async(async |x| {
                seen.push(x);
                suspend().await;
                x > 3
            })
            .await,
            Some(4)
        );
        assert_eq!(std::mem::take(&mut seen), [3, 1, 4]);
        xs.for_each_indexed_async(async |i, x| {
            suspend().await;
            seen.push(i * 10 + x)
        })
        .await;
        assert_eq!(std::mem::take(&mut seen), [3, 11, 24, 31, 45]);
        // (strings by characters, UTF-16 indices; maps by entries)
        let s = "aé𝄞b";
        assert_eq!(
            s.filter_async(async |c| {
                suspend().await;
                c != 'é'
            })
            .await,
            KtStr::filter(&s, |c| c != 'é')
        );
        s.for_each_indexed_async(async |i, _| {
            suspend().await;
            seen.push(i)
        })
        .await;
        assert_eq!(std::mem::take(&mut seen), [0, 1, 2, 4]);
        assert_eq!(s.if_empty_async(async || unreachable!()).await, s);
        let m = map_of([to("a", 1), to("b", 2)]);
        assert_eq!(
            m.map_values_async(async |(_, v)| {
                suspend().await;
                v * 3
            })
            .await,
            m.map_values(|(_, v)| v * 3)
        );
        assert_eq!(
            m.filter_keys_async(async |k| {
                suspend().await;
                k == "b"
            })
            .await,
            m.filter_keys(|k| k == "b")
        );
        let mut cache: KtMap<String, i32> = KtMap::new();
        assert_eq!(
            cache
                .get_or_put_async("k".into(), async || {
                    suspend().await;
                    7
                })
                .await,
            7
        );
        assert_eq!(cache.get_or_put_async("k".into(), async || unreachable!()).await, 7);
        // (scope functions, `run`, `with`, `repeat`, `List(n)`, the safe call, `?:`)
        assert_eq!(
            5.let_async(async |x| {
                suspend().await;
                x + 1
            })
            .await,
            6
        );
        assert_eq!(
            vec![1]
                .apply_async(async |v| {
                    suspend().await;
                    v.push(2)
                })
                .await,
            [1, 2]
        );
        assert_eq!(
            with_async(String::from("a"), async |s| {
                suspend().await;
                s.push('b');
                s.clone()
            })
            .await,
            "ab"
        );
        assert_eq!(
            run_async(async || {
                suspend().await;
                3
            })
            .await,
            3
        );
        assert_eq!(
            4.take_if_async(async |x| {
                suspend().await;
                x > 3
            })
            .await,
            Some(4)
        );
        assert_eq!(
            4.take_unless_async(async |x| {
                suspend().await;
                x > 3
            })
            .await,
            None
        );
        repeat_async(3, async |i| {
            suspend().await;
            seen.push(i)
        })
        .await;
        assert_eq!(std::mem::take(&mut seen), [0, 1, 2]);
        assert_eq!(
            List_async(3, async |i| {
                suspend().await;
                i * i
            })
            .await,
            [0, 1, 4]
        );
        assert_eq!(
            safe_async(Some(2), async |x| {
                suspend().await;
                x * 2
            })
            .await,
            Some(4)
        );
        assert_eq!(safe_async(None::<i32>, async |_| -> i32 { unreachable!() }).await, None);
        assert_eq!(
            elvis_async(None, async || {
                suspend().await;
                9
            })
            .await,
            9
        );
        assert_eq!(elvis_async(Some(1), async || -> i32 { unreachable!() }).await, 1);
        assert_eq!(elvis_async(None::<i32>, async || Some(2)).await, Some(2));
    });
    // Kotlin's exceptions, as their shims throw them.
    assert_eq!(
        thrown(|| {
            block_on(xs.first_that_async(async |x| x > 9));
        }),
        "NoSuchElementException: Collection contains no element matching the predicate."
    );
    assert_eq!(
        thrown(|| {
            block_on(Vec::<i32>::new().reduce_async(async |a, b| a + b));
        }),
        "UnsupportedOperationException: Empty collection can't be reduced."
    );
    assert_eq!(
        thrown(|| block_on(require_async(
            false,
            Some(&async || {
                suspend().await;
                "no".to_string()
            })
        ))),
        "IllegalArgumentException: no"
    );
    assert_eq!(
        thrown(|| {
            block_on(check_not_null_async(None::<i32>, Some(&async || "gone".to_string())));
        }),
        "IllegalStateException: gone"
    );
    block_on(check_async(true, Some(&async || -> String { unreachable!() })));
}

#[test]
fn composes_list_helpers_as_ui_util_has_them() {
    // (androidx.compose.ui.util's ListUtils.kt: Kotlin's own functions by index, with Compose's results)
    let xs = vec![3, 1, 4, 1, 5];
    let mut seen = Vec::new();
    xs.fast_for_each(|x| seen.push(x));
    xs.fast_for_each_reversed(|x| seen.push(x));
    xs.fast_for_each_indexed(|i, x| seen.push(i * 10 + x));
    assert_eq!(seen, [3, 1, 4, 1, 5, 5, 1, 4, 1, 3, 3, 11, 24, 31, 45]);
    assert!(xs.fast_all(|x| x > 0) && !xs.fast_any(|x| x > 5));
    assert_eq!((xs.fast_first_or_null(|x| x > 3), xs.fast_last_or_null(|x| x < 2)), (Some(4), Some(1)));
    assert_eq!(xs.fast_sum_by(|x| x * 2), 28);
    assert_eq!(xs.fast_map(|x| x + 1), [4, 2, 5, 2, 6]);
    // (fastMaxBy and fastMinByOrNull: the first of the greatest, the first of the least; null for none)
    let pairs = vec![(1, 'a'), (3, 'b'), (3, 'c'), (1, 'd')];
    assert_eq!(pairs.fast_max_by(|p| p.0), Some((3, 'b')));
    assert_eq!(pairs.fast_min_by_or_null(|p| p.0), Some((1, 'a')));
    assert_eq!(Vec::<i32>::new().fast_max_by(|x| x), None);
    let mut out = vec![0];
    assert_eq!(*xs.fast_map_to(&mut out, |x| x * x), [0, 9, 1, 16, 1, 25]);
    assert_eq!(xs.fast_filter(|x| x != 1), [3, 4, 5]);
    assert_eq!(xs.fast_filtered_map(|x| x > 2, |x| x * 100), [300, 400, 500]);
    assert_eq!(xs.fast_fold(0, |a, x| a * 10 + x), 31415);
    assert_eq!(xs.fast_map_indexed(|i, x| i + x), [3, 2, 6, 4, 9]);
    assert_eq!(xs.fast_map_indexed_not_null(|i, x| (i % 2 == 0).then_some(x)), [3, 4, 5]);
    assert_eq!((xs.fast_max_of_or_null(|x| -x), Vec::<i32>::new().fast_max_of_or_null(|x| x)), (Some(-1), None));
    assert_eq!((xs.fast_max_of_or_default(0, |x| x), Vec::<i32>::new().fast_max_of_or_default(7, |x| x)), (5, 7));
    assert_eq!(xs.fast_zip_with_next(|a, b| b - a), [-2, 3, -3, 4]);
    assert!(vec![1].fast_zip_with_next(|a, b| a + b).is_empty());
    assert_eq!(xs.fast_reduce(|a, x| a.max(x)), 5);
    assert_eq!(xs.fast_zip(&["a", "b"], |x, s| format!("{s}{x}")), ["a3", "b1"]);
    assert_eq!(xs.fast_map_not_null(|x| (x > 3).then_some(x)), [4, 5]);
    assert_eq!(xs.fast_join_to_string(", ", "[", "]", 3, "...", None), "[3, 1, 4, ...]");
    assert_eq!(xs.fast_distinct_by(|x| x % 3), [3, 1, 5]);
    assert_eq!(xs.fast_flat_map(|x| vec![x; (x % 2) as usize]), [3, 1, 1, 5]);
    assert_eq!(vec![Some(1), None, Some(2)].fast_filter_not_null(), [1, 2]);
    assert_eq!(xs.fast_first(|x| x > 3), 4);
    assert_eq!(
        thrown(|| {
            xs.fast_first(|x| x > 9);
        }),
        "NoSuchElementException: Collection contains no element matching the predicate."
    );
    assert_eq!(
        thrown(|| {
            Vec::<i32>::new().fast_reduce(|a, x| a + x);
        }),
        "UnsupportedOperationException: Empty collection can't be reduced."
    );
    // (their suspending twins: the same results, each lambda suspending)
    block_on(async {
        let mut seen = Vec::new();
        xs.fast_for_each_indexed_async(async |i, x| {
            suspend().await;
            seen.push(i * 10 + x);
        })
        .await;
        xs.fast_for_each_reversed_async(async |x| {
            suspend().await;
            seen.push(x);
        })
        .await;
        assert_eq!(seen, [3, 11, 24, 31, 45, 5, 1, 4, 1, 3]);
        let doubled = xs
            .fast_map_async(async |x| {
                suspend().await;
                x * 2
            })
            .await;
        assert_eq!(doubled, [6, 2, 8, 2, 10]);
        let picked = xs
            .fast_filtered_map_async(
                async |x| {
                    suspend().await;
                    x > 2
                },
                async |x| x * 100,
            )
            .await;
        assert_eq!(picked, [300, 400, 500]);
        let steps = xs
            .fast_zip_with_next_async(async |a, b| {
                suspend().await;
                b - a
            })
            .await;
        assert_eq!(steps, [-2, 3, -3, 4]);
        assert_eq!(pairs.fast_max_by_async(async |p| p.0).await, Some((3, 'b')));
        assert_eq!(xs.fast_max_of_or_default_async(0, async |x| x).await, 5);
        let mut out = Vec::new();
        assert_eq!(*xs.fast_map_to_async(&mut out, async |x| x + 1).await, [4, 2, 5, 2, 6]);
        assert_eq!(xs.fast_map_indexed_not_null_async(async |i, x| (i == 2).then_some(x)).await, [4]);
        assert_eq!(xs.fast_zip_async(&[10, 20], async |x, y| x + y).await, [13, 21]);
        assert_eq!(xs.fast_first_async(async |x| x > 3).await, 4);
    });
}

#[test]
fn random_of_a_seed_and_shuffle() {
    // The same seed, the same numbers; `nextInt(until)` within its range (a power of two by its upper bits, else by
    // rejection); an empty range thrown as Kotlin throws it; a shuffle a permutation, the same for the same seed.
    let (a, b) = (Random(7), Random(7));
    let xs: Vec<i32> = (0..20).map(|_| a.next_int()).collect();
    assert_eq!(xs, (0..20).map(|_| b.next_int()).collect::<Vec<_>>());
    assert_ne!(xs, (0..20).map(|_| Random(8).next_int()).collect::<Vec<_>>());
    let r = Random(42);
    for until in [1, 2, 3, 8, 10, 1000] {
        for _ in 0..50 {
            let v = r.next_int_until(until);
            assert!((0..until).contains(&v), "{v} of {until}");
        }
    }
    assert!((-5..5).contains(&r.next_int_in(-5, 5)));
    assert_eq!(
        thrown(|| {
            Random(1).next_int_until(0);
        }),
        "IllegalArgumentException: Random range is empty: [0, 0)."
    );
    let mut list: Vec<i32> = (1..=10).collect();
    list.shuffle(&Random(7));
    let mut sorted = list.clone();
    sorted.sort();
    assert_eq!(sorted, (1..=10).collect::<Vec<_>>());
    let state = mutable_state_list_of((1..=10).collect::<Vec<i32>>());
    state.shuffle(&Random(7));
    assert_eq!(state.to_vec(), list);
}

#[test]
fn compose_ui_util_list_functions_and_digits_only() {
    // Compose's `fastForEach` & co. give what the plain functions give.
    let xs = vec![3, 1, 2];
    let mut seen = Vec::new();
    xs.fast_for_each_indexed(|i, v| seen.push((i, v)));
    assert_eq!(seen, [(0, 3), (1, 1), (2, 2)]);
    let mut back = Vec::new();
    xs.fast_for_each_reversed(|v| back.push(v));
    assert_eq!(back, [2, 1, 3]);
    let mut all = Vec::new();
    xs.fast_for_each(|v| all.push(v));
    assert_eq!(all, xs);
    assert_eq!(xs.fast_map(|v| v * 2), [6, 2, 4]);
    assert_eq!(xs.fast_map_indexed(|i, v| i + v), [3, 2, 4]);
    assert_eq!(xs.fast_map_not_null(|v| (v > 1).then_some(v)), [3, 2]);
    assert_eq!(xs.fast_filter(|v| v != 1), [3, 2]);
    assert!(xs.fast_any(|v| v == 2) && !xs.fast_all(|v| v > 1));
    assert_eq!((xs.fast_first_or_null(|v| v < 3), xs.fast_first(|v| v < 3)), (Some(1), 1));
    assert_eq!(xs.fast_sum_by(|v| v), 6);
    assert_eq!((xs.fast_max_by(|v| v), xs.fast_min_by_or_null(|v| v)), (Some(3), Some(1)));
    assert_eq!(Vec::<i32>::new().fast_max_by(|v| v), None);
    // Android's `isDigitsOnly`: Unicode's decimal digits, an empty text too.
    assert!("0123".is_digits_only() && "".is_digits_only() && "٣٤".is_digits_only());
    assert!(!"12a".is_digits_only() && !"1 2".is_digits_only() && !"½".is_digits_only());
}

#[test]
fn delegates_of_a_declared_type_and_a_states_setter() {
    // `var x: Int? by remember { mutableStateOf(null) }`: the declared type tells what the state holds.
    let picked = delegate_of::<Option<i32>, _>(mutable_state_of(None));
    assert_eq!(KtDelegate::get_value(&picked), None);
    KtMutableDelegate::set_value(&picked, Some(2));
    assert_eq!(picked.get(), Some(2));
    // `val (value, setValue) = state`: the setter is a value Kotlin shares (cloned here).
    let state = mutable_state_of(false);
    let set = state.component2();
    let other = set.clone();
    other(true);
    assert!(state.component1());
    set(false);
    assert!(!state.get());
}
