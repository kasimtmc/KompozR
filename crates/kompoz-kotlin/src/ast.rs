//! The Kotlin syntax tree: what the parser reads and the generator turns into Rust.
//!
//! It covers the Kotlin that KompozR's syntax takes — the whole of the C++ kompozc's (`compiler/ast.hpp`) and
//! what stage 14 adds — as Kotlin's grammar has it: a file of declarations (functions, properties, classes in all
//! their kinds, objects, type aliases), their members, statements and expressions. Every node knows where it is
//! (`loc`), names keep theirs (`Ident`), string templates keep their parts' positions: the generator gives each
//! Rust token the span of the Kotlin it comes from.
//!
//! What a declaration *is* follows Kotlin's own model rather than a struct per kind: a [`Class`] has a
//! [`ClassKind`] (class, interface, fun interface, enum class, annotation class) and modifiers (`data`, `sealed`,
//! `inner`, `value`, `abstract`, `open`); an [`Object`] is a declaration, a companion (modifier `companion`) or,
//! as [`ExprKind::Object`], an expression. Helpers answer the questions the generator asks (`is_data()`,
//! `is_kompozable()`, …).
//!
//! The parser leaves [`ExprKind::Error`] where it recovered inside an expression and [`Decl::Failed`] for a
//! declaration it gave up on (each with a diagnostic), so the rest of a file is still read and translated.

use crate::pos::Loc;

// ---- Names -----------------------------------------------------------------------------------------------------

/// A name as written: `count`, or a backticked `` `my name` `` (without its backticks).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ident {
    pub name: String,
    pub backticked: bool,
    pub loc: Loc,
}

impl Ident {
    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }
}

/// A dotted name: `androidx.compose.runtime.Composable`, `Outer.Inner`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub loc: Loc,
}

impl Path {
    /// Its last name (`Composable` of `androidx.compose.runtime.Composable`).
    pub fn last(&self) -> &Ident {
        self.segments.last().expect("a path has a name")
    }

    /// Its names joined with dots.
    pub fn dotted(&self) -> String {
        self.segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(".")
    }
}

// ---- Files, annotations, modifiers -----------------------------------------------------------------------------

/// What `kompoz! { … }` holds, or a `.kt` file: an optional package, imports, declarations in order.
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    /// File annotations: `@file:Suppress("…")`.
    pub annotations: Vec<Annotation>,
    pub package: Option<Path>,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl>,
    pub loc: Loc,
}

/// `import a.b.C`, `import a.b.*`, `import a.b.C as D`.
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub path: Path,
    pub star: bool,
    pub alias: Option<Ident>,
    pub loc: Loc,
}

/// `@Name`, `@Name(args)`, `@a.b.Name`, `@get:Name`: what it names, its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    /// `@get:`, `@field:`, `@param:`, `@file:`, … (Kotlin's use-site targets).
    pub use_site: Option<Ident>,
    pub name: Path,
    pub type_args: Vec<TypeArg>,
    /// None: written without parentheses.
    pub args: Option<Vec<Arg>>,
    pub loc: Loc,
}

impl Annotation {
    /// `@Kompozable`, or `@Composable` (pasted Compose code: an alias).
    pub fn is_kompozable(&self) -> bool {
        matches!(self.name.last().name.as_str(), "Kompozable" | "Composable")
    }
}

/// Whether one of the annotations is `@Kompozable` (or `@Composable`).
pub fn has_kompozable(annotations: &[Annotation]) -> bool {
    annotations.iter().any(Annotation::is_kompozable)
}

/// A modifier keyword: Kotlin's (visibility, inheritance, class kinds, members, parameters, functions,
/// properties, multiplatform).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierKind {
    Public,
    Private,
    Protected,
    Internal,
    Open,
    Final,
    Abstract,
    Sealed,
    Data,
    Inner,
    Value,
    Companion,
    Override,
    Lateinit,
    Const,
    Suspend,
    Inline,
    Noinline,
    Crossinline,
    Vararg,
    Operator,
    Infix,
    Tailrec,
    External,
    Expect,
    Actual,
}

impl ModifierKind {
    /// The modifier a keyword is (`enum`, `annotation` and `fun` start a class's kind, not here).
    pub fn from_keyword(word: &str) -> Option<ModifierKind> {
        use ModifierKind::*;
        Some(match word {
            "public" => Public,
            "private" => Private,
            "protected" => Protected,
            "internal" => Internal,
            "open" => Open,
            "final" => Final,
            "abstract" => Abstract,
            "sealed" => Sealed,
            "data" => Data,
            "inner" => Inner,
            "value" => Value,
            "companion" => Companion,
            "override" => Override,
            "lateinit" => Lateinit,
            "const" => Const,
            "suspend" => Suspend,
            "inline" => Inline,
            "noinline" => Noinline,
            "crossinline" => Crossinline,
            "vararg" => Vararg,
            "operator" => Operator,
            "infix" => Infix,
            "tailrec" => Tailrec,
            "external" => External,
            "expect" => Expect,
            "actual" => Actual,
            _ => return None,
        })
    }

    pub fn keyword(self) -> &'static str {
        use ModifierKind::*;
        match self {
            Public => "public",
            Private => "private",
            Protected => "protected",
            Internal => "internal",
            Open => "open",
            Final => "final",
            Abstract => "abstract",
            Sealed => "sealed",
            Data => "data",
            Inner => "inner",
            Value => "value",
            Companion => "companion",
            Override => "override",
            Lateinit => "lateinit",
            Const => "const",
            Suspend => "suspend",
            Inline => "inline",
            Noinline => "noinline",
            Crossinline => "crossinline",
            Vararg => "vararg",
            Operator => "operator",
            Infix => "infix",
            Tailrec => "tailrec",
            External => "external",
            Expect => "expect",
            Actual => "actual",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Modifier {
    pub kind: ModifierKind,
    pub loc: Loc,
}

/// The modifiers of a declaration or parameter, in the order written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub list: Vec<Modifier>,
}

impl Modifiers {
    pub fn has(&self, kind: ModifierKind) -> bool {
        self.list.iter().any(|m| m.kind == kind)
    }

    /// Where `kind` is written, if it is.
    pub fn find(&self, kind: ModifierKind) -> Option<Loc> {
        self.list.iter().find(|m| m.kind == kind).map(|m| m.loc)
    }

    /// Its visibility (public when none is written, as Kotlin's).
    pub fn visibility(&self) -> Visibility {
        for m in &self.list {
            match m.kind {
                ModifierKind::Private => return Visibility::Private,
                ModifierKind::Protected => return Visibility::Protected,
                ModifierKind::Internal => return Visibility::Internal,
                ModifierKind::Public => return Visibility::Public,
                _ => {}
            }
        }
        Visibility::Public
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

// ---- Types -----------------------------------------------------------------------------------------------------

/// A type as written: `Int`, `String?`, `List<Item>`, `Map.Entry<K, V>`, `(String) -> Unit`,
/// `@Kompozable () -> Unit`, `RowScope.() -> Unit`, `suspend () -> Unit`, `(() -> Unit)?`.
#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    /// `T?`.
    pub nullable: bool,
    /// `@Kompozable` on a function type (and any other type annotation).
    pub annotations: Vec<Annotation>,
    pub loc: Loc,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    /// A named type: its segments, each with its type arguments (`Outer<A>.Inner<B>`).
    Named(Vec<TypeSegment>),
    Function(FunctionType),
    /// What the parser left where a type couldn't be read (a diagnostic says why).
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeSegment {
    pub name: Ident,
    pub args: Vec<TypeArg>,
}

/// `[suspend] [Receiver.](A, b: B) -> R`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionType {
    pub suspend: bool,
    pub receiver: Option<Box<Type>>,
    pub params: Vec<FunctionTypeParam>,
    pub result: Box<Type>,
}

/// A function type's parameter: its type, named or not (`(index: Int) -> Unit`).
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionTypeParam {
    pub name: Option<Ident>,
    pub ty: Type,
}

/// A type argument: `*`, or a type with its variance (`out T`, `in T`).
#[derive(Clone, Debug, PartialEq)]
pub enum TypeArg {
    Star(Loc),
    Type { variance: Option<Variance>, ty: Type },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variance {
    In,
    Out,
}

impl Type {
    /// Its name when it is a plain named one (`Int`, `kotlin.String`), its segments dotted.
    pub fn simple_name(&self) -> Option<String> {
        match &self.kind {
            TypeKind::Named(segs) => Some(segs.iter().map(|s| s.name.name.as_str()).collect::<Vec<_>>().join(".")),
            _ => None,
        }
    }

    /// A composable function type (`@Kompozable () -> Unit`).
    pub fn is_kompozable(&self) -> bool {
        matches!(self.kind, TypeKind::Function(_)) && has_kompozable(&self.annotations)
    }

    pub fn as_function(&self) -> Option<&FunctionType> {
        match &self.kind {
            TypeKind::Function(f) => Some(f),
            _ => None,
        }
    }
}

/// `<T, out R : Bound, reified E>` on a function, class, interface, type alias or property.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeParam {
    pub annotations: Vec<Annotation>,
    pub variance: Option<Variance>,
    pub reified: bool,
    pub name: Ident,
    /// `: Bound`.
    pub bound: Option<Type>,
    pub loc: Loc,
}

/// `where T : A, T : B`: a bound of a type parameter given after the signature.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeConstraint {
    pub name: Ident,
    pub bound: Type,
    pub loc: Loc,
}

// ---- Declarations ----------------------------------------------------------------------------------------------

/// A declaration: at the top of a file, in a class's body, or local (in a block). (Boxed: statements and members
/// hold them beside small things.)
#[derive(Clone, Debug, PartialEq)]
pub enum Decl {
    Function(Box<Function>),
    Property(Box<Property>),
    Class(Box<Class>),
    Object(Box<Object>),
    TypeAlias(Box<TypeAlias>),
    /// One the parser gave up on (its diagnostics say why): its name when it got that far, so uses of it don't
    /// add errors of their own.
    Failed {
        name: Option<Ident>,
        loc: Loc,
    },
}

impl Decl {
    pub fn loc(&self) -> Loc {
        match self {
            Decl::Function(d) => d.loc,
            Decl::Property(d) => d.loc,
            Decl::Class(d) => d.loc,
            Decl::Object(d) => d.loc,
            Decl::TypeAlias(d) => d.loc,
            Decl::Failed { loc, .. } => *loc,
        }
    }

    /// Its name (an unnamed companion object, a destructuring declaration have none).
    pub fn name(&self) -> Option<&Ident> {
        match self {
            Decl::Function(d) => Some(&d.name),
            Decl::Property(d) => match &d.binding {
                Binding::Name(n) => Some(n),
                Binding::Destructure(_) => None,
            },
            Decl::Class(d) => Some(&d.name),
            Decl::Object(d) => d.name.as_ref(),
            Decl::TypeAlias(d) => Some(&d.name),
            Decl::Failed { name, .. } => name.as_ref(),
        }
    }
}

/// `[annotations] [modifiers] fun [<T>] [Receiver.]name(params)[: Result] [where …] { body } | = expression`; an
/// interface's or abstract one has no body. Also the anonymous function expression `fun(x: Int) = x + 1` (named
/// `""`, see [`ExprKind::AnonymousFunction`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub type_params: Vec<TypeParam>,
    /// An extension's: `fun Modifier.card()`.
    pub receiver: Option<Type>,
    pub name: Ident,
    pub params: Vec<Param>,
    /// None: `Unit`, or inferred from an expression body.
    pub result: Option<Type>,
    pub constraints: Vec<TypeConstraint>,
    pub body: Option<FunctionBody>,
    pub loc: Loc,
}

impl Function {
    pub fn is_kompozable(&self) -> bool {
        has_kompozable(&self.annotations)
    }

    pub fn is_suspend(&self) -> bool {
        self.modifiers.has(ModifierKind::Suspend)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FunctionBody {
    Block(Block),
    /// `= expression`.
    Expr(Expr),
}

/// A parameter of a function, a constructor or an accessor:
/// `[annotations] [vararg | noinline | crossinline] name: Type [= default]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub name: Ident,
    /// None only where Kotlin lets it be inferred (a setter's `set(v)`).
    pub ty: Option<Type>,
    pub default: Option<Expr>,
    pub loc: Loc,
}

impl Param {
    pub fn is_vararg(&self) -> bool {
        self.modifiers.has(ModifierKind::Vararg)
    }
}

/// `val`/`var`: a property, a local, a constructor's property parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mutability {
    Val,
    Var,
}

/// A property or a local:
/// `[annotations] [modifiers] val|var [<T>] [Receiver.]name[: Type] [= value | by delegate] [get() …] [set(v) …]`,
/// or a destructuring local `val (a, b) = pair`.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub mutability: Mutability,
    pub type_params: Vec<TypeParam>,
    /// An extension property's: `val String.shout: String get() = …`.
    pub receiver: Option<Type>,
    pub binding: Binding,
    pub ty: Option<Type>,
    pub constraints: Vec<TypeConstraint>,
    pub init: PropertyInit,
    pub getter: Option<Accessor>,
    pub setter: Option<Accessor>,
    pub loc: Loc,
}

impl Property {
    pub fn is_var(&self) -> bool {
        self.mutability == Mutability::Var
    }

    pub fn is_delegated(&self) -> bool {
        matches!(self.init, PropertyInit::Delegate(_))
    }
}

/// What a property is set from.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyInit {
    None,
    /// `= value`.
    Value(Expr),
    /// `by delegate`: read (and set) through it (`var count by remember { mutableStateOf(0) }`).
    Delegate(Expr),
}

/// What a declaration (local, loop variable, lambda parameter) binds: one name, or a destructuring of several
/// (`(a, b)`, `(_, b)`), each with an optional type.
#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Name(Ident),
    Destructure(Vec<DestructureEntry>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DestructureEntry {
    /// `_`: skipped.
    pub name: Ident,
    pub ty: Option<Type>,
    pub loc: Loc,
}

/// A property's `get()` or `set(v)`: `[modifiers] get() = …`, `get(): T { … }`, `private set`, `set(value) { field
/// = value }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Accessor {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    /// A setter's parameter.
    pub param: Option<Param>,
    /// A getter's declared result.
    pub ty: Option<Type>,
    /// None: only its visibility or annotations changed (`private set`).
    pub body: Option<FunctionBody>,
    pub loc: Loc,
}

/// What a [`Class`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClassKind {
    /// `class` (with `data`, `sealed`, `abstract`, `open`, `inner`, `value` among its modifiers).
    Class,
    /// `interface` (`sealed interface` with the `sealed` modifier).
    Interface,
    /// `fun interface`: an interface of one abstract function, made from a lambda too (`Name { … }`).
    FunInterface,
    /// `enum class`.
    Enum,
    /// `annotation class`.
    Annotation,
}

/// A class, interface, enum class or annotation class:
/// `[annotations] [modifiers] kind Name[<T>] [primary constructor] [: supers] [where …] [{ entries; members }]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub kind: ClassKind,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub primary_constructor: Option<PrimaryConstructor>,
    pub supers: Vec<SuperType>,
    pub constraints: Vec<TypeConstraint>,
    /// An enum class's entries, in order.
    pub enum_entries: Vec<EnumEntry>,
    pub body: ClassBody,
    pub loc: Loc,
}

impl Class {
    pub fn is_data(&self) -> bool {
        self.modifiers.has(ModifierKind::Data)
    }

    pub fn is_sealed(&self) -> bool {
        self.modifiers.has(ModifierKind::Sealed)
    }

    pub fn is_abstract(&self) -> bool {
        self.modifiers.has(ModifierKind::Abstract)
    }

    pub fn is_open(&self) -> bool {
        self.modifiers.has(ModifierKind::Open)
    }

    pub fn is_inner(&self) -> bool {
        self.modifiers.has(ModifierKind::Inner)
    }

    pub fn is_interface(&self) -> bool {
        matches!(self.kind, ClassKind::Interface | ClassKind::FunInterface)
    }

    /// Its companion object, if its body has one.
    pub fn companion(&self) -> Option<&Object> {
        self.body.members.iter().find_map(|m| match m {
            Member::Decl(Decl::Object(o)) if o.is_companion() => Some(&**o),
            _ => None,
        })
    }
}

/// `[annotations] [modifiers] [constructor](params)` after a class's name.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimaryConstructor {
    pub annotations: Vec<Annotation>,
    /// `private constructor(…)`.
    pub modifiers: Modifiers,
    pub params: Vec<ConstructorParam>,
    pub loc: Loc,
}

/// A primary constructor's parameter: a plain one, or a property declared in place
/// (`[modifiers] val|var name: Type [= default]`, the modifiers in `param.modifiers`).
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorParam {
    /// `val`/`var`: also a property of the class.
    pub property: Option<Mutability>,
    pub param: Param,
}

/// A supertype in `: A(args), B, C by delegate`.
#[derive(Clone, Debug, PartialEq)]
pub struct SuperType {
    pub ty: Type,
    /// `Base(args)`: the superclass's constructor called (a sealed class's case gives its arguments so).
    pub constructor_args: Option<Vec<Arg>>,
    /// `Interface by value`: implemented by delegation.
    pub delegate: Option<Expr>,
    pub loc: Loc,
}

/// An enum class's entry: `[annotations] NAME[(args)] [{ body }]`.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumEntry {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub args: Vec<Arg>,
    /// What it overrides of its class's (`PLUS { override fun apply(a: Int, b: Int) = a + b }`).
    pub body: Option<ClassBody>,
    pub loc: Loc,
}

/// A class's, an object's or an enum entry's body: its members in declaration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassBody {
    pub members: Vec<Member>,
    /// None: no body was written.
    pub loc: Option<Loc>,
}

/// A member of a class body.
#[derive(Clone, Debug, PartialEq)]
pub enum Member {
    /// Functions, properties, nested and inner classes, objects (the companion among them), type aliases.
    Decl(Decl),
    /// `init { … }`.
    Init(Block),
    /// `constructor(params) [: this(args) | super(args)] [{ … }]`.
    Constructor(Box<SecondaryConstructor>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryConstructor {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub params: Vec<Param>,
    pub delegation: Option<ConstructorDelegation>,
    pub body: Option<Block>,
    pub loc: Loc,
}

/// `: this(args)` or `: super(args)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorDelegation {
    pub to_super: bool,
    pub args: Vec<Arg>,
    pub loc: Loc,
}

/// `[annotations] [modifiers] object Name [: supers] [{ members }]`; a class's `companion object [Name] { … }`
/// (no name: `Companion`); `data object Name`.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub name: Option<Ident>,
    pub supers: Vec<SuperType>,
    pub body: ClassBody,
    pub loc: Loc,
}

impl Object {
    pub fn is_companion(&self) -> bool {
        self.modifiers.has(ModifierKind::Companion)
    }

    pub fn is_data(&self) -> bool {
        self.modifiers.has(ModifierKind::Data)
    }
}

/// `[annotations] [modifiers] typealias Name[<T>] = Type`.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeAlias {
    pub annotations: Vec<Annotation>,
    pub modifiers: Modifiers,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub ty: Type,
    pub loc: Loc,
}

// ---- Statements ------------------------------------------------------------------------------------------------

/// `{ statements }`, or the one statement a control structure has without braces (`if (a) x = 1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Written with braces.
    pub braced: bool,
    pub loc: Loc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub loc: Loc,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    /// A local declaration: `val`/`var` (destructuring too), `fun`, `class`, `object`, `typealias`.
    Decl(Decl),
    /// `target = value`, `target += value`, …: Kotlin's assignments are statements.
    Assign { target: Expr, op: AssignOp, value: Expr },
    /// `[label@] for (binding[: Type] in iterable) body`.
    For { label: Option<Ident>, binding: Binding, ty: Option<Type>, iterable: Expr, body: Block },
    /// `[label@] while (condition) body`.
    While { label: Option<Ident>, condition: Expr, body: Block },
    /// `[label@] do body while (condition)`.
    DoWhile { label: Option<Ident>, body: Block, condition: Expr },
    /// Any expression (a call, `if`, `when`, `return`, …).
    Expr(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
    /// `%=`
    Rem,
}

// ---- Expressions -----------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub loc: Loc,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    /// A name: a local, a parameter, a property, a function or class being called, `it`, `field`.
    Name(Ident),
    /// `16`, `0xFF00FF00`, `10L`, `1.5f`, `1_000`, `1e3`, `2u`: see [`Number`].
    Number(Number),
    /// `"…"`, `"""…"""` with their template parts.
    String(StringLit),
    /// `'c'`, `'\n'`, `'\u0041'`: the character.
    Char(char),
    Bool(bool),
    Null,
    /// `this`, `this@Label`.
    This {
        label: Option<Ident>,
    },
    /// `super`, `super<Base>`, `super@Label`.
    Super {
        ty: Option<Box<Type>>,
        label: Option<Ident>,
    },
    /// `callee<T>(args) [trailing lambda]`, `callee { … }`. A trailing lambda is not in `args`.
    Call {
        callee: Box<Expr>,
        type_args: Vec<TypeArg>,
        args: Vec<Arg>,
        trailing_lambda: Option<Box<Lambda>>,
    },
    /// `receiver.name`, `receiver?.name` (`safe`).
    Member {
        receiver: Box<Expr>,
        safe: bool,
        name: Ident,
    },
    /// `receiver[indices]`.
    Index {
        receiver: Box<Expr>,
        indices: Vec<Expr>,
    },
    /// `::name`, `Type::name`, `value::name`, `Type::class` (`name` is `class`).
    CallableRef {
        receiver: Option<Box<Expr>>,
        name: Ident,
    },
    Lambda(Lambda),
    /// `fun(x: Int): Int { … }` / `fun(x: Int) = …` (its name is empty).
    AnonymousFunction(Box<Function>),
    /// `object : Base(), Interface { … }`.
    Object {
        supers: Vec<SuperType>,
        body: ClassBody,
    },
    /// `-x`, `+x`, `!x`, `++x`, `--x`.
    Prefix {
        op: PrefixOp,
        operand: Box<Expr>,
    },
    /// `x++`, `x--`, `x!!`.
    Postfix {
        op: PostfixOp,
        operand: Box<Expr>,
    },
    /// Operators: arithmetic, comparison, equality, logic, ranges, `?:`, `in`.
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// An infix call: `a until b`, `x to y`, `1 downTo 0`, `0..10 step 2`, a user's `infix fun`.
    Infix {
        name: Ident,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// `x is T`, `x !is T`.
    Is {
        expr: Box<Expr>,
        negated: bool,
        ty: Box<Type>,
    },
    /// `x as T`, `x as? T`.
    As {
        expr: Box<Expr>,
        safe: bool,
        ty: Box<Type>,
    },
    /// `if (condition) then [else otherwise]`.
    If {
        condition: Box<Expr>,
        then: Block,
        otherwise: Option<Block>,
    },
    When(When),
    Try(Try),
    /// `(expr)`.
    Paren(Box<Expr>),
    /// `return[@label] [value]`.
    Return {
        label: Option<Ident>,
        value: Option<Box<Expr>>,
    },
    /// `break[@label]`.
    Break {
        label: Option<Ident>,
    },
    /// `continue[@label]`.
    Continue {
        label: Option<Ident>,
    },
    /// `throw value`.
    Throw(Box<Expr>),
    /// `@Ann expr`: annotations on an expression (`@Composable { … }`, `@Suppress("x") a + b` at a statement's
    /// start, where they are the whole statement's).
    Annotated {
        annotations: Vec<Annotation>,
        expr: Box<Expr>,
    },
    /// What the parser left where it recovered (a diagnostic says why).
    Error,
}

/// A number as written, its kind from its form and suffix.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Number {
    /// The source text (`0xFF_00L`), separators and suffix included.
    pub text: String,
    pub kind: NumberKind,
    pub radix: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumberKind {
    /// No suffix, no fraction: an `Int` (a `Long` when it doesn't fit, as Kotlin's).
    Int,
    /// `L`.
    Long,
    /// `u`/`U`.
    UInt,
    /// `uL`/`UL`.
    ULong,
    /// `f`/`F`.
    Float,
    /// A fraction or an exponent without `f` (Kotlin's `Double`).
    Double,
}

impl Number {
    /// Its digits without the radix prefix, separators or suffix (`0xFF_00L` → `FF00`).
    pub fn digits(&self) -> String {
        let t = self.text.as_str();
        // (`f`/`F` are digits of a hex number, a suffix only of a decimal one)
        let t = if self.radix != 10 {
            t[2..].trim_end_matches(['L', 'u', 'U'])
        } else {
            t.trim_end_matches(['f', 'F', 'L', 'u', 'U'])
        };
        t.chars().filter(|&c| c != '_').collect()
    }
}

/// A string literal: its parts in order (an empty string has none).
#[derive(Clone, Debug, PartialEq)]
pub struct StringLit {
    /// `"""…"""`.
    pub raw: bool,
    pub parts: Vec<StringPart>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StringPart {
    /// Text, its escapes resolved (`"a\tb"` → a, tab, b).
    Text { text: String, loc: Loc },
    /// `$name` (`short`) or `${expression}`.
    Expr { expr: Expr, short: bool, loc: Loc },
}

/// `[label@] { [params ->] statements }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Lambda {
    pub label: Option<Ident>,
    /// None: none written (`it` when it takes one); Some(empty): `{ -> … }`.
    pub params: Option<Vec<LambdaParam>>,
    pub body: Block,
    pub loc: Loc,
}

/// A lambda's parameter: a name (`_` too) or a destructuring (`(key, value)`), with an optional type.
#[derive(Clone, Debug, PartialEq)]
pub struct LambdaParam {
    pub binding: Binding,
    pub ty: Option<Type>,
    pub loc: Loc,
}

/// A call's argument: `value`, `name = value`, `*spread`.
#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub name: Option<Ident>,
    pub spread: bool,
    pub value: Expr,
    pub loc: Loc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrefixOp {
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `!`
    Not,
    /// `++`
    Inc,
    /// `--`
    Dec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PostfixOp {
    /// `++`
    Inc,
    /// `--`
    Dec,
    /// `!!`
    NotNull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// `||`
    Or,
    /// `&&`
    And,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
    /// `===`
    Identical,
    /// `!==`
    NotIdentical,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `in`
    In,
    /// `!in`
    NotIn,
    /// `?:`
    Elvis,
    /// `..`
    RangeTo,
    /// `..<`
    RangeUntil,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
}

/// `when [(subject)] { branches }`.
#[derive(Clone, Debug, PartialEq)]
pub struct When {
    pub subject: Option<Box<WhenSubject>>,
    pub branches: Vec<WhenBranch>,
}

/// `when (x)` or `when (val y = f())`.
#[derive(Clone, Debug, PartialEq)]
pub struct WhenSubject {
    /// `val y[: T] =`.
    pub binding: Option<(Ident, Option<Type>)>,
    pub expr: Expr,
}

/// `conditions [if guard] -> body`, or `else -> body` (`conditions` empty).
#[derive(Clone, Debug, PartialEq)]
pub struct WhenBranch {
    pub conditions: Vec<WhenCondition>,
    /// Kotlin 2.1's guard: `is Loaded if data.isEmpty() -> …`.
    pub guard: Option<Expr>,
    pub body: Block,
    pub loc: Loc,
}

impl WhenBranch {
    pub fn is_else(&self) -> bool {
        self.conditions.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WhenCondition {
    /// A value equal to the subject (or, without a subject, a condition).
    Expr(Expr),
    /// `in range`, `!in range`.
    In { negated: bool, range: Expr, loc: Loc },
    /// `is T`, `!is T`.
    Is { negated: bool, ty: Type, loc: Loc },
}

/// `try { … } catch (e: T) { … } … [finally { … }]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Try {
    pub body: Block,
    pub catches: Vec<Catch>,
    pub finally: Option<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Catch {
    pub name: Ident,
    pub ty: Type,
    pub body: Block,
    pub loc: Loc,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pos::Pos;

    fn ident(name: &str) -> Ident {
        Ident { name: name.into(), backticked: false, loc: Loc::at(Pos::default()) }
    }

    #[test]
    fn kompozable_annotations_and_modifiers() {
        let path = |names: &[&str]| Path { segments: names.iter().map(|n| ident(n)).collect(), loc: Loc::default() };
        let a = |names: &[&str]| Annotation {
            use_site: None,
            name: path(names),
            type_args: Vec::new(),
            args: None,
            loc: Loc::default(),
        };
        assert!(has_kompozable(&[a(&["Preview"]), a(&["Composable"])]));
        assert!(has_kompozable(&[a(&["androidx", "compose", "runtime", "Composable"])]));
        assert!(!has_kompozable(&[a(&["Preview"])]));
        assert_eq!(path(&["a", "b", "C"]).dotted(), "a.b.C");

        let m = Modifiers {
            list: vec![
                Modifier { kind: ModifierKind::from_keyword("private").unwrap(), loc: Loc::default() },
                Modifier { kind: ModifierKind::Data, loc: Loc::default() },
            ],
        };
        assert_eq!(m.visibility(), Visibility::Private);
        assert!(m.has(ModifierKind::Data) && !m.has(ModifierKind::Sealed));
        assert_eq!(Modifiers::default().visibility(), Visibility::Public);
        assert_eq!(ModifierKind::Lateinit.keyword(), "lateinit");
    }

    #[test]
    fn number_digits() {
        let n = Number { text: "0xFF_00L".into(), kind: NumberKind::Long, radix: 16 };
        assert_eq!(n.digits(), "FF00");
        let f = Number { text: "1_000.5f".into(), kind: NumberKind::Float, radix: 10 };
        assert_eq!(f.digits(), "1000.5");
        let h = Number { text: "0xFFu".into(), kind: NumberKind::UInt, radix: 16 };
        assert_eq!(h.digits(), "FF");
    }
}
