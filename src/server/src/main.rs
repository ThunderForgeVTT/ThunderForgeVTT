mod config;
mod errors;
mod world;
mod auth;

use clap::clap_app;

use clap::ArgMatches;

#[macro_use]
extern crate rocket;

const APP_NAME: &str = "vtt-interloper";

#[tokio::main]
async fn main() {
    let mut app = clap_app!(myapp =>
        (name: APP_NAME)
        (@subcommand agent =>
            (about: "controls testing features")
            (@arg verbose: -v --verbose "Print test information verbosely")
        )
    );

    let matches: ArgMatches = app.clone().get_matches();

    match matches.subcommand() {
        ("inject", sub_args) => inject(&sub_args).await,
        ("agent", sub_args) => server(&sub_args).await,
        _ => {
            println!("\nNo command utilized! Please rerun with a subcommand.\n");
            app.print_help()
                .unwrap_or_else(|_| panic!("An Unknown error has occurred!"));
        }
    }
}

async fn server(args: &Option<&ArgMatches<'_>>) {
    let mut ip_address = "127.0.0.1";
    let mut port = "3000";
    if args.is_some() {
        let arg_matches = args.unwrap();
        ip_address = arg_matches.value_of("ip_address").unwrap_or(ip_address);
        port = arg_matches.value_of("port").unwrap_or(port)
    }

    let figment = rocket::Config::figment()
        .merge(("port", port.parse::<u16>().unwrap()))
        .merge(("address", ip_address))
        .merge(("ident", "FoundryVTT Interloper"));

    rocket::custom(figment)
        .attach(errors::stage())
        .attach(world::stage())
        .launch()
        .await
        .unwrap_or_else(|_| panic!("Something unexpected happened!"));
}

async fn inject(args: &Option<&ArgMatches<'_>>) {
    const CONNECTION: &'static str = "ws://127.0.0.1:30000/socket.io/";
}
