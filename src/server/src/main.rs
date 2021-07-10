mod auth;
mod config;
mod errors;
mod serve;
mod users;
mod utils;
mod world;

use clap::clap_app;

use crate::config::{Config, Directories};
use clap::ArgMatches;
use rocket::config::{Ident, SecretKey};
use rocket::{Build, Rocket};

#[macro_use]
extern crate rocket;

const APP_NAME: &str = "thunderforge";

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let mut app = clap_app!(myapp =>
        (name: APP_NAME)
        (@subcommand start =>
            (about: "controls testing features")
            (@arg verbose: -v --verbose "Print test information verbosely")
            (@arg data_path: -d --data-path "Where do you want ThunderForgeVTT to store data?")
            (@arg redis_url: -r --redis-url "What redis url would you like to connect to?")
        )
    );

    let matches: ArgMatches = app.clone().get_matches();

    match matches.subcommand() {
        ("start", sub_args) => server(&sub_args).await,
        _ => {
            println!("\nNo command utilized! Please rerun with a subcommand.\n");
            app.print_help()
                .unwrap_or_else(|_| panic!("An Unknown error has occurred!"));
        }
    }
}

fn load_config() -> Config {
    let dir = std::env::current_dir().unwrap();
    let current_dir = dir.as_path();
    let config = &current_dir.join("config.json");
    if config.exists() {
        let data = std::fs::read_to_string(&config).unwrap();
        serde_json::from_str(&data).unwrap()
    }
    Config::default()
}

pub struct RocketSetup {
    port: u16,
    address: String,
    ident: String,
    config: Config,
    directories: Directories,
    redis_url: String,
}

pub async fn rocket_ship(setup: RocketSetup) -> Rocket<Build> {
    let mut rocket_config =
        rocket::config::Config::try_from(rocket::config::Config::default()).unwrap();
    rocket_config.port = setup.port;
    rocket_config.address = setup.address.parse().unwrap();
    rocket_config.ident = Ident::try_new(&setup.ident).unwrap();
    rocket_config.secret_key = SecretKey::from(setup.config.secret.as_bytes());

    let database = unqlite::UnQLite::create_temp();

    rocket::custom(rocket_config)
        .attach(errors::stage())
        .attach(auth::stage())
        .attach(world::stage())
        .attach(serve::stage())
        .manage(setup.config)
        .manage(setup.directories)
        .manage(database)
}

async fn server(args: &Option<&ArgMatches<'_>>) {
    let mut ip_address = "127.0.0.1";
    let mut port = "30000";
    let mut config = load_config();
    let mut redis_url = String::from("redis://127.0.0.1/");

    if args.is_some() {
        let arg_matches = args.unwrap();
        ip_address = arg_matches.value_of("ip_address").unwrap_or(ip_address);
        port = arg_matches.value_of("port").unwrap_or(port);
        if arg_matches.is_present("data_path") {
            config.data_path = String::from(
                arg_matches
                    .value_of("data_path")
                    .unwrap_or(&config.data_path),
            );
        }
        redis_url = String::from(arg_matches.value_of("redis_url").unwrap_or(&redis_url));
    }

    let directories = Directories::from(String::from(&config.data_path));
    directories.create_if_not_present();
    let rocket_setup = RocketSetup {
        port: port.parse::<u16>().unwrap(),
        address: ip_address.parse().unwrap(),
        ident: String::from("ThunderForgeVTT"),
        config,
        directories,
        redis_url,
    };
    let little_rocket_ship = rocket_ship(rocket_setup).await;
    match little_rocket_ship.launch().await {
        Ok(_) => println!("Start Successful!"),
        Err(e) => panic!("Unexpected Error has Occurred: \n{}\n", e.to_string()),
    }
}
