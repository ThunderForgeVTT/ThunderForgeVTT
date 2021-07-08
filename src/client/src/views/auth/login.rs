use thunderforge_core::auth::Credentials;
use yew::prelude::*;
use yew::services::ConsoleService;

enum LoginMethod {
    Basic,
}

enum LoginStatus {
    Success,
    Failure,
}

pub enum Msg {
    SetUserName(String),
    SetPassWord(String),
    Click(LoginMethod),
    LoginResponse(LoginStatus, String),
}

pub struct LoginComponent {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    link: ComponentLink<Self>,
    username: String,
    password: String,
}

impl LoginComponent {
    fn render_basic_login(&self) -> Html {
        let change_username = self
            .link
            .callback(|change: InputData| Msg::SetUserName(change.value));
        let change_password = self
            .link
            .callback(|change: InputData| Msg::SetPassWord(change.value));
        let login_basic = self
            .link
            .callback(|_action: MouseEvent| Msg::Click(LoginMethod::Basic));
        html! {
            <div action="POST">
                <div>
                    <label for="username">{"Username:"}</label>
                    <input id="username" oninput=change_username/>
                </div>
                <div>
                    <label for="password">{"Password:"}</label>
                    <input type="password" id="password" oninput=change_password/>
                </div>
                <button onclick=login_basic>{"Login"}</button>
            </div>
        }
    }
}

impl Component for LoginComponent {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            username: String::from(""),
            password: String::from(""),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SetUserName(value) => {
                self.username = String::from(&value);
                true
            }
            Msg::SetPassWord(value) => {
                self.password = String::from(&value);
                true
            }
            Msg::Click(action) => {
                use LoginMethod::Basic;
                match action {
                    Basic => {
                        let u_name = String::from(&self.username);
                        let p_word = String::from(&self.username);
                        ConsoleService::debug(
                            "[Authentication][Basic]: Logging in with basic auth",
                        );
                        wasm_bindgen_futures::spawn_local(async {
                            Credentials::new(Option::None, u_name, p_word).login().await;
                        });
                        false
                    }
                }
            }
            Msg::LoginResponse(response, message) => {
                use LoginStatus::{Failure, Success};
                match response {
                    Success => {
                        ConsoleService::log("Successfully logged in!");
                        true
                    }
                    Failure => {
                        ConsoleService::error(&message);
                        false
                    }
                }
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn view(&self) -> Html {
        let render: Html = html! {
            <div>
            {self.render_basic_login()}
            </div>
        };
        render
    }
}
