mod counter;
mod views;

use yew::prelude::*;
use yew_router::prelude::*;

use counter::CounterComponent;
use wasm_bindgen::prelude::*;
use yew::services::ConsoleService;

#[derive(Switch, Debug, Clone)]
pub enum AppRoute {
    #[to = "/world/{id}"]
    World(String),
    #[to = "/counter"]
    Counter,
    #[to = "/login"]
    Login,
    #[to = "/signup"]
    SignUp,
}

struct Main {}

impl Component for Main {
    type Message = ();
    type Properties = ();

    fn create(_props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, _props: Self::Properties) -> bool {
        false
    }

    fn view(&self) -> Html {
        use views::auth::{login::LoginComponent, signup::SignUpComponent};
        use views::world::WorldComponent;

        let routes = Router::render(|switch: AppRoute| match switch {
            AppRoute::Counter => html! {<CounterComponent/>},
            AppRoute::Login => html! {<LoginComponent/>},
            AppRoute::SignUp => html! {<SignUpComponent/>},
            AppRoute::World(world_id) => html! {<WorldComponent world_id=world_id />},
        });

        html! {
            <Router<AppRoute, ()>
                render = routes
            />
        }
    }
}

#[wasm_bindgen]
pub fn main() {
    ConsoleService::debug("Loading Client...");
    yew::start_app::<Main>();
}
