//! Look closely at a handful of documents: pages, and where text starts.
//!
//!     cargo run -p thunderforge-pdf --example probe -- <file>...
//!     cargo run -p thunderforge-pdf --example probe -- --structure <file>...
//!
//! `--structure` answers where a document draws its text (spec 048 T010):
//! in the page's own content, inside Form XObjects the page draws with `Do`,
//! or in form-field appearance streams the page content never names. It
//! prints counts only, never text, so it is safe to run over personal files.

#![allow(clippy::print_stdout)] // a command-line tool: its output is the point

use thunderforge_pdf::{Document, layout};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--structure") {
        for path in &args[1..] {
            structure(path);
        }
        return;
    }
    for path in args {
        match Document::open(&path) {
            Err(error) => println!("{path}: {error}"),
            Ok(document) => {
                let pages = document.pages();
                let mut first_text: Option<(u32, usize)> = None;
                let mut total = 0usize;
                for page in pages.iter().take(60) {
                    let Ok(runs) = document.runs(page) else {
                        continue;
                    };
                    let chars: usize = layout::lines(&runs)
                        .iter()
                        .map(|l| l.text.chars().count())
                        .sum();
                    total += chars;
                    if chars > 100 && first_text.is_none() {
                        first_text = Some((page.number, chars));
                    }
                }
                let name = path.rsplit('/').next().unwrap_or(&path);
                println!(
                    "{:52.52} pages {:4}  first text: {:>12}  chars in 60pp {}",
                    name,
                    pages.len(),
                    first_text
                        .map(|(p, _)| format!("page {p}"))
                        .unwrap_or_else(|| "none".into()),
                    total
                );
            }
        }
    }
}

/// Whether a content stream shows text, and how many show-text operators.
fn shows_text(content: &[u8]) -> usize {
    lopdf::content::Content::decode(content)
        .map(|c| {
            c.operations
                .iter()
                .filter(|op| matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\""))
                .count()
        })
        .unwrap_or(0)
}

fn structure(path: &str) {
    use lopdf::Object;
    let name = path.rsplit('/').next().unwrap_or(path);
    let Ok(bytes) = std::fs::read(path) else {
        println!("{name}: unreadable");
        return;
    };
    let Ok(doc) = lopdf::Document::load_mem(&bytes) else {
        println!("{name}: lopdf could not open it");
        return;
    };
    let mut page_text_ops = 0usize;
    let mut do_forms = 0usize;
    let mut do_forms_with_text = 0usize;
    let mut form_text_ops = 0usize;
    let mut widgets = 0usize;
    let mut widgets_with_text = 0usize;
    let mut widgets_with_value = 0usize;
    let mut widgets_no_appearance = 0usize;
    let mut widgets_state_appearance = 0usize;
    for (_, page_id) in doc.get_pages() {
        let content = doc.get_page_content(page_id).unwrap_or_default();
        page_text_ops += shows_text(&content);
        // Form XObjects the page draws by name.
        let names: Vec<Vec<u8>> = lopdf::content::Content::decode(&content)
            .map(|c| {
                c.operations
                    .iter()
                    .filter(|op| op.operator == "Do")
                    .filter_map(|op| op.operands.first()?.as_name().ok().map(<[u8]>::to_vec))
                    .collect()
            })
            .unwrap_or_default();
        // The page's resources may be inline or referenced, and may be
        // inherited; take the first that names an XObject table.
        let (inline, referenced) = doc
            .get_page_resources(page_id)
            .unwrap_or((None, Vec::new()));
        let mut tables: Vec<&lopdf::Dictionary> = inline.into_iter().collect();
        tables.extend(
            referenced
                .iter()
                .filter_map(|id| doc.get_dictionary(*id).ok()),
        );
        let xobjects = tables.iter().find_map(|r| match r.get(b"XObject").ok()? {
            Object::Reference(id) => doc.get_dictionary(*id).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        });
        for name in names {
            let Some(id) = xobjects
                .and_then(|x| x.get(&name).ok())
                .and_then(|o| o.as_reference().ok())
            else {
                continue;
            };
            let Ok(Object::Stream(stream)) = doc.get_object(id) else {
                continue;
            };
            if stream.dict.get(b"Subtype").and_then(|s| s.as_name()).ok() != Some(b"Form") {
                continue;
            }
            do_forms += 1;
            let ops = shows_text(&stream.decompressed_content().unwrap_or_default());
            if ops > 0 {
                do_forms_with_text += 1;
                form_text_ops += ops;
            }
        }
        // Widget annotations and their normal appearance.
        let annots = doc
            .get_dictionary(page_id)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|a| match a {
                Object::Reference(id) => doc.get_object(*id).ok(),
                other => Some(other),
            })
            .and_then(|a| a.as_array().ok())
            .cloned()
            .unwrap_or_default();
        for annot in annots {
            let Some(dict) = annot
                .as_reference()
                .ok()
                .and_then(|id| doc.get_dictionary(id).ok())
            else {
                continue;
            };
            if dict.get(b"Subtype").and_then(|s| s.as_name()).ok() != Some(b"Widget") {
                continue;
            }
            widgets += 1;
            // The field's value: on the widget, or on its parent field.
            let value = dict.get(b"V").ok().or_else(|| {
                dict.get(b"Parent")
                    .ok()
                    .and_then(|p| p.as_reference().ok())
                    .and_then(|id| doc.get_dictionary(id).ok())
                    .and_then(|p| p.get(b"V").ok())
            });
            if let Some(Object::String(bytes, _)) = value
                && !bytes.is_empty()
            {
                widgets_with_value += 1;
            }
            let ap = dict.get(b"AP").ok().and_then(|ap| match ap {
                Object::Reference(id) => doc.get_dictionary(*id).ok(),
                Object::Dictionary(d) => Some(d),
                _ => None,
            });
            match ap.and_then(|ap| ap.get(b"N").ok()) {
                None => widgets_no_appearance += 1,
                Some(Object::Dictionary(_)) => widgets_state_appearance += 1,
                _ => {}
            }
            let normal = dict
                .get(b"AP")
                .ok()
                .and_then(|ap| match ap {
                    Object::Reference(id) => doc.get_dictionary(*id).ok(),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                })
                .and_then(|ap| ap.get(b"N").ok())
                .and_then(|n| n.as_reference().ok())
                .and_then(|id| doc.get_object(id).ok());
            if let Some(Object::Stream(stream)) = normal
                && shows_text(&stream.decompressed_content().unwrap_or_default()) > 0
            {
                widgets_with_text += 1;
            }
        }
    }
    let fields = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| match a {
            Object::Reference(id) => doc.get_dictionary(*id).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        })
        .and_then(|a| a.get(b"Fields").ok())
        .and_then(|f| match f {
            Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|f| f.as_array().ok())
        .map(|f| f.len())
        .unwrap_or(0);
    // What the crate itself reads, to check it against the raw count.
    let (read_fields, read_values) = thunderforge_pdf::Document::from_bytes(&bytes)
        .map(|d| {
            let fields: Vec<_> = d.pages().iter().flat_map(|p| d.form_fields(p)).collect();
            let values = fields.iter().filter(|f| !f.value.trim().is_empty()).count();
            (fields.len(), values)
        })
        .unwrap_or((0, 0));
    println!("{name:28.28} form_fields() read {read_fields} fields, {read_values} with a value");
    println!(
        "{name:28.28} pages {:2}  page text ops {page_text_ops:5}  forms drawn {do_forms:3} (with text {do_forms_with_text:3}, ops {form_text_ops:5})  acroform fields {fields:4}  widgets {widgets:4} (value {widgets_with_value:4}, appearance with text {widgets_with_text:4}, none {widgets_no_appearance:4}, by state {widgets_state_appearance:4})",
        doc.get_pages().len()
    );
}
