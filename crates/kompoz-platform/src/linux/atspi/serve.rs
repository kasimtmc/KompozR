//! What the bridge answers (C++'s `AtspiBridge::State::answer` and `property`): the AT-SPI interfaces of the
//! application, its window and its nodes, read from the tree last published; what an assistive technology asks the
//! app to do kept as requests.

use super::tree::*;
use crate::linux::dbus::{Message, Type, Value};
use kompoz_ui::semantics::SemanticsAction;

pub const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
pub const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";

/// What an assistive technology asked the app to do: an action on the node `id`.
#[derive(Clone, PartialEq, Debug)]
pub struct Request {
    pub id: u64,
    pub action: SemanticsAction,
}

/// What the bus's thread knows of the accessibility bus: its own name there, the registry's desktop (the
/// application's parent).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bus {
    pub unique_name: String,
    pub desktop_name: String,
    pub desktop_path: String,
}

/// What the bridge serves, under its mutex: the tree last published, the events and requests not yet taken, the
/// id the registry gave the application.
#[derive(Default)]
pub struct Served {
    pub tree: Tree,
    pub events: Vec<Event>,
    pub requests: Vec<Request>,
    pub app_id: i32,
}

/// A call's arguments read in order; one of another type than asked for reads as nothing and makes `ok` false (C++'s
/// Reader).
struct Args<'a> {
    values: &'a [Value],
    next: usize,
    ok: bool,
}

impl<'a> Args<'a> {
    fn new(values: &'a [Value]) -> Args<'a> {
        Args { values, next: 0, ok: true }
    }

    fn take<T>(&mut self, read: impl Fn(&'a Value) -> Option<T>) -> Option<T> {
        let v = self.values.get(self.next).and_then(read);
        if v.is_some() {
            self.next += 1;
        } else {
            self.ok = false;
        }
        v
    }

    fn i32(&mut self) -> i32 {
        self.take(Value::as_i32).unwrap_or(0)
    }

    fn u32(&mut self) -> u32 {
        self.take(Value::as_u32).unwrap_or(0)
    }

    fn str(&mut self) -> String {
        self.take(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }).unwrap_or_default()
    }

    /// What a variant holds.
    fn variant(&mut self) -> Option<&'a Value> {
        self.take(|v| if let Value::Variant(inner) = v { Some(&**inner) } else { None })
    }
}

fn reference(bus: &Bus, o: Object) -> Value {
    Value::Struct(vec![Value::str(bus.unique_name.as_str()), Value::path(o.path())])
}

fn null_reference() -> Value {
    Value::Struct(vec![Value::str(""), Value::path(NULL_PATH)])
}

fn reference_type() -> Type {
    Type::Struct(vec![Type::String, Type::ObjectPath])
}

fn no_attributes() -> Value {
    Value::dict(Type::String, Type::String, Vec::new())
}

/// The names of an interface's properties, in order.
fn property_names(interface: &str) -> &'static [&'static str] {
    match interface {
        ACCESSIBLE => &["Name", "Description", "Parent", "ChildCount", "Locale", "AccessibleId", "HelpText"],
        APPLICATION => &["ToolkitName", "Version", "AtspiVersion", "Id"],
        ACTION => &["NActions"],
        VALUE => &["MinimumValue", "MaximumValue", "MinimumIncrement", "CurrentValue", "Text"],
        TEXT => &["CharacterCount", "CaretOffset"],
        _ => &[],
    }
}

/// A property's value; none: it has none of that name.
fn property(served: &Served, app_name: &str, bus: &Bus, o: Object, interface: &str, name: &str) -> Option<Value> {
    let tree = &served.tree;
    let node = match o {
        Object::Node(id) => Some(tree.node(id)),
        _ => None,
    };
    match (interface, name) {
        (ACCESSIBLE, "Name") => Some(Value::str(tree.name_of(o, app_name))),
        (ACCESSIBLE, "Description") => Some(Value::str(node.map(|n| n.state_description.clone()).unwrap_or_default())),
        (ACCESSIBLE, "Locale" | "AccessibleId" | "HelpText") => Some(Value::str("")),
        (ACCESSIBLE, "ChildCount") => Some(Value::Int32(tree.kids_of(o).len() as i32)),
        (ACCESSIBLE, "Parent") => Some(if o == Object::Root {
            Value::Struct(vec![Value::str(bus.desktop_name.as_str()), Value::path(bus.desktop_path.as_str())])
        } else {
            reference(bus, tree.parent_of(o))
        }),
        (APPLICATION, "ToolkitName") => Some(Value::str("Kompoz")),
        (APPLICATION, "Version") => Some(Value::str("")),
        (APPLICATION, "AtspiVersion") => Some(Value::str("2.1")),
        (APPLICATION, "Id") => Some(Value::Int32(served.app_id)),
        (ACTION, "NActions") => Some(Value::Int32(actions_of(node?).len() as i32)),
        (VALUE, _) => {
            let n = node?;
            let r = n.range.as_ref()?;
            let (start, end) = (*r.range.start(), *r.range.end());
            match name {
                "MinimumValue" => Some(Value::Double(start as f64)),
                "MaximumValue" => Some(Value::Double(end as f64)),
                "CurrentValue" => Some(Value::Double(r.current as f64)),
                "MinimumIncrement" => {
                    Some(Value::Double(if r.steps > 0 { ((end - start) / (r.steps + 1) as f32) as f64 } else { 0.0 }))
                }
                "Text" => Some(Value::str(n.state_description.as_str())),
                _ => None,
            }
        }
        (TEXT, "CharacterCount" | "CaretOffset") => Some(Value::Int32(text_of(node?).chars().count() as i32)),
        _ => None,
    }
}

/// The reply to `call` on `o` (which exists): its values; none: no such method (or arguments it can't take).
pub fn answer(served: &mut Served, app_name: &str, bus: &Bus, call: &Message, o: Object) -> Option<Vec<Value>> {
    let interface = call.interface.as_deref().unwrap_or("");
    let member = call.member.as_deref().unwrap_or("");
    let mut args = Args::new(&call.body);
    if interface == INTROSPECTABLE && member == "Introspect" {
        let mut xml = String::from("<node>");
        for i in served.tree.interfaces_of(o) {
            xml.push_str(&format!("<interface name=\"{i}\"/>"));
        }
        xml.push_str("</node>");
        return Some(vec![Value::str(xml)]);
    }
    if interface == PROPERTIES {
        let iface = args.str();
        if !served.tree.implements(o, &iface) {
            return None;
        }
        return match member {
            "Get" => {
                let name = args.str();
                if !args.ok {
                    return None;
                }
                property(served, app_name, bus, o, &iface, &name).map(|v| vec![Value::variant(v)])
            }
            "GetAll" => {
                let entries = property_names(&iface)
                    .iter()
                    .filter_map(|name| {
                        let v = property(served, app_name, bus, o, &iface, name)?;
                        Some((Value::str(*name), Value::variant(v)))
                    })
                    .collect();
                Some(vec![Value::dict(Type::String, Type::Variant, entries)])
            }
            "Set" => {
                let name = args.str();
                let value = args.variant();
                match (iface.as_str(), name.as_str(), value) {
                    (APPLICATION, "Id", Some(Value::Int32(id))) => served.app_id = *id,
                    (VALUE, "CurrentValue", Some(Value::Double(v))) => {
                        if let Object::Node(id) = o
                            && served.tree.node(id).settable
                        {
                            served.requests.push(Request { id, action: SemanticsAction::SetProgress(*v as f32) });
                        }
                    }
                    _ => return None,
                }
                Some(Vec::new())
            }
            _ => None,
        };
    }
    if !served.tree.implements(o, interface) {
        return None;
    }
    let tree = &served.tree;
    match interface {
        ACCESSIBLE => {
            let kids = tree.kids_of(o);
            Some(match member {
                "GetChildren" => {
                    vec![Value::array(reference_type(), kids.iter().map(|&k| reference(bus, k)).collect())]
                }
                "GetChildAtIndex" => {
                    let i = args.i32();
                    vec![match usize::try_from(i).ok().and_then(|i| kids.get(i)) {
                        Some(&k) => reference(bus, k),
                        None => null_reference(),
                    }]
                }
                "GetIndexInParent" => {
                    let siblings = tree.kids_of(tree.parent_of(o));
                    let i = siblings.iter().position(|&s| s == o).filter(|_| o != Object::Root);
                    vec![Value::Int32(i.map_or(-1, |i| i as i32))]
                }
                "GetRelationSet" => vec![Value::array(
                    Type::Struct(vec![Type::Uint32, Type::Array(Box::new(reference_type()))]),
                    Vec::new(),
                )],
                "GetRole" => vec![Value::Uint32(tree.role_of(o))],
                "GetRoleName" | "GetLocalizedRoleName" => vec![Value::str(role_name(tree.role_of(o)))],
                "GetState" => {
                    let bits = tree.states_of(o);
                    vec![Value::array(Type::Uint32, vec![Value::Uint32(bits[0]), Value::Uint32(bits[1])])]
                }
                "GetAttributes" => vec![no_attributes()],
                "GetApplication" => vec![reference(bus, Object::Root)],
                "GetInterfaces" => {
                    vec![Value::array(Type::String, tree.interfaces_of(o).into_iter().map(Value::str).collect())]
                }
                _ => return None,
            })
        }
        APPLICATION => (member == "GetLocale").then(|| vec![Value::str("")]),
        COMPONENT => Some(match member {
            "GetExtents" => {
                let r = tree.extents_of(o, args.u32());
                vec![Value::Struct(vec![
                    Value::Int32(r.x),
                    Value::Int32(r.y),
                    Value::Int32(r.width),
                    Value::Int32(r.height),
                ])]
            }
            "GetPosition" => {
                let r = tree.extents_of(o, args.u32());
                vec![Value::Int32(r.x), Value::Int32(r.y)]
            }
            "GetSize" => {
                let r = tree.extents_of(o, 0);
                vec![Value::Int32(r.width), Value::Int32(r.height)]
            }
            "Contains" | "GetAccessibleAtPoint" => {
                let (x, y, coord_type) = (args.i32(), args.i32(), args.u32());
                let inside = |k: Object| tree.extents_of(k, coord_type).contains(x, y);
                if member == "Contains" {
                    vec![Value::Boolean(inside(o))]
                } else {
                    // (The last drawn on top.)
                    vec![match tree.kids_of(o).into_iter().rev().find(|&k| inside(k)) {
                        Some(k) => reference(bus, k),
                        None => null_reference(),
                    }]
                }
            }
            "GetLayer" => vec![Value::Uint32(if o == Object::Window { LAYER_WINDOW } else { LAYER_WIDGET })],
            "GetMDIZOrder" => vec![Value::Int16(0)],
            "GrabFocus" => {
                let focusable = match o {
                    Object::Node(id) => {
                        let n = tree.node(id);
                        n.editable || (n.clickable && n.enabled)
                    }
                    _ => false,
                };
                if let (true, Object::Node(id)) = (focusable, o) {
                    served.requests.push(Request { id, action: SemanticsAction::Focus });
                }
                vec![Value::Boolean(focusable)]
            }
            "GetAlpha" => vec![Value::Double(1.0)],
            "ScrollTo" | "ScrollToPoint" | "SetExtents" | "SetPosition" | "SetSize" => vec![Value::Boolean(false)],
            _ => return None,
        }),
        ACTION => {
            let Object::Node(id) = o else { return None };
            let actions = actions_of(tree.node(id));
            if member == "GetActions" {
                let items = actions
                    .iter()
                    .map(|(name, _)| Value::Struct(vec![Value::str(name.as_str()), Value::str(""), Value::str("")]))
                    .collect();
                return Some(vec![Value::array(Type::Struct(vec![Type::String; 3]), items)]);
            }
            let i = args.i32();
            let action = usize::try_from(i).ok().filter(|_| args.ok).and_then(|i| actions.get(i));
            Some(match member {
                "GetName" | "GetLocalizedName" => vec![Value::str(action.map(|a| a.0.clone()).unwrap_or_default())],
                "GetDescription" | "GetKeyBinding" => vec![Value::str("")],
                "DoAction" => {
                    if let Some((_, action)) = action {
                        served.requests.push(Request { id, action: action.clone() });
                    }
                    vec![Value::Boolean(action.is_some())]
                }
                _ => return None,
            })
        }
        TEXT => {
            let Object::Node(id) = o else { return None };
            let text = text_of(tree.node(id));
            let cps = code_points(&text);
            let count = cps.len() as i32 - 1;
            let range = |(from, to): (i32, i32)| {
                vec![Value::str(slice(&text, &cps, from, to)), Value::Int32(from), Value::Int32(to)]
            };
            Some(match member {
                "GetText" => {
                    let (from, to) = (args.i32(), args.i32());
                    vec![Value::str(slice(&text, &cps, from, to))]
                }
                "GetCharacterAtOffset" => {
                    let i = args.i32();
                    let c = if (0..count).contains(&i) { text[cps[i as usize]..].chars().next() } else { None };
                    vec![Value::Int32(c.map_or(0, |c| c as i32))]
                }
                "GetStringAtOffset" => {
                    // AtspiTextGranularity: char, word, sentence, line, paragraph.
                    let (offset, granularity) = (args.i32(), args.u32());
                    let unit = match granularity {
                        0 => Unit::Char,
                        1 => Unit::Word,
                        _ => Unit::Line,
                    };
                    range(segment(&text, &cps, offset, unit))
                }
                "GetTextAtOffset" | "GetTextBeforeOffset" | "GetTextAfterOffset" => {
                    // AtspiTextBoundaryType: char, word start and end, sentence start and end, line start and end.
                    let (offset, boundary) = (args.i32(), args.u32());
                    let unit = match boundary {
                        0 => Unit::Char,
                        1 | 2 => Unit::Word,
                        _ => Unit::Line,
                    };
                    let mut r = segment(&text, &cps, offset, unit);
                    if member == "GetTextBeforeOffset" {
                        r = if r.0 > 0 { segment(&text, &cps, r.0 - 1, unit) } else { (0, 0) };
                    }
                    if member == "GetTextAfterOffset" {
                        r = segment(&text, &cps, r.1, unit);
                    }
                    range(r)
                }
                "GetNSelections" => vec![Value::Int32(0)],
                "GetSelection" => vec![Value::Int32(0), Value::Int32(0)],
                "SetCaretOffset" | "AddSelection" | "RemoveSelection" | "SetSelection" => vec![Value::Boolean(false)],
                "GetAttributes" | "GetAttributeRun" => vec![no_attributes(), Value::Int32(0), Value::Int32(count)],
                "GetDefaultAttributes" | "GetDefaultAttributeSet" => vec![no_attributes()],
                "GetCharacterExtents" | "GetRangeExtents" => {
                    args.i32();
                    if member == "GetRangeExtents" {
                        args.i32();
                    }
                    let r = tree.extents_of(o, args.u32());
                    vec![Value::Int32(r.x), Value::Int32(r.y), Value::Int32(r.width), Value::Int32(r.height)]
                }
                "GetOffsetAtPoint" => vec![Value::Int32(-1)],
                _ => return None,
            })
        }
        EDITABLE_TEXT => {
            let Object::Node(id) = o else { return None };
            let value = tree.node(id).value.clone();
            let cps = code_points(&value);
            let text = match member {
                "SetTextContents" => args.str(),
                "InsertText" => {
                    let position = args.i32();
                    let typed = args.str();
                    let length = args.i32();
                    let typed = slice(&typed, &code_points(&typed), 0, length).to_string();
                    format!("{}{typed}{}", slice(&value, &cps, 0, position), slice(&value, &cps, position, -1))
                }
                "DeleteText" => {
                    let (from, to) = (args.i32(), args.i32());
                    format!("{}{}", slice(&value, &cps, 0, from), slice(&value, &cps, to, -1))
                }
                "CopyText" => return Some(Vec::new()), // (nothing to say)
                "CutText" | "PasteText" => return Some(vec![Value::Boolean(false)]),
                _ => return None,
            };
            if !args.ok {
                return None;
            }
            served.requests.push(Request { id, action: SemanticsAction::SetText(text) });
            Some(vec![Value::Boolean(true)])
        }
        _ => None,
    }
}

/// An event as its signal: (detail, detail1, detail2, data, properties) from the object it is about.
pub fn signal(bus: &Bus, e: &Event) -> Message {
    let data = match &e.data {
        EventData::None => Value::Int32(0),
        EventData::Text(t) => Value::str(t.as_str()),
        EventData::Number(n) => Value::Double(*n),
        EventData::Ref(o) => reference(bus, *o),
    };
    Message::signal(&e.object.path(), e.interface, e.member).with_body(vec![
        Value::str(e.detail.as_str()),
        Value::Int32(e.detail1),
        Value::Int32(e.detail2),
        Value::variant(data),
        Value::empty_properties(),
    ])
}

/// The cache's items (`org.a11y.atspi.Cache.GetItems`): none cached up front, what is asked for is read as it is
/// then.
pub fn cache_items() -> Value {
    Value::array(Type::parse_single("((so)(so)(so)iiassusau)").expect("AT-SPI's cache item"), Vec::new())
}
