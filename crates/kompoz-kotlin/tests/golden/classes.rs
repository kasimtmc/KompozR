#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Note {
    pub id: i32,
    pub title: ::std::string::String,
    pub pinned: bool,
    pub tags: ::std::vec::Vec<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Note(
    id: i32,
    title: impl ::core::convert::Into<::std::string::String>,
    #[default(false)] pinned: bool,
    # [default (:: kompoz :: runtime :: kt :: list_of ([]))] tags: ::std::vec::Vec<::std::string::String>,
) -> Note {
    let title: ::std::string::String = ::core::convert::Into::into(title);
    Note::__kz_new(id, title, pinned, tags)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Note {
    #[doc(hidden)]
    pub fn __kz_new(
        id: i32,
        title: ::std::string::String,
        pinned: bool,
        tags: ::std::vec::Vec<::std::string::String>,
    ) -> Note {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Note(rrdd)), { Note { id, title, pinned, tags } })
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn title(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.title)
    }
    pub fn pinned(&self) -> bool {
        self.pinned
    }
    pub fn tags(&self) -> ::std::vec::Vec<::std::string::String> {
        ::core::clone::Clone::clone(&self.tags)
    }
    pub fn component1(&self) -> i32 {
        self.id()
    }
    pub fn component2(&self) -> ::std::string::String {
        self.title()
    }
    pub fn component3(&self) -> bool {
        self.pinned()
    }
    pub fn component4(&self) -> ::std::vec::Vec<::std::string::String> {
        self.tags()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Note {
    pub fn copy(
        &self,
        # [default (self . id)] id: i32,
        # [default (:: core :: clone :: Clone :: clone (& self . title))] title: ::std::string::String,
        # [default (self . pinned)] pinned: bool,
        # [default (:: core :: clone :: Clone :: clone (& self . tags))] tags: ::std::vec::Vec<::std::string::String>,
    ) -> Note {
        Note::__kz_new(id, title, pinned, tags)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Note {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.id, state);
        ::core::hash::Hash::hash(&self.title, state);
        ::core::hash::Hash::hash(&self.pinned, state);
        ::core::hash::Hash::hash(&self.tags, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Note {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Note {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Note(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", title=");
        (&::kompoz::runtime::kt::KtProbe(&self.title)).kt_write_probe(__kz_out);
        __kz_out.push_str(", pinned=");
        (&::kompoz::runtime::kt::KtProbe(&self.pinned)).kt_write_probe(__kz_out);
        __kz_out.push_str(", tags=");
        (&::kompoz::runtime::kt::KtProbe(&self.tags)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Note {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Note {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Note(rrdd)))]
pub fn NoteRow(note: Note) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Text(::kompoz::runtime::kt::template!(
        note.id(),
        ": ",
        note.title(),
        if note.pinned() { ::std::string::String::from(" *") } else { ::std::string::String::from("") }
    ));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(Note(rrdd)))]
pub fn Notes() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let a = Note(title = "alışveriş", id = 1);
    let b = a.copy(pinned = true, title = ::std::string::String::from("iş"));
    Column(|| {
        NoteRow(a.clone());
        NoteRow(b.clone());
        Text(::kompoz::runtime::kt::template!(b), modifier = Modifier.test_tag("str"));
        Text(if a == b.copy(pinned = false, title = ::std::string::String::from("alışveriş")) {
            ::std::string::String::from("eşit")
        } else {
            ::std::string::String::from("farklı")
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzTally {
    Step: ::kompoz::runtime::kt::KtVal<i32>,
    total: ::kompoz::runtime::kt::KtVar<i32>,
    label: ::kompoz::runtime::kt::KtVal<::std::string::String>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct Tally {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Tally {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzTally {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzTally > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(
                || __KzTally {
                    Step: ::kompoz::runtime::kt::KtVal::new(),
                    total: ::kompoz::runtime::kt::KtVar::new(),
                    label: ::kompoz::runtime::kt::KtVal::new(),
                },
                |_| Tally {}.__kz_init(),
            )
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().Step.init(2);
            Self::__kz_state().total.set(0);
            Self::__kz_state().label.init(::std::string::String::from("toplam"));
            {
                self.set_total(10);
            }
        })
    }
    pub fn Step(&self) -> i32 {
        Self::__kz_state().Step.get()
    }
    pub fn total(&self) -> i32 {
        Self::__kz_state().total.get()
    }
    pub fn set_total(&self, value: i32) {
        Self::__kz_state().total.set(value)
    }
    fn label(&self) -> ::std::string::String {
        Self::__kz_state().label.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Tally {
    pub fn add(&self, # [default (self . Step ())] n: i32) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.set_total(self.total() + n);
            return self.total();
        })
    }
    pub fn describe(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(self.label(), " ", self.total()) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Tally {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Tally",
            Self::__kz_state() as *const __KzTally as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Tally {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Tally {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzLedger {
    id: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Ledger {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzLedger>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Ledger(id: i32) -> Ledger {
    Ledger::__kz_new(id)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Ledger {
    #[doc(hidden)]
    pub fn __kz_new(id: i32) -> Ledger {
        let __kz_this =
            Ledger { __kz_this: ::std::rc::Rc::new(__KzLedger { id: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init(id);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, id: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_companion = LedgerCompanion {};
        self.__kz_this.id.init(id);
        ::kompoz::runtime::named_args!({})
    }
    pub fn id(&self) -> i32 {
        self.__kz_this.id.get()
    }
    #[allow(non_upper_case_globals)]
    pub const Companion: LedgerCompanion = LedgerCompanion {};
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Ledger {
    pub fn label(&self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        #[allow(unused_variables)]
        let __kz_companion = LedgerCompanion {};
        ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(__kz_companion.PREFIX(), self.id()) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Ledger {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Ledger {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Ledger {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Ledger {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Ledger",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Ledger {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Ledger {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzLedgerCompanion {
    PREFIX: ::kompoz::runtime::kt::KtVal<::std::string::String>,
    made: ::kompoz::runtime::kt::KtVar<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct LedgerCompanion {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LedgerCompanion {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzLedgerCompanion {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzLedgerCompanion > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(
                || __KzLedgerCompanion {
                    PREFIX: ::kompoz::runtime::kt::KtVal::new(),
                    made: ::kompoz::runtime::kt::KtVar::new(),
                },
                |_| LedgerCompanion {}.__kz_init(),
            )
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().PREFIX.init(::std::string::String::from("hesap-"));
            Self::__kz_state().made.set(0);
        })
    }
    pub fn PREFIX(&self) -> ::std::string::String {
        Self::__kz_state().PREFIX.get()
    }
    pub fn made(&self) -> i32 {
        Self::__kz_state().made.get()
    }
    pub fn set_made(&self, value: i32) {
        Self::__kz_state().made.set(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl LedgerCompanion {
    pub fn create(&self) -> Ledger {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Ledger(r)), {
            self.set_made(::kompoz::runtime::kt::KtInc::inc(self.made()));
            return Ledger(self.made());
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for LedgerCompanion {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Ledger$Companion",
            Self::__kz_state() as *const __KzLedgerCompanion as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for LedgerCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for LedgerCompanion {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: cmp :: PartialOrd,
    :: core :: cmp :: Ord,
    :: core :: fmt :: Debug,
)]
pub enum Rack {
    Top,
    Bottom,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Rack {
    #[doc = r" The entry's name (Kotlin's `name`)."]
    pub fn name(self) -> ::std::string::String {
        ::std::string::String::from(match self {
            Rack::Top => "Top",
            Rack::Bottom => "Bottom",
        })
    }
    #[doc = r" Its place among the entries (Kotlin's `ordinal`)."]
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    #[doc = r" The entries in order (Kotlin's `entries`)."]
    pub fn entries() -> ::std::vec::Vec<Rack> {
        ::std::vec![Rack::Top, Rack::Bottom]
    }
    #[doc = r" The entries in order (Kotlin's `values()`)."]
    pub fn values() -> ::std::vec::Vec<Rack> {
        ::std::vec![Rack::Top, Rack::Bottom]
    }
    #[doc = r" The entry of a name (Kotlin's `valueOf`: an IllegalArgumentException for another name)."]
    #[track_caller]
    pub fn value_of(value: impl ::core::convert::Into<::std::string::String>) -> Rack {
        let value: ::std::string::String = value.into();
        match value.as_str() {
            "Top" => Rack::Top,
            "Bottom" => Rack::Bottom,
            _ => ::kompoz::runtime::kt::panic_kt(
                "IllegalArgumentException",
                &::std::format!("{}{}", "No enum constant Rack.", value),
            ),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Rack {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Rack {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&self.name());
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Rack {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&self.name())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::Saveable for Rack {
    fn save(&self) -> ::kompoz::runtime::SavedValue {
        ::kompoz::runtime::SavedValue::String(self.name())
    }
    fn restore(saved: &::kompoz::runtime::SavedValue) -> ::core::option::Option<Self> {
        let name = saved.as_str()?;
        Self::entries().into_iter().find(|e| e.name() == name)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: cmp :: PartialOrd,
    :: core :: cmp :: Ord,
    :: core :: fmt :: Debug,
)]
pub enum Tier {
    FREE,
    PAID,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Tier {
    #[doc = r" The entry's name (Kotlin's `name`)."]
    pub fn name(self) -> ::std::string::String {
        ::std::string::String::from(match self {
            Tier::FREE => "FREE",
            Tier::PAID => "PAID",
        })
    }
    #[doc = r" Its place among the entries (Kotlin's `ordinal`)."]
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    #[doc = r" The entries in order (Kotlin's `entries`)."]
    pub fn entries() -> ::std::vec::Vec<Tier> {
        ::std::vec![Tier::FREE, Tier::PAID]
    }
    #[doc = r" The entries in order (Kotlin's `values()`)."]
    pub fn values() -> ::std::vec::Vec<Tier> {
        ::std::vec![Tier::FREE, Tier::PAID]
    }
    #[doc = r" The entry of a name (Kotlin's `valueOf`: an IllegalArgumentException for another name)."]
    #[track_caller]
    pub fn value_of(value: impl ::core::convert::Into<::std::string::String>) -> Tier {
        let value: ::std::string::String = value.into();
        match value.as_str() {
            "FREE" => Tier::FREE,
            "PAID" => Tier::PAID,
            _ => ::kompoz::runtime::kt::panic_kt(
                "IllegalArgumentException",
                &::std::format!("{}{}", "No enum constant Tier.", value),
            ),
        }
    }
    pub fn price(self) -> i32 {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(match self {
            Tier::FREE => 0,
            Tier::PAID => 5,
        })
    }
    pub fn label(self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(match self {
            Tier::FREE => ::std::string::String::from(::std::string::String::from("tier")),
            Tier::PAID => ::std::string::String::from(::std::string::String::from("paid")),
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Tier {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Tier {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&self.name());
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Tier {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&self.name())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::Saveable for Tier {
    fn save(&self) -> ::kompoz::runtime::SavedValue {
        ::kompoz::runtime::SavedValue::String(self.name())
    }
    fn restore(saved: &::kompoz::runtime::SavedValue) -> ::core::option::Option<Self> {
        let name = saved.as_str()?;
        Self::entries().into_iter().find(|e| e.name() == name)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub type Names = ::std::vec::Vec<::std::string::String>;
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
pub type Cells<T> = ::std::vec::Vec<::std::vec::Vec<T>>;
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn total(grid: ::std::vec::Vec<::std::vec::Vec<i32>>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::KtList::sum_of(&grid, |it| ::kompoz::runtime::kt::KtList::sum(&it))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn count_names(names: ::std::vec::Vec<::std::string::String>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::size(&names) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCatalog {
    items: ::kompoz::runtime::kt::KtVal<::std::vec::Vec<CatalogEntry>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Catalog {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzCatalog>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Catalog() -> Catalog {
    Catalog::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Catalog {
    #[doc(hidden)]
    pub fn __kz_new() -> Catalog {
        let __kz_this =
            Catalog { __kz_this: ::std::rc::Rc::new(__KzCatalog { items: ::kompoz::runtime::kt::KtVal::new() }) };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(CatalogEntry(rd)), {
            self.__kz_this.items.init(::kompoz::runtime::kt::list_of([
                CatalogEntry("a"),
                CatalogEntry("b", CatalogKind::FILM),
                CatalogEntry("c"),
            ]));
        })
    }
    fn items(&self) -> ::std::vec::Vec<CatalogEntry> {
        self.__kz_this.items.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Catalog {
    pub fn films(&self) -> ::std::vec::Vec<CatalogEntry> {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::KtList::filter(&self.items(), |it| it.kind() == CatalogKind::FILM)
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Catalog {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Catalog {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Catalog {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Catalog {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Catalog",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Catalog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct CatalogEntry {
    pub title: ::std::string::String,
    pub kind: CatalogKind,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn CatalogEntry(
    title: impl ::core::convert::Into<::std::string::String>,
    #[default(CatalogKind::BOOK)] kind: CatalogKind,
) -> CatalogEntry {
    let title: ::std::string::String = ::core::convert::Into::into(title);
    CatalogEntry::__kz_new(title, kind)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CatalogEntry {
    #[doc(hidden)]
    pub fn __kz_new(title: ::std::string::String, kind: CatalogKind) -> CatalogEntry {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(CatalogEntry(rd)), { CatalogEntry { title, kind } })
    }
    pub fn title(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.title)
    }
    pub fn kind(&self) -> CatalogKind {
        self.kind
    }
    pub fn component1(&self) -> ::std::string::String {
        self.title()
    }
    pub fn component2(&self) -> CatalogKind {
        self.kind()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl CatalogEntry {
    pub fn copy(
        &self,
        # [default (:: core :: clone :: Clone :: clone (& self . title))] title: ::std::string::String,
        # [default (self . kind)] kind: CatalogKind,
    ) -> CatalogEntry {
        CatalogEntry::__kz_new(title, kind)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for CatalogEntry {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.title, state);
        ::core::hash::Hash::hash(&self.kind, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for CatalogEntry {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for CatalogEntry {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Entry(");
        __kz_out.push_str("title=");
        (&::kompoz::runtime::kt::KtProbe(&self.title)).kt_write_probe(__kz_out);
        __kz_out.push_str(", kind=");
        (&::kompoz::runtime::kt::KtProbe(&self.kind)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for CatalogEntry {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for CatalogEntry {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: cmp :: PartialOrd,
    :: core :: cmp :: Ord,
    :: core :: fmt :: Debug,
)]
pub enum CatalogKind {
    BOOK,
    FILM,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CatalogKind {
    #[doc = r" The entry's name (Kotlin's `name`)."]
    pub fn name(self) -> ::std::string::String {
        ::std::string::String::from(match self {
            CatalogKind::BOOK => "BOOK",
            CatalogKind::FILM => "FILM",
        })
    }
    #[doc = r" Its place among the entries (Kotlin's `ordinal`)."]
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    #[doc = r" The entries in order (Kotlin's `entries`)."]
    pub fn entries() -> ::std::vec::Vec<CatalogKind> {
        ::std::vec![CatalogKind::BOOK, CatalogKind::FILM]
    }
    #[doc = r" The entries in order (Kotlin's `values()`)."]
    pub fn values() -> ::std::vec::Vec<CatalogKind> {
        ::std::vec![CatalogKind::BOOK, CatalogKind::FILM]
    }
    #[doc = r" The entry of a name (Kotlin's `valueOf`: an IllegalArgumentException for another name)."]
    #[track_caller]
    pub fn value_of(value: impl ::core::convert::Into<::std::string::String>) -> CatalogKind {
        let value: ::std::string::String = value.into();
        match value.as_str() {
            "BOOK" => CatalogKind::BOOK,
            "FILM" => CatalogKind::FILM,
            _ => ::kompoz::runtime::kt::panic_kt(
                "IllegalArgumentException",
                &::std::format!("{}{}", "No enum constant Kind.", value),
            ),
        }
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl CatalogKind {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for CatalogKind {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&self.name());
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for CatalogKind {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&self.name())
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::Saveable for CatalogKind {
    fn save(&self) -> ::kompoz::runtime::SavedValue {
        ::kompoz::runtime::SavedValue::String(self.name())
    }
    fn restore(saved: &::kompoz::runtime::SavedValue) -> ::core::option::Option<Self> {
        let name = saved.as_str()?;
        Self::entries().into_iter().find(|e| e.name() == name)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzCatalogDefaults {
    limit: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(
    :: core :: clone :: Clone,
    :: core :: marker :: Copy,
    :: core :: cmp :: PartialEq,
    :: core :: cmp :: Eq,
    :: core :: hash :: Hash,
    :: core :: default :: Default,
)]
pub struct CatalogDefaults {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl CatalogDefaults {
    #[doc(hidden)]
    fn __kz_state() -> &'static __KzCatalogDefaults {
        ::std::thread_local! { static __KZ_STATE : :: kompoz :: runtime :: kt :: KtSingleton < __KzCatalogDefaults > = const { :: kompoz :: runtime :: kt :: KtSingleton :: new () } ; }
        __KZ_STATE.with(|s| {
            s.get(
                || __KzCatalogDefaults { limit: ::kompoz::runtime::kt::KtVal::new() },
                |_| CatalogDefaults {}.__kz_init(),
            )
        })
    }
    #[doc(hidden)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            Self::__kz_state().limit.init(2);
        })
    }
    pub fn limit(&self) -> i32 {
        Self::__kz_state().limit.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl CatalogDefaults {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for CatalogDefaults {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Catalog.Defaults",
            Self::__kz_state() as *const __KzCatalogDefaults as *const (),
        ));
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for CatalogDefaults {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for CatalogDefaults {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn uses() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Catalog(), CatalogEntry(rd)), {
        let e = CatalogEntry("d", CatalogKind::FILM);
        return ::kompoz::runtime::kt::template!(
            "",
            Tally {}.add(),
            Tally {}.describe(),
            LedgerCompanion {}.create().label(),
            LedgerCompanion {}.made(),
            Rack::Bottom,
            Tier::PAID.price(),
            CatalogDefaults {}.limit(),
            e,
            ::kompoz::runtime::kt::KtList::size(&Catalog().films()),
            Rack::value_of("Top").ordinal(),
            ::kompoz::runtime::kt::KtList::size(&Tier::entries())
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable(types(LocalItemsItem(rd), LocalItemsCounter()))]
pub fn LocalItems() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let mut items =
        ::kompoz::runtime::kt::list_of([LocalItemsItem(0, ::std::string::String::from("a")), LocalItemsItem(1)]);
    let counter = remember(|| LocalItemsCounter());
    Text(::kompoz::runtime::kt::template!(
        ::kompoz::runtime::kt::KtList::first(&items),
        " ",
        ::kompoz::runtime::kt::KtList::last(&items).label(),
        " ",
        counter.count()
    ));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub(crate) struct LocalItemsItem {
    pub id: i32,
    pub label: ::std::string::String,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn LocalItemsItem(
    id: i32,
    #[default(::std::string::String::from(""))] label: ::std::string::String,
) -> LocalItemsItem {
    LocalItemsItem::__kz_new(id, label)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LocalItemsItem {
    #[doc(hidden)]
    pub(crate) fn __kz_new(id: i32, label: ::std::string::String) -> LocalItemsItem {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(LocalItemsItem(rd)), { LocalItemsItem { id, label } })
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn label(&self) -> ::std::string::String {
        ::core::clone::Clone::clone(&self.label)
    }
    pub fn component1(&self) -> i32 {
        self.id()
    }
    pub fn component2(&self) -> ::std::string::String {
        self.label()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl LocalItemsItem {
    pub(crate) fn copy(
        &self,
        # [default (self . id)] id: i32,
        # [default (:: core :: clone :: Clone :: clone (& self . label))] label: ::std::string::String,
    ) -> LocalItemsItem {
        LocalItemsItem::__kz_new(id, label)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for LocalItemsItem {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.id, state);
        ::core::hash::Hash::hash(&self.label, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for LocalItemsItem {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for LocalItemsItem {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Item(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", label=");
        (&::kompoz::runtime::kt::KtProbe(&self.label)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for LocalItemsItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for LocalItemsItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub(crate) struct __KzLocalItemsCounter {
    count: ::kompoz::runtime::kt::KtVar<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub(crate) struct LocalItemsCounter {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzLocalItemsCounter>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn LocalItemsCounter() -> LocalItemsCounter {
    LocalItemsCounter::__kz_new()
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LocalItemsCounter {
    #[doc(hidden)]
    pub(crate) fn __kz_new() -> LocalItemsCounter {
        let __kz_this = LocalItemsCounter {
            __kz_this: ::std::rc::Rc::new(__KzLocalItemsCounter { count: ::kompoz::runtime::kt::KtVar::new() }),
        };
        __kz_this.__kz_init();
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.__kz_this.count.set(0);
        })
    }
    pub fn count(&self) -> i32 {
        self.__kz_this.count.get()
    }
    pub fn set_count(&self, value: i32) {
        self.__kz_this.count.set(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl LocalItemsCounter {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for LocalItemsCounter {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for LocalItemsCounter {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for LocalItemsCounter {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for LocalItemsCounter {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Counter",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for LocalItemsCounter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for LocalItemsCounter {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn local_sizes() -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(LocalSizesItem(rr)), {
        let __kz_d1 = LocalSizesItem(2, 3);
        let id = __kz_d1.component1();
        let size = __kz_d1.component2();
        return id.clone() + size.clone();
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub(crate) struct LocalSizesItem {
    pub id: i32,
    pub size: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub(crate) fn LocalSizesItem(id: i32, size: i32) -> LocalSizesItem {
    LocalSizesItem::__kz_new(id, size)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl LocalSizesItem {
    #[doc(hidden)]
    pub(crate) fn __kz_new(id: i32, size: i32) -> LocalSizesItem {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(LocalSizesItem(rr)), { LocalSizesItem { id, size } })
    }
    pub fn id(&self) -> i32 {
        self.id
    }
    pub fn size(&self) -> i32 {
        self.size
    }
    pub fn component1(&self) -> i32 {
        self.id()
    }
    pub fn component2(&self) -> i32 {
        self.size()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl LocalSizesItem {
    pub(crate) fn copy(&self, # [default (self . id)] id: i32, # [default (self . size)] size: i32) -> LocalSizesItem {
        LocalSizesItem::__kz_new(id, size)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for LocalSizesItem {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.id, state);
        ::core::hash::Hash::hash(&self.size, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for LocalSizesItem {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for LocalSizesItem {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Item(");
        __kz_out.push_str("id=");
        (&::kompoz::runtime::kt::KtProbe(&self.id)).kt_write_probe(__kz_out);
        __kz_out.push_str(", size=");
        (&::kompoz::runtime::kt::KtProbe(&self.size)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for LocalSizesItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for LocalSizesItem {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone, :: core :: cmp :: PartialEq)]
pub struct Meters {
    pub value: i32,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Meters(value: i32) -> Meters {
    Meters::__kz_new(value)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Meters {
    #[doc(hidden)]
    pub fn __kz_new(value: i32) -> Meters {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!(types(Meters(r)), { Meters { value } })
    }
    pub fn value(&self) -> i32 {
        self.value
    }
    pub fn component1(&self) -> i32 {
        self.value()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Meters {
    pub fn copy(&self, # [default (self . value)] value: i32) -> Meters {
        Meters::__kz_new(value)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Meters {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&self.value, state);
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Meters {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Meters {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::{KtViaDisplay as _, KtViaToString as _};
        __kz_out.push_str("Meters(");
        __kz_out.push_str("value=");
        (&::kompoz::runtime::kt::KtProbe(&self.value)).kt_write_probe(__kz_out);
        __kz_out.push(')');
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Meters {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Meters {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn twice(m: Meters) -> Meters {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!(types(Meters(r)), { Meters(m.value() * 2) })
}
