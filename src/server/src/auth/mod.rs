use rocket::fairing::AdHoc;
use base64::{encode, decode};
use std::str::from_utf8;

#[derive(Debug, PartialEq, FromForm)]
struct Credentials {
    username: String,
    password: String
}

impl From<String> for Credentials {
    fn from(cred: String) -> Self {
        let cred_bytes = &decode(&cred).unwrap()[..];
        let cred_string: String = from_utf8(&cred_bytes).unwrap().to_string();
        let cred_parts: Vec<&str> = cred_string.split("~UwU~").collect();
        Credentials {
            username: cred_parts[0].to_string(),
            password: cred_parts[1].to_string()
        }
    }
}

#[post("/basic?<credentials>")]
fn basic_authentication(credentials: String) {
    let cred = Credentials::from(credentials);

}


pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Authentication", |rocket| async {
        rocket.mount("/authentication", routes![])
    })
}
