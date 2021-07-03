use rocket::fairing::AdHoc;

#[get("/all")]
fn all_worlds() {}

#[get("/<id>")]
fn world_by_id(id: String) {}

#[post("/<id>")]
fn switch_world(id: String) {}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Worlds", |rocket| async {
        rocket.mount("/world", routes![all_worlds, world_by_id, switch_world])
    })
}
