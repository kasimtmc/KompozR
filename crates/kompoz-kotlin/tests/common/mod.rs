//! What the parse tests share: lexing and parsing a text, and the tree written compactly (an S-expression-like
//! form without positions) so a test states the structure it expects in one line.
//!
//! The form: names as written (`` `a b` `` backticked), literals as Kotlin writes them, `(op lhs rhs)` for
//! operators, `(call f args… {lambda})`, `(. a b)`/`(?. a b)` for members, `{a b -> s1; s2}` for lambdas, `{s1; s2}`
//! for braced blocks (an unbraced body is its statement, `[]` when empty), `(fun …)`, `(val …)`, `(class …)` for
//! declarations, types as Kotlin writes them.

#![allow(dead_code)]

use kompoz_kotlin::ast::*;
use kompoz_kotlin::lexer::{Token, lex};
use kompoz_kotlin::parser::parse;
use kompoz_kotlin::{Diagnostic, Diagnostics, Pos, Severity};

/// The tokens of `text` (from 1:1) and the diagnostics.
pub fn tokens(text: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut d = Diagnostics::default();
    let t = lex(text, Pos::start(1, 1), &mut d);
    (t, d.list)
}

/// The file `text` (from 1:1) makes and the diagnostics.
pub fn parse_text(text: &str) -> (File, Vec<Diagnostic>) {
    let mut d = Diagnostics::default();
    let t = lex(text, Pos::start(1, 1), &mut d);
    let f = parse(&t, &mut d);
    (f, d.list)
}

/// The file `text` makes, which must have no diagnostics.
pub fn parse_ok(text: &str) -> File {
    let (f, d) = parse_text(text);
    assert!(d.is_empty(), "diagnostics for {text:?}:\n{}", render(&d));
    f
}

/// The diagnostics, one per line: `line:col: message` (`warning:` for a warning).
pub fn render(d: &[Diagnostic]) -> String {
    d.iter()
        .map(|d| {
            let w = if d.severity == Severity::Warning { "warning: " } else { "" };
            format!("{}: {w}{}", d.loc.start, d.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The declarations of `text` written out, one per line.
pub fn decls(text: &str) -> String {
    parse_ok(text).decls.iter().map(decl).collect::<Vec<_>>().join("\n")
}

/// The statements of `text`'s body (`text` is put in a function's body), written out, one per line.
pub fn stmts(text: &str) -> String {
    let f = parse_ok(&format!("fun t() {{\n{text}\n}}"));
    let Decl::Function(f) = &f.decls[0] else { panic!("a function") };
    let Some(FunctionBody::Block(b)) = &f.body else { panic!("a block") };
    b.stmts.iter().map(stmt).collect::<Vec<_>>().join("\n")
}

/// The expression `text` (an expression body) written out.
pub fn expr_of(text: &str) -> String {
    let f = parse_ok(&format!("fun t() = {text}"));
    let Decl::Function(f) = &f.decls[0] else { panic!("a function") };
    let Some(FunctionBody::Expr(e)) = &f.body else { panic!("an expression body") };
    expr(e)
}

fn join<T>(items: &[T], f: impl Fn(&T) -> String, sep: &str) -> String {
    items.iter().map(f).collect::<Vec<_>>().join(sep)
}

pub fn ident(i: &Ident) -> String {
    if i.backticked { format!("`{}`", i.name) } else { i.name.clone() }
}

pub fn path(p: &Path) -> String {
    join(&p.segments, ident, ".")
}

pub fn annotation(a: &Annotation) -> String {
    let mut s = String::from("@");
    if let Some(u) = &a.use_site {
        s += &format!("{}:", u.name);
    }
    s += &path(&a.name);
    if !a.type_args.is_empty() {
        s += &format!("<{}>", join(&a.type_args, type_arg, ", "));
    }
    if let Some(args) = &a.args {
        s += &format!("({})", join(args, arg, " "));
    }
    s
}

fn head(annotations: &[Annotation], modifiers: &Modifiers) -> String {
    let mut parts: Vec<String> = annotations.iter().map(annotation).collect();
    parts.extend(modifiers.list.iter().map(|m| m.kind.keyword().to_string()));
    parts.iter().map(|p| format!("{p} ")).collect()
}

pub fn ty(t: &Type) -> String {
    let mut s = join(&t.annotations, |a| annotation(a) + " ", "");
    let inner = match &t.kind {
        TypeKind::Named(segs) => join(
            segs,
            |seg| {
                if seg.args.is_empty() {
                    ident(&seg.name)
                } else {
                    format!("{}<{}>", ident(&seg.name), join(&seg.args, type_arg, ", "))
                }
            },
            ".",
        ),
        TypeKind::Function(f) => {
            let mut s = String::new();
            if f.suspend {
                s += "suspend ";
            }
            if let Some(r) = &f.receiver {
                s += &format!("{}.", receiver(r));
            }
            let params = join(
                &f.params,
                |p| match &p.name {
                    Some(n) => format!("{}: {}", ident(n), ty(&p.ty)),
                    None => ty(&p.ty),
                },
                ", ",
            );
            s += &format!("({params}) -> {}", ty(&f.result));
            if t.nullable { format!("({s})") } else { s }
        }
        TypeKind::Error => "<error>".into(),
    };
    s += &inner;
    if t.nullable {
        s += "?";
    }
    s
}

/// A receiver type as written before its `.`: a function type in parentheses.
fn receiver(t: &Type) -> String {
    match t.kind {
        TypeKind::Function(_) if !t.nullable => format!("({})", ty(t)),
        _ => ty(t),
    }
}

pub fn type_arg(a: &TypeArg) -> String {
    match a {
        TypeArg::Star(_) => "*".into(),
        TypeArg::Type { variance, ty: t } => {
            let v = match variance {
                Some(Variance::In) => "in ",
                Some(Variance::Out) => "out ",
                None => "",
            };
            format!("{v}{}", ty(t))
        }
    }
}

fn type_params(ps: &[TypeParam]) -> String {
    if ps.is_empty() {
        return String::new();
    }
    let p = |p: &TypeParam| {
        let mut s = join(&p.annotations, |a| annotation(a) + " ", "");
        match p.variance {
            Some(Variance::In) => s += "in ",
            Some(Variance::Out) => s += "out ",
            None => {}
        }
        if p.reified {
            s += "reified ";
        }
        s += &ident(&p.name);
        if let Some(b) = &p.bound {
            s += &format!(": {}", ty(b));
        }
        s
    };
    format!("<{}>", join(ps, p, ", "))
}

fn constraints(cs: &[TypeConstraint]) -> String {
    if cs.is_empty() {
        return String::new();
    }
    format!(" where {}", join(cs, |c| format!("{}: {}", ident(&c.name), ty(&c.bound)), ", "))
}

pub fn param(p: &Param) -> String {
    let mut s = head(&p.annotations, &p.modifiers) + &ident(&p.name);
    if let Some(t) = &p.ty {
        s += &format!(": {}", ty(t));
    }
    if let Some(d) = &p.default {
        s += &format!(" = {}", expr(d));
    }
    s
}

fn body(b: &FunctionBody) -> String {
    match b {
        FunctionBody::Block(b) => format!(" {}", block(b)),
        FunctionBody::Expr(e) => format!(" = {}", expr(e)),
    }
}

pub fn function(f: &Function) -> String {
    let mut s = format!("(fun {}{}", head(&f.annotations, &f.modifiers), type_params(&f.type_params));
    if !f.type_params.is_empty() {
        s += " ";
    }
    if let Some(r) = &f.receiver {
        s += &format!("{}.", receiver(r));
    }
    s += &ident(&f.name);
    s += &format!("({})", join(&f.params, param, ", "));
    if let Some(r) = &f.result {
        s += &format!(": {}", ty(r));
    }
    s += &constraints(&f.constraints);
    if let Some(b) = &f.body {
        s += &body(b);
    }
    s + ")"
}

fn binding(b: &Binding) -> String {
    match b {
        Binding::Name(n) => ident(n),
        Binding::Destructure(es) => format!(
            "({})",
            join(
                es,
                |e| match &e.ty {
                    Some(t) => format!("{}: {}", ident(&e.name), ty(t)),
                    None => ident(&e.name),
                },
                ", "
            )
        ),
    }
}

fn accessor(word: &str, a: &Accessor) -> String {
    let mut s = format!(" {}{word}", head(&a.annotations, &a.modifiers));
    if a.body.is_some() || a.param.is_some() {
        s += &format!("({})", a.param.as_ref().map(param).unwrap_or_default());
    }
    if let Some(t) = &a.ty {
        s += &format!(": {}", ty(t));
    }
    if let Some(b) = &a.body {
        s += &body(b);
    }
    s
}

pub fn property(p: &Property) -> String {
    let word = if p.is_var() { "var" } else { "val" };
    let mut s = format!("({word} {}{}", head(&p.annotations, &p.modifiers), type_params(&p.type_params));
    if !p.type_params.is_empty() {
        s += " ";
    }
    if let Some(r) = &p.receiver {
        s += &format!("{}.", receiver(r));
    }
    s += &binding(&p.binding);
    if let Some(t) = &p.ty {
        s += &format!(": {}", ty(t));
    }
    s += &constraints(&p.constraints);
    match &p.init {
        PropertyInit::None => {}
        PropertyInit::Value(e) => s += &format!(" = {}", expr(e)),
        PropertyInit::Delegate(e) => s += &format!(" by {}", expr(e)),
    }
    if let Some(g) = &p.getter {
        s += &accessor("get", g);
    }
    if let Some(g) = &p.setter {
        s += &accessor("set", g);
    }
    s + ")"
}

fn supers(ss: &[SuperType]) -> String {
    if ss.is_empty() {
        return String::new();
    }
    let one = |st: &SuperType| {
        let mut s = ty(&st.ty);
        if let Some(args) = &st.constructor_args {
            s += &format!("({})", join(args, arg, " "));
        }
        if let Some(d) = &st.delegate {
            s += &format!(" by {}", expr(d));
        }
        s
    };
    format!(" : {}", join(ss, one, ", "))
}

fn class_body(b: &ClassBody) -> String {
    if b.loc.is_none() {
        return String::new();
    }
    let m = |m: &Member| match m {
        Member::Decl(d) => decl(d),
        Member::Init(b) => format!("(init {})", block(b)),
        Member::Constructor(c) => {
            let mut s =
                format!("(constructor {}({})", head(&c.annotations, &c.modifiers), join(&c.params, param, ", "));
            if let Some(d) = &c.delegation {
                s += &format!(" : {}({})", if d.to_super { "super" } else { "this" }, join(&d.args, arg, " "));
            }
            if let Some(b) = &c.body {
                s += &format!(" {}", block(b));
            }
            s + ")"
        }
    };
    format!(" {{{}}}", join(&b.members, m, "; "))
}

pub fn decl(d: &Decl) -> String {
    match d {
        Decl::Function(f) => function(f),
        Decl::Property(p) => property(p),
        Decl::Class(c) => {
            let kind = match c.kind {
                ClassKind::Class => "class",
                ClassKind::Interface => "interface",
                ClassKind::FunInterface => "fun interface",
                ClassKind::Enum => "enum class",
                ClassKind::Annotation => "annotation class",
            };
            let mut s = format!(
                "({kind} {}{}{}",
                head(&c.annotations, &c.modifiers),
                ident(&c.name),
                type_params(&c.type_params)
            );
            if let Some(pc) = &c.primary_constructor {
                if !pc.annotations.is_empty() || !pc.modifiers.list.is_empty() {
                    s += &format!(" {}constructor", head(&pc.annotations, &pc.modifiers));
                }
                let p = |cp: &ConstructorParam| {
                    let word = match cp.property {
                        Some(Mutability::Val) => "val ",
                        Some(Mutability::Var) => "var ",
                        None => "",
                    };
                    let h = head(&cp.param.annotations, &cp.param.modifiers);
                    let mut rest = param(&cp.param);
                    rest.replace_range(..h.len(), "");
                    format!("{h}{word}{rest}")
                };
                s += &format!("({})", join(&pc.params, p, ", "));
            }
            s += &supers(&c.supers);
            s += &constraints(&c.constraints);
            if !c.enum_entries.is_empty() {
                let e = |e: &EnumEntry| {
                    let mut s = join(&e.annotations, |a| annotation(a) + " ", "") + &ident(&e.name);
                    if !e.args.is_empty() {
                        s += &format!("({})", join(&e.args, arg, " "));
                    }
                    if let Some(b) = &e.body {
                        s += &class_body(b);
                    }
                    s
                };
                s += &format!(" [{}]", join(&c.enum_entries, e, ", "));
            }
            s += &class_body(&c.body);
            s + ")"
        }
        Decl::Object(o) => {
            let name = o.name.as_ref().map(|n| format!(" {}", ident(n))).unwrap_or_default();
            format!(
                "(object {}{}{}{})",
                head(&o.annotations, &o.modifiers).trim_end(),
                name,
                supers(&o.supers),
                class_body(&o.body)
            )
            .replace("(object  ", "(object ")
        }
        Decl::TypeAlias(a) => {
            format!(
                "(typealias {}{}{} = {})",
                head(&a.annotations, &a.modifiers),
                ident(&a.name),
                type_params(&a.type_params),
                ty(&a.ty)
            )
        }
        Decl::Failed { name, .. } => format!("<failed {}>", name.as_ref().map(ident).unwrap_or_default()),
    }
}

pub fn block(b: &Block) -> String {
    if b.braced {
        format!("{{{}}}", join(&b.stmts, stmt, "; "))
    } else if b.stmts.is_empty() {
        "[]".into()
    } else {
        join(&b.stmts, stmt, "; ")
    }
}

fn label(l: &Option<Ident>) -> String {
    l.as_ref().map(|l| format!("@{}", ident(l))).unwrap_or_default()
}

pub fn stmt(s: &Stmt) -> String {
    match &s.kind {
        StmtKind::Decl(d) => decl(d),
        StmtKind::Assign { target, op, value } => {
            let op = match op {
                AssignOp::Assign => "=",
                AssignOp::Add => "+=",
                AssignOp::Sub => "-=",
                AssignOp::Mul => "*=",
                AssignOp::Div => "/=",
                AssignOp::Rem => "%=",
            };
            format!("({op} {} {})", expr(target), expr(value))
        }
        StmtKind::For { label: l, binding: b, ty: t, iterable, body } => {
            let t = t.as_ref().map(|t| format!(": {}", ty(t))).unwrap_or_default();
            format!("(for{} {}{t} in {} {})", label(l), binding(b), expr(iterable), block(body))
        }
        StmtKind::While { label: l, condition, body } => {
            format!("(while{} {} {})", label(l), expr(condition), block(body))
        }
        StmtKind::DoWhile { label: l, body, condition } => {
            format!("(do{} {} while {})", label(l), block(body), expr(condition))
        }
        StmtKind::Expr(e) => expr(e),
    }
}

pub fn arg(a: &Arg) -> String {
    let v = if a.spread { format!("*{}", expr(&a.value)) } else { expr(&a.value) };
    match &a.name {
        Some(n) => format!("{}={v}", ident(n)),
        None => v,
    }
}

pub fn lambda(l: &Lambda) -> String {
    let mut s = l.label.as_ref().map(|l| format!("{}@", ident(l))).unwrap_or_default();
    s += "{";
    if let Some(ps) = &l.params {
        let p = |p: &LambdaParam| match &p.ty {
            Some(t) => format!("{}: {}", binding(&p.binding), ty(t)),
            None => binding(&p.binding),
        };
        s += &join(ps, p, ", ");
        s += if ps.is_empty() { "-> " } else { " -> " };
    }
    s += &join(&l.body.stmts, stmt, "; ");
    s + "}"
}

pub fn string(s: &StringLit) -> String {
    let q = if s.raw { "\"\"\"" } else { "\"" };
    let mut out = q.to_string();
    for p in &s.parts {
        match p {
            StringPart::Text { text, .. } => out += &text.escape_debug().to_string(),
            StringPart::Expr { expr: e, short: true, .. } => out += &format!("${}", expr(e)),
            StringPart::Expr { expr: e, short: false, .. } => out += &format!("${{{}}}", expr(e)),
        }
    }
    out + q
}

pub fn expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Name(n) => ident(n),
        ExprKind::Number(n) => n.text.clone(),
        ExprKind::String(s) => string(s),
        ExprKind::Char(c) => format!("'{}'", c.escape_debug()),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Null => "null".into(),
        ExprKind::This { label: l } => format!("this{}", label(l)),
        ExprKind::Super { ty: t, label: l } => {
            format!("super{}{}", t.as_ref().map(|t| format!("<{}>", ty(t))).unwrap_or_default(), label(l))
        }
        ExprKind::Call { callee, type_args, args, trailing_lambda } => {
            let mut s = format!("(call {}", expr(callee));
            if !type_args.is_empty() {
                s += &format!("<{}>", join(type_args, type_arg, ", "));
            }
            for a in args {
                s += &format!(" {}", arg(a));
            }
            if let Some(l) = trailing_lambda {
                s += &format!(" {}", lambda(l));
            }
            s + ")"
        }
        ExprKind::Member { receiver, safe, name } => {
            format!("({} {} {})", if *safe { "?." } else { "." }, expr(receiver), ident(name))
        }
        ExprKind::Index { receiver, indices } => format!("([] {} {})", expr(receiver), join(indices, expr, " ")),
        ExprKind::CallableRef { receiver, name } => match receiver {
            Some(r) => format!("(:: {} {})", expr(r), ident(name)),
            None => format!("(:: {})", ident(name)),
        },
        ExprKind::Lambda(l) => lambda(l),
        ExprKind::AnonymousFunction(f) => function(f),
        ExprKind::Object { supers: s, body } => format!("(object{}{})", supers(s), class_body(body)),
        ExprKind::Prefix { op, operand } => {
            let op = match op {
                PrefixOp::Plus => "+",
                PrefixOp::Minus => "-",
                PrefixOp::Not => "!",
                PrefixOp::Inc => "++",
                PrefixOp::Dec => "--",
            };
            format!("({op} {})", expr(operand))
        }
        ExprKind::Postfix { op, operand } => {
            let op = match op {
                PostfixOp::Inc => "++",
                PostfixOp::Dec => "--",
                PostfixOp::NotNull => "!!",
            };
            format!("({} {op})", expr(operand))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            use BinaryOp::*;
            let op = match op {
                Or => "||",
                And => "&&",
                Eq => "==",
                NotEq => "!=",
                Identical => "===",
                NotIdentical => "!==",
                Lt => "<",
                Gt => ">",
                Le => "<=",
                Ge => ">=",
                In => "in",
                NotIn => "!in",
                Elvis => "?:",
                RangeTo => "..",
                RangeUntil => "..<",
                Add => "+",
                Sub => "-",
                Mul => "*",
                Div => "/",
                Rem => "%",
            };
            format!("({op} {} {})", expr(lhs), expr(rhs))
        }
        ExprKind::Infix { name, lhs, rhs } => format!("({} {} {})", ident(name), expr(lhs), expr(rhs)),
        ExprKind::Is { expr: x, negated, ty: t } => {
            format!("({} {} {})", if *negated { "!is" } else { "is" }, expr(x), ty(t))
        }
        ExprKind::As { expr: x, safe, ty: t } => {
            format!("({} {} {})", if *safe { "as?" } else { "as" }, expr(x), ty(t))
        }
        ExprKind::If { condition, then, otherwise } => match otherwise {
            Some(o) => format!("(if {} {} else {})", expr(condition), block(then), block(o)),
            None => format!("(if {} {})", expr(condition), block(then)),
        },
        ExprKind::When(w) => {
            let mut s = String::from("(when");
            if let Some(subject) = &w.subject {
                s += " (";
                if let Some((n, t)) = &subject.binding {
                    s += &format!("val {}", ident(n));
                    if let Some(t) = t {
                        s += &format!(": {}", ty(t));
                    }
                    s += " = ";
                }
                s += &expr(&subject.expr);
                s += ")";
            }
            for b in &w.branches {
                let conds = if b.is_else() {
                    "else".to_string()
                } else {
                    join(
                        &b.conditions,
                        |c| match c {
                            WhenCondition::Expr(e) => expr(e),
                            WhenCondition::In { negated, range, .. } => {
                                format!("{} {}", if *negated { "!in" } else { "in" }, expr(range))
                            }
                            WhenCondition::Is { negated, ty: t, .. } => {
                                format!("{} {}", if *negated { "!is" } else { "is" }, ty(t))
                            }
                        },
                        ", ",
                    )
                };
                let guard = b.guard.as_ref().map(|g| format!(" if {}", expr(g))).unwrap_or_default();
                s += &format!(" ({conds}{guard} -> {})", block(&b.body));
            }
            s + ")"
        }
        ExprKind::Try(t) => {
            let mut s = format!("(try {}", block(&t.body));
            for c in &t.catches {
                s += &format!(" (catch {}: {} {})", ident(&c.name), ty(&c.ty), block(&c.body));
            }
            if let Some(f) = &t.finally {
                s += &format!(" (finally {})", block(f));
            }
            s + ")"
        }
        ExprKind::Paren(x) => format!("(paren {})", expr(x)),
        ExprKind::Return { label: l, value } => match value {
            Some(v) => format!("(return{} {})", label(l), expr(v)),
            None => format!("(return{})", label(l)),
        },
        ExprKind::Break { label: l } => format!("(break{})", label(l)),
        ExprKind::Continue { label: l } => format!("(continue{})", label(l)),
        ExprKind::Throw(x) => format!("(throw {})", expr(x)),
        ExprKind::Annotated { annotations, expr: x } => {
            format!("({} {})", join(annotations, annotation, " "), expr(x))
        }
        ExprKind::Error => "<error>".into(),
    }
}

/// `tree` with every position zeroed (`Debug` output with `Pos { … }` replaced): two trees of the same text
/// written differently compare equal.
pub fn without_positions(debug: &str) -> String {
    let mut out = String::with_capacity(debug.len());
    let mut rest = debug;
    while let Some(i) = rest.find("Pos {") {
        out += &rest[..i];
        let after = &rest[i..];
        let end = after.find('}').expect("a closing brace") + 1;
        out += "Pos";
        rest = &after[end..];
    }
    out + rest
}
