use crate::config::Directories;
use rocket::fairing::AdHoc;
use rocket::fs::NamedFile;
use rocket::response::Redirect;
use rocket::State;
use std::path::{Path, PathBuf};

#[get("/")]
async fn serve_index(directories: &State<Directories>) -> Option<NamedFile> {
    let static_files = &directories.static_files;
    NamedFile::open(Path::new(&static_files).join("index.html"))
        .await
        .ok()
}

#[get("/<file..>")]
async fn serve_static(file: PathBuf, directories: &State<Directories>) -> Option<NamedFile> {
    let static_files = &directories.static_files;
    let path = Path::new(&static_files).join(file);
    if path.exists() {
        NamedFile::open(path).await.ok()
    } else {
        serve_index(directories).await
    }
}

#[get("/assets/<file>")]
async fn get_asset_file(file: PathBuf, directories: &State<Directories>) -> Option<NamedFile> {
    NamedFile::open(Path::new(&directories.asset_directory).join(file))
        .await
        .ok()
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Serve Assets", |rocket| async {
        rocket
            // .register("/", catchers![spa_catch])
            .mount("/", routes![serve_index, serve_static, get_asset_file])
    })
}
