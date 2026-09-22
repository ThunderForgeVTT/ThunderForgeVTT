//! What fonts a page declares, and what this crate can do with them.
//!     cargo run -p thunderforge-pdf --example fonts -- <file> <page>

#![allow(clippy::print_stdout)] // a command-line tool: its output is the point

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: fonts <file> <page>");
    let want: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);

    let bytes = std::fs::read(&path).expect("readable");
    let mut doc = match lopdf::Document::load_mem(&bytes) {
        Ok(d) => d,
        Err(_) => {
            let r = thunderforge_pdf::rebuild_xref(&bytes).expect("rebuildable");
            lopdf::Document::load_mem(&r).expect("loads")
        }
    };
    thunderforge_pdf::absorb_object_streams(&mut doc);

    let Some((_, id)) = doc.get_pages().into_iter().find(|(n, _)| *n == want) else {
        println!("no page {want}");
        return;
    };
    let Ok((Some(res), _)) = doc.get_page_resources(id) else {
        println!("no resources");
        return;
    };
    let Ok(fonts) = res
        .get(b"Font")
        .and_then(|f| doc.dereference(f))
        .map(|(_, o)| o)
    else {
        println!("no /Font");
        return;
    };
    let Ok(fonts) = fonts.as_dict() else { return };

    for (name, value) in fonts.iter() {
        let Ok((_, obj)) = doc.dereference(value) else {
            continue;
        };
        let Ok(font) = obj.as_dict() else { continue };
        let keys: Vec<String> = font
            .iter()
            .map(|(k, _)| String::from_utf8_lossy(k).to_string())
            .collect();
        let subtype = font
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("?")
            .to_string();
        let base = font
            .get(b"BaseFont")
            .ok()
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("?")
            .to_string();
        let enc = match font.get(b"Encoding") {
            Ok(lopdf::Object::Name(n)) => format!("name {}", String::from_utf8_lossy(n)),
            Ok(lopdf::Object::Reference(_)) | Ok(lopdf::Object::Dictionary(_)) => {
                let d = font
                    .get(b"Encoding")
                    .ok()
                    .and_then(|o| doc.dereference(o).ok())
                    .map(|(_, o)| o);
                match d.and_then(|o| o.as_dict().ok()) {
                    Some(dd) => format!(
                        "dict[{}]",
                        dd.iter()
                            .map(|(k, _)| String::from_utf8_lossy(k).to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    None => "dict?".into(),
                }
            }
            _ => "none".into(),
        };
        let decoded = font
            .get_font_encoding(&doc)
            .map(|e| match e {
                lopdf::Encoding::OneByteEncoding(_) => "table",
                lopdf::Encoding::UnicodeMapEncoding(_) => "cmap",
                lopdf::Encoding::SimpleEncoding(_) => "simple(unusable)",
            })
            .unwrap_or("error");
        println!(
            "{:8.8} {:14.14} {:34.34} enc={:28.28} -> {decoded:16} keys={:?}",
            String::from_utf8_lossy(name),
            subtype,
            base,
            enc,
            keys
        );
    }
}
