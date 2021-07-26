use thunderforge_core::auth::Credentials;
use yew::prelude::*;
use yew::services::ConsoleService;

pub enum SignUpMethod {
    Basic,
}

pub enum SignUpStatus {
    Success,
    Failure,
}

pub enum Msg {
    SetUserName(String),
    SetPassWord(String),
    SetPassWordConfirmation(String),
    Click(SignUpMethod),
    LoginResponse(SignUpStatus, String),
}

pub struct SignUpComponent {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    link: ComponentLink<Self>,
    username: String,
    password: String,
    password_confirmation: String,
}

impl SignUpComponent {
    fn render_basic_login(&self) -> Html {
        let change_username = self
            .link
            .callback(|change: InputData| Msg::SetUserName(change.value));
        let change_password = self
            .link
            .callback(|change: InputData| Msg::SetPassWord(change.value));
        let change_password_confirmation = self
            .link
            .callback(|change: InputData| Msg::SetPassWordConfirmation(change.value));
        let signup_basic = self
            .link
            .callback(|_action: MouseEvent| Msg::Click(SignUpMethod::Basic));
        html! {
            <div action="POST" class="card">
                <div>
                    <label for="username">{"Username:"}</label>
                    <input id="username" oninput=change_username/>
                </div>
                <div>
                    <label for="password">{"Password:"}</label>
                    <input type="password" id="password" oninput=change_password/>
                </div>
            <div>
                    <label for="password-confirmation">{"Password Confirmation:"}</label>
                    <input type="password" id="password-confirmation" oninput=change_password_confirmation/>
                </div>
                <button class="btn" onclick=signup_basic>{"Sign Up"}</button>
            </div>
        }
    }
}

impl Component for SignUpComponent {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            username: String::from(""),
            password: String::from(""),
            password_confirmation: String::from(""),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SetUserName(value) => {
                ConsoleService::info(&value);
                self.username = String::from(&value);
                true
            }
            Msg::SetPassWord(value) => {
                self.password = String::from(&value);
                true
            }
            Msg::SetPassWordConfirmation(value) => {
                self.password_confirmation = String::from(&value);
                true
            }
            Msg::Click(action) => {
                use SignUpMethod::Basic;
                match action {
                    Basic => {
                        let u_name = String::from(&self.username);
                        let p_word = String::from(&self.password);
                        let pc_word = String::from(&self.password_confirmation);
                        ConsoleService::debug(
                            "[Authentication][Basic]: Logging in with basic auth",
                        );
                        if !p_word.eq(&pc_word) {
                            true
                        } else {
                            wasm_bindgen_futures::spawn_local(async {
                                Credentials::new(Option::None, u_name, p_word).login().await;
                            });
                            false
                        }
                    }
                }
            }
            Msg::LoginResponse(response, message) => {
                use SignUpStatus::{Failure, Success};
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
