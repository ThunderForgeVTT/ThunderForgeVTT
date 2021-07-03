mod counter;
mod views;

use yew::prelude::*;
use yew_router::prelude::*;

use counter::CounterComponent;
use wasm_bindgen::prelude::*;

#[derive(Switch, Debug, Clone)]
pub enum AppRoute {
    #[to = "/counter"]
    Counter,
    #[to = "/login"]
    Login,
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
        use views::login::LoginComponent;
        html! {
            <Router<AppRoute, ()>
                render = Router::render(|switch: AppRoute| {
                    match switch {
                        AppRoute::Counter => html!{<CounterComponent/>},
                        AppRoute::Login => html!{<LoginComponent/>},
                    }
                })
            />
        }
    }
}

#[wasm_bindgen]
pub fn main() {
    yew::start_app::<Main>();
}
