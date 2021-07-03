use yew::prelude::*;
use yew::services::ConsoleService;

pub enum LoginMethod {
    Basic,
}

pub enum Msg {
    SetUserName(String),
    SetPassWord(String),
    Click(LoginMethod),
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
        let login_basic = self.link.callback(|_action: MouseEvent| Msg::Click(LoginMethod::Basic));
        html! {
            <form action="POST">
                <fieldset>
                    <label for="username-field">{"Username:"}</label>
                    <input id="username-field" oninput=change_username/>
                </fieldset>
                <fieldset>
                    <label for="password-field">{"Password:"}</label>
                    <input type="password" id="password-field" oninput=change_password/>
                </fieldset>
                <button onclick=login_basic>{"Login"}</button>
            </form>
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
                ConsoleService::info(&value);
                self.username = String::from(&value);
                true
            }
            Msg::SetPassWord(value) => {
                ConsoleService::info(&value);
                self.password = String::from(&value);
                true
            }
            Msg::Click(action) => {
                use LoginMethod::Basic;
                match action {
                    Basic => {
                        ConsoleService::debug("Logging in via basic auth.");
                        true
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
        html! {
            <div>
            {self.render_basic_login()}
            </div>
        }
    }
}
