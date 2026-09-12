//! Why a document yields no pages.
//!     cargo run -p thunderforge-pdf --example diagnose -- <file>
fn main() {
    let path = std::env::args().nth(1).expect("usage: diagnose <file>");
    let bytes = std::fs::read(&path).expect("readable");
    println!("{} bytes", bytes.len());

    match lopdf::Document::load_mem(&bytes) {
        Ok(d) => println!(
            "loads natively: {} objects, {} pages",
            d.objects.len(),
            d.get_pages().len()
        ),
        Err(e) => println!("native load failed: {e}"),
    }

    let Some(repaired) = thunderforge_pdf::rebuild_xref(&bytes) else {
        println!("rebuild_xref: refused");
        return;
    };
    println!("rebuilt (+{} bytes)", repaired.len() - bytes.len());

    match lopdf::Document::load_mem(&repaired) {
        Err(e) => println!("repaired load failed: {e}"),
        Ok(mut d) => {
            println!(
                "repaired: {} objects, {} pages",
                d.objects.len(),
                d.get_pages().len()
            );
            let streams = d
                .objects
                .values()
                .filter(|o| {
                    matches!(o, lopdf::Object::Stream(s)
                    if matches!(s.dict.get(b"Type"), Ok(lopdf::Object::Name(n)) if n == b"ObjStm"))
                })
                .count();
            println!("object streams present: {streams}");
            let n = thunderforge_pdf::absorb_object_streams(&mut d);
            println!(
                "absorbed {n} objects -> {} objects, {} pages",
                d.objects.len(),
                d.get_pages().len()
            );
            match d.catalog() {
                Err(e) => println!("catalog: {e}"),
                Ok(c) => {
                    println!(
                        "catalog keys: {:?}",
                        c.iter()
                            .map(|(k, _)| String::from_utf8_lossy(k).to_string())
                            .collect::<Vec<_>>()
                    );
                    match c.get(b"Pages") {
                        Err(e) => println!("  /Pages missing: {e}"),
                        Ok(p) => {
                            println!("  /Pages = {p:?}");
                            match d.dereference(p) {
                                Err(e) => println!("  deref failed: {e}"),
                                Ok((_, o)) => println!(
                                    "  deref ok: {}",
                                    match o {
                                        lopdf::Object::Dictionary(dd) => format!(
                                                "dict keys {:?}",
                                                dd.iter()
                                                    .map(|(k, _)| String::from_utf8_lossy(k)
                                                        .to_string())
                                                    .collect::<Vec<_>>()
                                            ),
                                        other => format!("{other:?}").chars().take(120).collect(),
                                    }
                                ),
                            }
                        }
                    }
                }
            }
        }
    }
}
