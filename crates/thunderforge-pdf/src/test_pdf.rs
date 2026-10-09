//! Small PDFs drawn with lopdf's writer, for tests that need a real document.

use lopdf::content::{Content, Operation};
use lopdf::{Object, Stream, dictionary};

/// What a test document should be.
#[derive(Default)]
pub struct Spec<'a> {
    /// One line of Helvetica per page, at (72, 700).
    pub pages: &'a [&'a str],
    /// `/Rotate` on the page tree's root.
    pub rotate: Option<i64>,
    /// Give the trailer an `/Encrypt` dictionary.
    pub encrypted: bool,
    /// Form-field widgets, each on the page it names.
    pub widgets: &'a [Widget<'a>],
}

/// One widget annotation, the way a fillable form keeps a value.
#[derive(Clone, Copy, Default)]
pub struct Widget<'a> {
    /// Zero-based index into `pages`.
    pub page: usize,
    /// `/T` on the widget itself, or on its parent when `parent` is set.
    pub name: &'a str,
    /// `/V`; `None` leaves the key out.
    pub value: Option<&'a str>,
    /// `/Rect` in PDF space: llx, lly, urx, ury.
    pub rect: [i64; 4],
    /// `/FT /Btn` with a name value instead of `/FT /Tx` with a string.
    pub button: bool,
    /// A parent field whose `/T` prefixes the name, and which holds the
    /// value and type instead of the widget.
    pub parent: Option<&'a str>,
    /// Write the value as UTF-16BE with a byte-order mark.
    pub utf16: bool,
}

/// The bytes of a document drawn to `spec`.
pub fn document(spec: Spec<'_>) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let mut kids: Vec<Object> = Vec::new();
    for text in spec.pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 10.into()]),
                Operation::new("Td", vec![72.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(*text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("encode"),
        ));
        let page_id = doc.new_object_id();
        let mut annots: Vec<Object> = Vec::new();
        for widget in spec.widgets.iter().filter(|w| w.page == kids.len()) {
            annots.push(add_widget(&mut doc, page_id, widget).into());
        }
        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        };
        if !annots.is_empty() {
            page.set("Annots", annots);
        }
        doc.objects.insert(page_id, Object::Dictionary(page));
        kids.push(page_id.into());
    }
    let count = kids.len() as i64;
    let mut pages = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => count,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    };
    if let Some(rotate) = spec.rotate {
        pages.set("Rotate", rotate);
    }
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    if spec.encrypted {
        let encrypt_id = doc.add_object(dictionary! {
            "Filter" => "Standard",
            "V" => 1,
            "R" => 2,
            "O" => Object::string_literal(vec![0u8; 32]),
            "U" => Object::string_literal(vec![0u8; 32]),
            "P" => -4,
        });
        doc.trailer.set("Encrypt", encrypt_id);
    }
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save");
    bytes
}

fn add_widget(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    w: &Widget<'_>,
) -> lopdf::ObjectId {
    let value = w.value.map(|v| {
        if w.button {
            Object::Name(v.as_bytes().to_vec())
        } else if w.utf16 {
            let mut bytes = vec![0xFE, 0xFF];
            for unit in v.encode_utf16() {
                bytes.extend(unit.to_be_bytes());
            }
            Object::String(bytes, lopdf::StringFormat::Hexadecimal)
        } else {
            Object::string_literal(v)
        }
    });
    let kind: Object = if w.button { "Btn".into() } else { "Tx".into() };
    let mut field = dictionary! {
        "T" => Object::string_literal(w.name),
        "FT" => kind,
    };
    if let Some(value) = value {
        field.set("V", value);
    }
    let rect: Vec<Object> = w.rect.iter().map(|n| Object::Integer(*n)).collect();
    let mut widget = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "Rect" => rect,
        "P" => page_id,
    };
    match w.parent {
        None => {
            for (key, value) in field.iter() {
                widget.set(key.clone(), value.clone());
            }
        }
        Some(parent) => {
            // The parent holds the type and value; the widget holds only
            // its own part of the name.
            field.set("T", Object::string_literal(parent));
            let parent_id = doc.add_object(field);
            widget.set("Parent", parent_id);
            widget.set("T", Object::string_literal(w.name));
        }
    }
    doc.add_object(widget)
}
