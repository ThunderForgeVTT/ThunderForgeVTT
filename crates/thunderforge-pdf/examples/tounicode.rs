//! Dump a page's ToUnicode CMaps, to see why a parser rejects them.
//!     cargo run -p thunderforge-pdf --example tounicode -- <file> <page>
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage");
    let want: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let bytes = std::fs::read(&path).expect("readable");
    let mut doc = match lopdf::Document::load_mem(&bytes) {
        Ok(d) => d,
        Err(_) => {
            lopdf::Document::load_mem(&thunderforge_pdf::rebuild_xref(&bytes).expect("rebuildable"))
                .expect("loads")
        }
    };
    thunderforge_pdf::absorb_object_streams(&mut doc);
    let Some((_, id)) = doc.get_pages().into_iter().find(|(n, _)| *n == want) else {
        return;
    };
    let Ok((Some(res), _)) = doc.get_page_resources(id) else {
        return;
    };
    let Ok(fonts) = res
        .get(b"Font")
        .and_then(|f| doc.dereference(f))
        .map(|(_, o)| o)
    else {
        return;
    };
    let Ok(fonts) = fonts.as_dict() else { return };
    for (name, value) in fonts.iter() {
        let Ok((_, obj)) = doc.dereference(value) else {
            continue;
        };
        let Ok(font) = obj.as_dict() else { continue };
        let Ok(tu) = font
            .get_deref(b"ToUnicode", &doc)
            .and_then(lopdf::Object::as_stream)
        else {
            continue;
        };
        let Ok(content) = tu.get_plain_content() else {
            continue;
        };
        println!(
            "=== {} ({} bytes) ===",
            String::from_utf8_lossy(name),
            content.len()
        );
        let text = String::from_utf8_lossy(&content);
        for l in text.lines().take(22) {
            println!("  {l}");
        }
    }
}
