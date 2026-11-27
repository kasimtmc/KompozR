//! Our own JNI binding (the survey's D4: no `jni` crate): the few JNI types, the functions of the `JNIEnv` and
//! `JavaVM` tables KompozR calls (taken from the tables by their index in `jni.h`, which the JVM spec fixes: they
//! never move between releases), and a safe layer over them ([`Env`], [`Global`], [`Local`]) — local references
//! deleted as they go, global ones kept for objects held across calls, a Java exception after each call described to
//! the log and cleared (an exception left pending would make the next JNI call abort the process), text as UTF-8
//! `byte[]` both ways (JNI's own strings are modified UTF-8, which mangles emoji: no `NewStringUTF` here).
//!
//! An FFI module: its `unsafe` is here, each block saying why it holds; everything above it uses [`Env`].

#![allow(unsafe_code, non_camel_case_types)]

use std::ffi::{CStr, c_char, c_void};
use std::marker::PhantomData;
use std::ptr;

pub type jint = i32;
pub type jlong = i64;
pub type jboolean = u8;
pub type jfloat = f32;
pub type jsize = jint;
/// Any Java object reference (a class, an array, a string are ones too).
pub type jobject = *mut c_void;
pub type jclass = jobject;
pub type jmethodID = *mut c_void;

/// A Java reference as KompozR holds one: a native method's argument (the VM's, as JNI passes it: this type is
/// `jobject` in a native's signature), a [`Local`]'s or a [`Global`]'s; null for Java's null. Opaque: only the VM
/// reads it, and only these give one (nothing else can make one up).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JObject(jobject);

impl JObject {
    pub const fn null() -> JObject {
        JObject(ptr::null_mut())
    }

    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    /// The reference itself, for the NDK's functions that take one (in this crate's FFI modules).
    pub(super) fn raw(self) -> jobject {
        self.0
    }
}

pub const JNI_OK: jint = 0;
pub const JNI_VERSION_1_6: jint = 0x0001_0006;

/// A Java call's argument (JNI's `jvalue`), made from what it is.
#[repr(C)]
#[derive(Clone, Copy)]
pub union jvalue {
    z: jboolean,
    i: jint,
    j: jlong,
    f: jfloat,
    l: jobject,
}

impl From<bool> for jvalue {
    fn from(v: bool) -> jvalue {
        jvalue { z: v as jboolean }
    }
}
impl From<jint> for jvalue {
    fn from(v: jint) -> jvalue {
        jvalue { i: v }
    }
}
impl From<jlong> for jvalue {
    fn from(v: jlong) -> jvalue {
        jvalue { j: v }
    }
}
impl From<jfloat> for jvalue {
    fn from(v: jfloat) -> jvalue {
        jvalue { f: v }
    }
}
impl From<JObject> for jvalue {
    fn from(v: JObject) -> jvalue {
        jvalue { l: v.0 }
    }
}

/// What `JNIEnv *` points to: the function table's address (JNI's `JNINativeInterface *`).
pub type JNIEnv = *const *const c_void;
/// What `JavaVM *` points to: the invocation table's address.
pub type JavaVM = *const *const c_void;

/// A native method as `RegisterNatives` takes it.
#[repr(C)]
pub struct JNINativeMethod {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub fn_ptr: *mut c_void,
}

/// The slots of `JNINativeInterface` used here, by their index in `jni.h` (the first four are reserved).
mod slot {
    pub const FIND_CLASS: usize = 6;
    pub const EXCEPTION_DESCRIBE: usize = 16;
    pub const EXCEPTION_CLEAR: usize = 17;
    pub const NEW_GLOBAL_REF: usize = 21;
    pub const DELETE_GLOBAL_REF: usize = 22;
    pub const DELETE_LOCAL_REF: usize = 23;
    pub const IS_SAME_OBJECT: usize = 24;
    pub const GET_OBJECT_CLASS: usize = 31;
    pub const GET_METHOD_ID: usize = 33;
    pub const CALL_OBJECT_METHOD_A: usize = 36;
    pub const CALL_BOOLEAN_METHOD_A: usize = 39;
    pub const CALL_INT_METHOD_A: usize = 51;
    pub const CALL_VOID_METHOD_A: usize = 63;
    pub const GET_ARRAY_LENGTH: usize = 171;
    pub const NEW_BYTE_ARRAY: usize = 176;
    pub const NEW_INT_ARRAY: usize = 179;
    pub const NEW_FLOAT_ARRAY: usize = 181;
    pub const GET_BYTE_ARRAY_REGION: usize = 200;
    pub const GET_INT_ARRAY_REGION: usize = 203;
    pub const GET_LONG_ARRAY_REGION: usize = 204;
    pub const GET_FLOAT_ARRAY_REGION: usize = 205;
    pub const SET_BYTE_ARRAY_REGION: usize = 208;
    pub const SET_INT_ARRAY_REGION: usize = 211;
    pub const SET_FLOAT_ARRAY_REGION: usize = 213;
    pub const REGISTER_NATIVES: usize = 215;
    pub const GET_JAVA_VM: usize = 219;
    pub const EXCEPTION_CHECK: usize = 228;
}

/// The slots of `JNIInvokeInterface` used here.
mod vm_slot {
    pub const ATTACH_CURRENT_THREAD: usize = 4;
    pub const DETACH_CURRENT_THREAD: usize = 5;
    pub const GET_ENV: usize = 6;
}

/// The function at `index` of the table `object` points to (a `JNIEnv *` or `JavaVM *` is the address of a pointer
/// to its table; the table's layout is JNI's, fixed by the spec).
///
/// # Safety
/// `object` must be a live `JNIEnv *` or `JavaVM *`, `index` one of its table's slots, and `F` that slot's exact
/// type.
unsafe fn function<F: Copy>(object: *const *const c_void, index: usize) -> F {
    debug_assert_eq!(size_of::<F>(), size_of::<*const c_void>());
    // SAFETY: (the caller's) `*object` is the function table, `index` within it, its entry a function pointer of
    // type F (same size as a pointer: asserted).
    unsafe {
        let table = (*object).cast::<*const c_void>();
        let entry = *table.add(index);
        std::mem::transmute_copy::<*const c_void, F>(&entry)
    }
}

/// The JNI of the calling thread, for the length of a native call or while the thread is attached (`'a`). Not
/// `Send`: a `JNIEnv` is its thread's.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    raw: *mut JNIEnv,
    _thread: PhantomData<&'a ()>,
}

/// A local reference (deleted as it goes): valid in the native call that made it.
pub struct Local<'a> {
    env: Env<'a>,
    raw: jobject,
}

impl Local<'_> {
    pub fn object(&self) -> JObject {
        JObject(self.raw)
    }

    /// The reference handed back to Java (a native's result: Java deletes it), no longer ours to delete.
    pub fn into_object(self) -> JObject {
        let raw = self.raw;
        std::mem::forget(self);
        JObject(raw)
    }
}

impl Drop for Local<'_> {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            // SAFETY: a local reference this env made, deleted once.
            unsafe {
                let delete: unsafe extern "system" fn(*mut JNIEnv, jobject) =
                    function(self.env.raw.cast(), slot::DELETE_LOCAL_REF);
                delete(self.env.raw, self.raw);
            }
        }
    }
}

/// A global reference: an object kept across native calls (the activity, its view), on any thread; deleted on the
/// thread it is dropped on (attached to the VM: the UI thread, where KompozR keeps them).
pub struct Global {
    vm: Vm,
    raw: jobject,
}

impl Global {
    pub fn object(&self) -> JObject {
        JObject(self.raw)
    }
}

impl Drop for Global {
    fn drop(&mut self) {
        if let Some(env) = self.vm.env() {
            // SAFETY: a global reference made by `Env::new_global`, deleted once, on an attached thread.
            unsafe {
                let delete: unsafe extern "system" fn(*mut JNIEnv, jobject) =
                    function(env.raw.cast(), slot::DELETE_GLOBAL_REF);
                delete(env.raw, self.raw);
            }
        }
    }
}

/// The Java VM (the process has one): what gives a thread its [`Env`] — `JNI_OnLoad`'s argument (this type is
/// `JavaVM *` in its signature), or an env's. `Send` and `Sync`, as JNI's `JavaVM` is.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vm(*mut JavaVM);

// SAFETY: JNI's JavaVM is the process's, its functions callable from any thread (the spec's "JavaVM ... can be
// shared between threads").
unsafe impl Send for Vm {}
// SAFETY: as above.
unsafe impl Sync for Vm {}

impl Vm {
    /// The calling thread's env, if it is attached to the VM (the UI thread is; none otherwise).
    pub fn env(&self) -> Option<Env<'static>> {
        let mut env: *mut JNIEnv = ptr::null_mut();
        // SAFETY: GetEnv of the live VM, writing the env pointer.
        let result = unsafe {
            let get_env: unsafe extern "system" fn(*mut JavaVM, *mut *mut c_void, jint) -> jint =
                function(self.0.cast(), vm_slot::GET_ENV);
            get_env(self.0, (&raw mut env).cast(), JNI_VERSION_1_6)
        };
        (result == JNI_OK && !env.is_null()).then_some(Env { raw: env, _thread: PhantomData })
    }

    /// The calling thread attached to the VM (a thread of KompozR's own that calls Java: an image's download), its
    /// env; detached again by [`detach`](Vm::detach) before the thread ends.
    pub fn attach(&self) -> Option<Env<'static>> {
        if let Some(env) = self.env() {
            return Some(env);
        }
        let mut env: *mut JNIEnv = ptr::null_mut();
        // SAFETY: AttachCurrentThread of the live VM (no arguments: the thread's name and group the VM's own).
        let result = unsafe {
            let attach: unsafe extern "system" fn(*mut JavaVM, *mut *mut JNIEnv, *mut c_void) -> jint =
                function(self.0.cast(), vm_slot::ATTACH_CURRENT_THREAD);
            attach(self.0, &mut env, ptr::null_mut())
        };
        (result == JNI_OK && !env.is_null()).then_some(Env { raw: env, _thread: PhantomData })
    }

    /// The calling thread detached (one [`attach`](Vm::attach) made it; never the UI thread, the VM's own).
    pub fn detach(&self) {
        // SAFETY: DetachCurrentThread of the live VM, on a thread this module attached.
        unsafe {
            let detach: unsafe extern "system" fn(*mut JavaVM) -> jint =
                function(self.0.cast(), vm_slot::DETACH_CURRENT_THREAD);
            detach(self.0);
        }
    }
}

/// The value of a method's identifier between calls (a `jmethodID` lives as long as its class: the app's classes
/// are never unloaded).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MethodId(jmethodID);

// SAFETY: a jmethodID is a plain identifier, valid on every thread while its class is loaded.
unsafe impl Send for MethodId {}
// SAFETY: as above.
unsafe impl Sync for MethodId {}

impl<'a> Env<'a> {
    /// The env a native method was called with.
    ///
    /// # Safety
    /// `raw` must be the `JNIEnv *` of the current native call (valid for `'a`, on this thread).
    pub unsafe fn from_raw(raw: *mut JNIEnv) -> Env<'a> {
        Env { raw, _thread: PhantomData }
    }

    /// A function of the env's table (see [`function`]).
    ///
    /// # Safety
    /// `F` must be slot `index`'s exact type.
    unsafe fn f<F: Copy>(&self, index: usize) -> F {
        // SAFETY: self.raw is a live env (Env's invariant); the type is the caller's.
        unsafe { function(self.raw.cast(), index) }
    }

    /// Whether the last call threw: the exception described into the log (logcat: its stack) and cleared, so that
    /// the next call can go on (JNI aborts on a call made with one pending).
    pub fn check(&self) -> bool {
        // SAFETY: ExceptionCheck, ExceptionDescribe and ExceptionClear take only the env.
        unsafe {
            let pending: unsafe extern "system" fn(*mut JNIEnv) -> jboolean = self.f(slot::EXCEPTION_CHECK);
            if pending(self.raw) == 0 {
                return false;
            }
            let describe: unsafe extern "system" fn(*mut JNIEnv) = self.f(slot::EXCEPTION_DESCRIBE);
            describe(self.raw);
            let clear: unsafe extern "system" fn(*mut JNIEnv) = self.f(slot::EXCEPTION_CLEAR);
            clear(self.raw);
        }
        true
    }

    fn local(&self, raw: jobject) -> Option<Local<'a>> {
        let thrown = self.check();
        (!raw.is_null() && !thrown).then_some(Local { env: *self, raw })
    }

    /// The VM this env is of.
    pub fn vm(&self) -> Option<Vm> {
        let mut vm: *mut JavaVM = ptr::null_mut();
        // SAFETY: GetJavaVM writes the VM's pointer.
        let result = unsafe {
            let get: unsafe extern "system" fn(*mut JNIEnv, *mut *mut JavaVM) -> jint = self.f(slot::GET_JAVA_VM);
            get(self.raw, &mut vm)
        };
        (result == JNI_OK && !vm.is_null()).then_some(Vm(vm))
    }

    /// The class named `name` ("dev/kompozr/KompozView"), as the calling thread's class loader finds it: an app's
    /// classes only from the UI thread (`JNI_OnLoad`'s), whose loader is the app's.
    pub fn find_class(&self, name: &CStr) -> Option<Local<'a>> {
        // SAFETY: FindClass with a NUL-terminated name.
        let raw = unsafe {
            let find: unsafe extern "system" fn(*mut JNIEnv, *const c_char) -> jclass = self.f(slot::FIND_CLASS);
            find(self.raw, name.as_ptr())
        };
        self.local(raw)
    }

    /// `object`'s class.
    pub fn class_of(&self, object: JObject) -> Option<Local<'a>> {
        if object.is_null() {
            return None;
        }
        // SAFETY: GetObjectClass of a live reference.
        let raw = unsafe {
            let get: unsafe extern "system" fn(*mut JNIEnv, jobject) -> jclass = self.f(slot::GET_OBJECT_CLASS);
            get(self.raw, object.0)
        };
        self.local(raw)
    }

    /// The instance method `name` of `class` with JNI's `signature` ("(I)V").
    pub fn method(&self, class: &Local<'_>, name: &CStr, signature: &CStr) -> Option<MethodId> {
        // SAFETY: GetMethodID with a live class and NUL-terminated strings.
        let raw = unsafe {
            let get: unsafe extern "system" fn(*mut JNIEnv, jclass, *const c_char, *const c_char) -> jmethodID =
                self.f(slot::GET_METHOD_ID);
            get(self.raw, class.raw, name.as_ptr(), signature.as_ptr())
        };
        let thrown = self.check();
        (!raw.is_null() && !thrown).then_some(MethodId(raw))
    }

    /// `methods` registered as `class`'s native ones (instead of exported `Java_…` symbols, which a cdylib may not
    /// keep from the rlib they are in); false if one isn't the class's.
    pub fn register_natives(&self, class: &Local<'_>, methods: &[JNINativeMethod]) -> bool {
        // SAFETY: RegisterNatives with a live class and `methods.len()` entries, each a NUL-terminated name and
        // signature and an extern "system" function of that signature (the caller's table).
        let result = unsafe {
            let register: unsafe extern "system" fn(*mut JNIEnv, jclass, *const JNINativeMethod, jint) -> jint =
                self.f(slot::REGISTER_NATIVES);
            register(self.raw, class.raw, methods.as_ptr(), methods.len() as jint)
        };
        let thrown = self.check();
        result == JNI_OK && !thrown
    }

    /// A global reference to `object` (none for null).
    pub fn new_global(&self, object: JObject) -> Option<Global> {
        if object.is_null() {
            return None;
        }
        let vm = self.vm()?;
        // SAFETY: NewGlobalRef of a live reference.
        let raw = unsafe {
            let new: unsafe extern "system" fn(*mut JNIEnv, jobject) -> jobject = self.f(slot::NEW_GLOBAL_REF);
            new(self.raw, object.0)
        };
        (!raw.is_null()).then_some(Global { vm, raw })
    }

    /// Whether `a` and `b` are the same object.
    pub fn same(&self, a: JObject, b: JObject) -> bool {
        // SAFETY: IsSameObject of two references (null allowed).
        unsafe {
            let same: unsafe extern "system" fn(*mut JNIEnv, jobject, jobject) -> jboolean =
                self.f(slot::IS_SAME_OBJECT);
            same(self.raw, a.0, b.0) != 0
        }
    }

    /// `object.method(args)` of a method returning nothing; false if it threw (the exception logged and cleared).
    ///
    /// The arguments must be what the method's signature says (the caller's: a method found with
    /// [`method`](Env::method) and its signature).
    pub fn call_void(&self, object: JObject, method: MethodId, args: &[jvalue]) -> bool {
        if object.is_null() {
            return false;
        }
        // SAFETY: CallVoidMethodA of a live object's method, with the arguments its signature takes (the caller's).
        unsafe {
            let call: unsafe extern "system" fn(*mut JNIEnv, jobject, jmethodID, *const jvalue) =
                self.f(slot::CALL_VOID_METHOD_A);
            call(self.raw, object.0, method.0, args.as_ptr());
        }
        !self.check()
    }

    /// `object.method(args)` of a method returning a boolean (false if it threw).
    pub fn call_boolean(&self, object: JObject, method: MethodId, args: &[jvalue]) -> bool {
        if object.is_null() {
            return false;
        }
        // SAFETY: as call_void's, of a method returning a boolean.
        let result = unsafe {
            let call: unsafe extern "system" fn(*mut JNIEnv, jobject, jmethodID, *const jvalue) -> jboolean =
                self.f(slot::CALL_BOOLEAN_METHOD_A);
            call(self.raw, object.0, method.0, args.as_ptr())
        };
        !self.check() && result != 0
    }

    /// `object.method(args)` of a method returning an int (none if it threw).
    pub fn call_int(&self, object: JObject, method: MethodId, args: &[jvalue]) -> Option<jint> {
        if object.is_null() {
            return None;
        }
        // SAFETY: as call_void's, of a method returning an int.
        let result = unsafe {
            let call: unsafe extern "system" fn(*mut JNIEnv, jobject, jmethodID, *const jvalue) -> jint =
                self.f(slot::CALL_INT_METHOD_A);
            call(self.raw, object.0, method.0, args.as_ptr())
        };
        (!self.check()).then_some(result)
    }

    /// `object.method(args)` of a method returning an object (none for null, or if it threw).
    pub fn call_object(&self, object: JObject, method: MethodId, args: &[jvalue]) -> Option<Local<'a>> {
        if object.is_null() {
            return None;
        }
        // SAFETY: as call_void's, of a method returning an object (a local reference, deleted by Local).
        let raw = unsafe {
            let call: unsafe extern "system" fn(*mut JNIEnv, jobject, jmethodID, *const jvalue) -> jobject =
                self.f(slot::CALL_OBJECT_METHOD_A);
            call(self.raw, object.0, method.0, args.as_ptr())
        };
        self.local(raw)
    }

    /// The length of the array `array` (0 for null).
    pub fn array_length(&self, array: JObject) -> usize {
        if array.is_null() {
            return 0;
        }
        // SAFETY: GetArrayLength of a live array reference.
        let n = unsafe {
            let length: unsafe extern "system" fn(*mut JNIEnv, jobject) -> jsize = self.f(slot::GET_ARRAY_LENGTH);
            length(self.raw, array.0)
        };
        n.max(0) as usize
    }

    /// A `byte[]`'s bytes (none for null).
    pub fn bytes(&self, array: JObject) -> Option<Vec<u8>> {
        if array.is_null() {
            return None;
        }
        let n = self.array_length(array);
        let mut out = vec![0u8; n];
        // SAFETY: GetByteArrayRegion of the whole array into a buffer of its length.
        unsafe {
            let get: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *mut i8) =
                self.f(slot::GET_BYTE_ARRAY_REGION);
            get(self.raw, array.0, 0, n as jsize, out.as_mut_ptr().cast());
        }
        (!self.check()).then_some(out)
    }

    /// A `byte[]` of UTF-8 as text (none for null; what isn't UTF-8 replaced).
    pub fn utf8(&self, array: JObject) -> Option<String> {
        self.bytes(array).map(|b| String::from_utf8(b).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into()))
    }

    /// A new `byte[]` of `bytes` (none if the VM can't make it: out of memory, the exception logged).
    pub fn new_bytes(&self, bytes: &[u8]) -> Option<Local<'a>> {
        // SAFETY: NewByteArray, then SetByteArrayRegion of its whole length from `bytes`.
        let raw = unsafe {
            let new: unsafe extern "system" fn(*mut JNIEnv, jsize) -> jobject = self.f(slot::NEW_BYTE_ARRAY);
            new(self.raw, bytes.len() as jsize)
        };
        let array = self.local(raw)?;
        // SAFETY: as above.
        unsafe {
            let set: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *const i8) =
                self.f(slot::SET_BYTE_ARRAY_REGION);
            set(self.raw, array.raw, 0, bytes.len() as jsize, bytes.as_ptr().cast());
        }
        (!self.check()).then_some(array)
    }

    /// An `int[]`'s values (empty for null).
    pub fn ints(&self, array: JObject) -> Vec<i32> {
        let n = self.array_length(array);
        let mut out = vec![0i32; n];
        if n > 0 {
            // SAFETY: GetIntArrayRegion of the whole array into a buffer of its length.
            unsafe {
                let get: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *mut jint) =
                    self.f(slot::GET_INT_ARRAY_REGION);
                get(self.raw, array.0, 0, n as jsize, out.as_mut_ptr());
            }
        }
        if self.check() { Vec::new() } else { out }
    }

    /// A `float[]`'s values (empty for null).
    pub fn floats(&self, array: JObject) -> Vec<f32> {
        let n = self.array_length(array);
        let mut out = vec![0f32; n];
        if n > 0 {
            // SAFETY: GetFloatArrayRegion of the whole array into a buffer of its length.
            unsafe {
                let get: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *mut jfloat) =
                    self.f(slot::GET_FLOAT_ARRAY_REGION);
                get(self.raw, array.0, 0, n as jsize, out.as_mut_ptr());
            }
        }
        if self.check() { Vec::new() } else { out }
    }

    /// A `long[]`'s values (empty for null).
    pub fn longs(&self, array: JObject) -> Vec<i64> {
        let n = self.array_length(array);
        let mut out = vec![0i64; n];
        if n > 0 {
            // SAFETY: GetLongArrayRegion of the whole array into a buffer of its length.
            unsafe {
                let get: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *mut jlong) =
                    self.f(slot::GET_LONG_ARRAY_REGION);
                get(self.raw, array.0, 0, n as jsize, out.as_mut_ptr());
            }
        }
        if self.check() { Vec::new() } else { out }
    }

    /// A new `int[]` of `values` (none if the VM can't make it).
    pub fn new_ints(&self, values: &[i32]) -> Option<Local<'a>> {
        // SAFETY: NewIntArray, then SetIntArrayRegion of its whole length from `values`.
        let raw = unsafe {
            let new: unsafe extern "system" fn(*mut JNIEnv, jsize) -> jobject = self.f(slot::NEW_INT_ARRAY);
            new(self.raw, values.len() as jsize)
        };
        let array = self.local(raw)?;
        // SAFETY: as above.
        unsafe {
            let set: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *const jint) =
                self.f(slot::SET_INT_ARRAY_REGION);
            set(self.raw, array.raw, 0, values.len() as jsize, values.as_ptr());
        }
        (!self.check()).then_some(array)
    }

    /// A new `float[]` of `values` (none if the VM can't make it).
    pub fn new_floats(&self, values: &[f32]) -> Option<Local<'a>> {
        // SAFETY: NewFloatArray, then SetFloatArrayRegion of its whole length from `values`.
        let raw = unsafe {
            let new: unsafe extern "system" fn(*mut JNIEnv, jsize) -> jobject = self.f(slot::NEW_FLOAT_ARRAY);
            new(self.raw, values.len() as jsize)
        };
        let array = self.local(raw)?;
        // SAFETY: as above.
        unsafe {
            let set: unsafe extern "system" fn(*mut JNIEnv, jobject, jsize, jsize, *const jfloat) =
                self.f(slot::SET_FLOAT_ARRAY_REGION);
            set(self.raw, array.raw, 0, values.len() as jsize, values.as_ptr());
        }
        (!self.check()).then_some(array)
    }

    /// The raw `JNIEnv *` (for the NDK's functions that take one: `ANativeWindow_fromSurface`).
    pub fn raw(&self) -> *mut JNIEnv {
        self.raw
    }
}

/// A [`JNINativeMethod`] of `name`, `signature` and `function` (an `extern "system"` function of that signature).
pub fn native(name: &'static CStr, signature: &'static CStr, function: *mut c_void) -> JNINativeMethod {
    JNINativeMethod { name: name.as_ptr(), signature: signature.as_ptr(), fn_ptr: function }
}
