use crate::config::Directories;
use rocket::fairing::AdHoc;
use rocket::fs::NamedFile;
use rocket::State;
use std::path::{Path, PathBuf};

#[get("/")]
async fn serve_index(directories: &State<Directories>) -> Option<NamedFile> {
    get_static_file(PathBuf::from("index.html"), directories).await
}

#[get("/<file>")]
async fn get_static_file(file: PathBuf, directories: &State<Directories>) -> Option<NamedFile> {
    NamedFile::open(Path::new(&directories.static_files).join(file))
        .await
        .ok()
}

#[get("/assets/<file>")]
async fn get_asset_file(file: PathBuf, directories: &State<Directories>) -> Option<NamedFile> {
    NamedFile::open(Path::new(&directories.asset_directory).join(file))
        .await
        .ok()
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Serve Assets", |rocket| async {
        rocket.mount("/", routes![serve_index, get_static_file, get_asset_file])
    })
}
