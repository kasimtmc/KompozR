//! Kotlin's exceptions (stage 14): [`Throwable`], what the Kotlin syntax's `throw`, `try`/`catch`/`finally` and
//! the exception classes are on Rust's unwinding.
//!
//! A `throw` is a panic ([`throw`]): its payload is the exception's text as Kotlin's `toString()` writes it
//! (`IllegalStateException: Check failed.`), a `String`, as every Kotlin-syntax exception's has been (what a test
//! catching a panic reads, what a coroutine's failure says), and the [`Throwable`] itself is kept beside it, the
//! thread's, keyed by that payload, so a `catch` gets the object thrown back — its class, message, cause, and the
//! object of a class of the user's (`class Refused(val code: Int) : IllegalStateException("kod $code")`) — as
//! Kotlin's catch does. A panic of other Rust code is caught too: one whose text starts with a Kotlin exception's
//! class (`NumberFormatException: …`, what the kt shims panic with) is of that class, any other a
//! `RuntimeException` of its text.
//!
//! `try` is [`catching`] (a body run with the panic caught: `Err` of what it threw) or, in a coroutine,
//! [`catching_async`] (the body's future polled so); the generator writes the `catch` clauses as a match on the
//! class ([`Throwable::is_a`], [`Throwable::downcast`]) and rethrows what none takes ([`rethrow`]), the `finally`
//! block after both, whatever happened. While a `try` body runs, the panic hook is quiet (a hook of this module
//! installed once, before the program's own, which it calls outside of `try` bodies): a caught exception prints
//! nothing, as Kotlin's doesn't; one no `try` takes is reported as any panic is.
//!
//! **This needs `panic = "unwind"`** (Rust's default profile setting): with `panic = "abort"` a `throw` ends the
//! program, a `catch` never runs.

use super::text::KtToString;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// A Kotlin exception: Kotlin's `Throwable` and every class of it (`Exception`, `IllegalStateException`, a class
/// of the user's deriving from one), a reference to one object (clones are the same exception; `==` is identity).
#[derive(Clone)]
pub struct Throwable(Rc<Inner>);

struct Inner {
    /// Its class's name (`IllegalStateException`, `Refused`).
    class: String,
    /// The standard class it is, or (a user's class) the one its class derives from.
    base: String,
    message: Option<String>,
    cause: Option<Throwable>,
    /// The object of a user's class it is.
    object: Option<Rc<dyn Any>>,
}

/// The standard class Kotlin's (and Java's, and kotlinx's) exception class `name` derives from, as the JVM has them;
/// none for `Throwable`. A class this doesn't know derives from `RuntimeException` (`…Exception`) or `Error`
/// (`…Error`).
#[doc(hidden)]
pub fn parent_class(name: &str) -> Option<&'static str> {
    if name == "Throwable" {
        return None;
    }
    Some(standard_parent(name).unwrap_or(if name.ends_with("Error") { "Error" } else { "RuntimeException" }))
}

/// Whether `name` is one of the standard exception classes (Kotlin's, Java's, kotlinx's) KompozR knows.
#[doc(hidden)]
pub fn is_standard_class(name: &str) -> bool {
    name == "Throwable" || standard_parent(name).is_some()
}

fn standard_parent(name: &str) -> Option<&'static str> {
    Some(match name {
        "Exception" | "Error" => "Throwable",
        "RuntimeException" | "IOException" | "InterruptedException" | "CloneNotSupportedException" => "Exception",
        "IllegalArgumentException"
        | "IllegalStateException"
        | "IndexOutOfBoundsException"
        | "NoSuchElementException"
        | "UnsupportedOperationException"
        | "ArithmeticException"
        | "ClassCastException"
        | "NullPointerException"
        | "ConcurrentModificationException"
        | "UninitializedPropertyAccessException"
        | "NoWhenBranchMatchedException"
        | "DateTimeException"
        | "ArrayStoreException"
        | "NegativeArraySizeException" => "RuntimeException",
        "NumberFormatException" | "PatternSyntaxException" | "IllegalFormatException" | "SerializationException" => {
            "IllegalArgumentException"
        }
        "IllegalFormatConversionException"
        | "IllegalFormatPrecisionException"
        | "MissingFormatArgumentException"
        | "UnknownFormatConversionException"
        | "DuplicateFormatFlagsException"
        | "FormatFlagsConversionMismatchException" => "IllegalFormatException",
        "StringIndexOutOfBoundsException" | "ArrayIndexOutOfBoundsException" => "IndexOutOfBoundsException",
        "KotlinNullPointerException" => "NullPointerException",
        // (kotlinx.coroutines: a cancellation is an IllegalStateException on the JVM)
        "CancellationException" | "AnchoredDraggableUninitializedException" => "IllegalStateException",
        "TimeoutCancellationException" => "CancellationException",
        "FileNotFoundException" | "EOFException" => "IOException",
        "NotImplementedError" | "AssertionError" | "OutOfMemoryError" | "StackOverflowError" => "Error",
        _ => return None,
    })
}

impl Throwable {
    /// An exception of a standard class (`IllegalArgumentException("boş")`: Kotlin's constructors of them, a
    /// message and a cause).
    #[doc(hidden)]
    pub fn new(class: &str, message: Option<String>, cause: Option<Throwable>) -> Throwable {
        Throwable(Rc::new(Inner { class: class.to_string(), base: class.to_string(), message, cause, object: None }))
    }

    /// The exception a user's class's object is (`Refused(7)`): its class's name, the standard class it derives
    /// from, the message and cause its constructor gave that class, the object.
    #[doc(hidden)]
    pub fn of_object(
        class: &str,
        base: &str,
        message: Option<String>,
        cause: Option<Throwable>,
        object: Rc<dyn Any>,
    ) -> Throwable {
        Throwable(Rc::new(Inner {
            class: class.to_string(),
            base: base.to_string(),
            message,
            cause,
            object: Some(object),
        }))
    }

    /// Kotlin's `message`.
    pub fn message(&self) -> Option<String> {
        self.0.message.clone()
    }

    /// Kotlin's `localizedMessage` (its message).
    pub fn localized_message(&self) -> Option<String> {
        self.0.message.clone()
    }

    /// Kotlin's `cause`.
    pub fn cause(&self) -> Option<Throwable> {
        self.0.cause.clone()
    }

    /// Its class's (simple) name (Kotlin's `e::class.simpleName`).
    #[doc(hidden)]
    pub fn class_name(&self) -> &str {
        &self.0.class
    }

    /// Whether it is of the class `name` or of one deriving from it (Kotlin's `e is IllegalStateException`, a
    /// `catch (e: IllegalStateException)` taking it).
    #[doc(hidden)]
    pub fn is_a(&self, name: &str) -> bool {
        if self.0.class == name {
            return true;
        }
        let mut c: &str = &self.0.base;
        loop {
            if c == name {
                return true;
            }
            match parent_class(c) {
                Some(p) => c = p,
                None => return false,
            }
        }
    }

    /// The object of a user's class it is, if it is one of `T` (Kotlin's `e as? Refused`; a `catch (e: Refused)`).
    #[doc(hidden)]
    pub fn downcast<T: Any + Clone>(&self) -> Option<T> {
        self.0.object.as_ref()?.downcast_ref::<T>().cloned()
    }

    /// Whether it is the object of a user's class `T` (Kotlin's `e is Refused`).
    #[doc(hidden)]
    pub fn is_object<T: Any>(&self) -> bool {
        self.0.object.as_ref().is_some_and(|o| o.is::<T>())
    }

    /// Kotlin's `stackTraceToString()`: its text (KompozR keeps no stack trace of an exception).
    pub fn stack_trace_to_string(&self) -> String {
        self.kt_to_string()
    }

    /// The exception a caught panic is (see the module's doc).
    #[doc(hidden)]
    pub fn from_panic(payload: Box<dyn Any + Send>) -> Throwable {
        let text = if let Some(s) = payload.downcast_ref::<String>() {
            if let Some(t) = take_thrown(s) {
                return t;
            }
            s.clone()
        } else if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(t) = payload.downcast_ref::<Throwable>() {
            return t.clone();
        } else {
            return Throwable::new("RuntimeException", None, None);
        };
        Throwable::of_text(&text)
    }

    /// The exception a panic's text is: `Class: message` (or `Class`) of a Kotlin exception's class, else a
    /// `RuntimeException` of the text.
    #[doc(hidden)]
    pub fn of_text(text: &str) -> Throwable {
        let (head, message) = match text.split_once(": ") {
            Some((h, m)) => (h, Some(m.to_string())),
            None => (text, None),
        };
        let class_like = head.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && head.chars().all(|c| c.is_ascii_alphanumeric())
            && (head.ends_with("Exception") || head.ends_with("Error") || head == "Throwable");
        if class_like {
            Throwable::new(head, message, None)
        } else {
            Throwable::new("RuntimeException", Some(text.to_string()), None)
        }
    }
}

impl PartialEq for Throwable {
    /// Kotlin's `==` of exceptions: identity (the same object; a user's class's object, whatever made it one).
    fn eq(&self, other: &Self) -> bool {
        match (&self.0.object, &other.0.object) {
            (Some(a), Some(b)) => std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b)),
            _ => Rc::ptr_eq(&self.0, &other.0),
        }
    }
}

impl KtToString for Throwable {
    /// Kotlin's `toString()` of a Throwable: `Class: message`, or `Class` without a message (the class's simple
    /// name, as KompozR names classes).
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.0.class);
        if let Some(m) = &self.0.message {
            out.push_str(": ");
            out.push_str(m);
        }
    }
}

impl fmt::Display for Throwable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

impl fmt::Debug for Throwable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

/// A value that is a Kotlin exception (a user's exception class's object: what `throw` takes, what an `Exception`
/// parameter, field or list holds it as).
pub trait KtThrowable {
    /// The exception it is.
    fn to_throwable(&self) -> Throwable;
}

impl KtThrowable for Throwable {
    fn to_throwable(&self) -> Throwable {
        self.clone()
    }
}

// ---- Throwing and catching -------------------------------------------------------------------------------------

// How many `try` bodies of the thread are running (the panic hook is quiet while one is).
thread_local! {
    static TRYING: Cell<usize> = const { Cell::new(0) };
    /// The exceptions thrown and not caught yet, by their payload's text buffer.
    static THROWN: RefCell<Vec<(usize, Throwable)>> = const { RefCell::new(Vec::new()) };
}

/// How many thrown exceptions are kept for their catch (the oldest are dropped: those nothing caught).
const KEPT: usize = 64;

fn take_thrown(payload: &str) -> Option<Throwable> {
    let key = payload.as_ptr() as usize;
    THROWN.with(|t| {
        let mut t = t.borrow_mut();
        let at = t.iter().rposition(|(k, _)| *k == key)?;
        Some(t.remove(at).1)
    })
}

/// Kotlin's `throw e`: a panic of `e`'s text, `e` kept for the `catch` that takes it (see the module's doc).
#[track_caller]
#[doc(hidden)]
pub fn throw(e: impl KtThrowable) -> ! {
    let e = e.to_throwable();
    let text = e.kt_to_string();
    let key = text.as_ptr() as usize;
    THROWN.with(|t| {
        let mut t = t.borrow_mut();
        if t.len() == KEPT {
            t.remove(0);
        }
        t.push((key, e));
    });
    std::panic::panic_any(text)
}

/// What no `catch` of a `try` took, thrown on (the same exception).
#[track_caller]
#[doc(hidden)]
pub fn rethrow(e: Throwable) -> ! {
    throw(e)
}

/// Kotlin's `try` body: `f` run, `Err` of the exception it threw (the panic hook quiet meanwhile).
#[doc(hidden)]
pub fn catching<R>(f: impl FnOnce() -> R) -> Result<R, Throwable> {
    quiet_hook();
    TRYING.with(|t| t.set(t.get() + 1));
    let r = catch_unwind(AssertUnwindSafe(f));
    TRYING.with(|t| t.set(t.get() - 1));
    r.map_err(Throwable::from_panic)
}

/// Kotlin's `try` body in a coroutine: `f` awaited, `Err` of the exception it threw (in any of its polls).
#[doc(hidden)]
pub fn catching_async<F: Future>(f: F) -> impl Future<Output = Result<F::Output, Throwable>> {
    quiet_hook();
    Catching(Box::pin(f))
}

struct Catching<F>(Pin<Box<F>>);

impl<F: Future> Future for Catching<F> {
    type Output = Result<F::Output, Throwable>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        TRYING.with(|t| t.set(t.get() + 1));
        let inner = &mut self.0;
        let r = catch_unwind(AssertUnwindSafe(|| inner.as_mut().poll(cx)));
        TRYING.with(|t| t.set(t.get() - 1));
        match r {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(p) => Poll::Ready(Err(Throwable::from_panic(p))),
        }
    }
}

/// Installs (once) the hook keeping panics quiet while a `try` body runs, the program's own hook called otherwise.
fn quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let trying = TRYING.try_with(|t| t.get() > 0).unwrap_or(false);
            if !trying {
                previous(info);
            }
        }));
    });
}

// ---- Kotlin's constructors of the standard exceptions ------------------------------------------------------------

/// An exception of a standard class made as Kotlin's constructors make one: `IllegalStateException()`,
/// `IllegalStateException("m")`, `Exception("m", cause)`, `RuntimeException(cause)` (its message the cause's text).
#[doc(hidden)]
pub fn exception(class: &str, message: Option<String>, cause: Option<Throwable>) -> Throwable {
    let message = match (message, &cause) {
        (None, Some(c)) => Some(c.kt_to_string()),
        (m, _) => m,
    };
    Throwable::new(class, message, cause)
}

/// What an exception class's constructor's first argument is: a message (`String`, `String?`) or a cause
/// (`Throwable`, a user's exception).
#[doc(hidden)]
pub trait KtExceptionArg {
    /// Its message and cause.
    fn exception_parts(self) -> (Option<String>, Option<Throwable>);
}

impl KtExceptionArg for String {
    fn exception_parts(self) -> (Option<String>, Option<Throwable>) {
        (Some(self), None)
    }
}
impl KtExceptionArg for &str {
    fn exception_parts(self) -> (Option<String>, Option<Throwable>) {
        (Some(self.to_string()), None)
    }
}
impl KtExceptionArg for Option<String> {
    fn exception_parts(self) -> (Option<String>, Option<Throwable>) {
        (self, None)
    }
}
impl KtExceptionArg for Throwable {
    fn exception_parts(self) -> (Option<String>, Option<Throwable>) {
        (Some(self.kt_to_string()), Some(self))
    }
}
impl KtExceptionArg for Option<Throwable> {
    fn exception_parts(self) -> (Option<String>, Option<Throwable>) {
        let message = self.as_ref().map(|c| c.kt_to_string());
        (message, self)
    }
}

// ---- What the generated code of a `try` uses ----------------------------------------------------------------------

/// What a closure written for a `try` body gives when a `return`, `break` or `continue` in it leaves it: the code
/// after the `try` does the jump (`Loop(n)`: the closure's `n`th loop jump).
#[doc(hidden)]
pub enum Jump<V, R> {
    Value(V),
    Return(R),
    Loop(u32),
}

/// A `catch (e: CancellationException)`'s body in a coroutine: run if the coroutine is dropped (cancelled) before
/// [`OnCancel::disarm`].
#[doc(hidden)]
pub struct OnCancel<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> OnCancel<F> {
    #[doc(hidden)]
    pub fn new(on_cancel: F) -> OnCancel<F> {
        OnCancel(Some(on_cancel))
    }

    /// The body ended: nothing to run.
    #[doc(hidden)]
    pub fn disarm(mut self) {
        self.0 = None;
    }
}

impl<F: FnOnce()> Drop for OnCancel<F> {
    fn drop(&mut self) {
        // (not while unwinding: a panic is the body's exception, not a cancellation)
        if let Some(f) = self.0.take()
            && !std::thread::panicking()
        {
            f();
        }
    }
}
