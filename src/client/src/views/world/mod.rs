use yew::prelude::*;
use yew::services::ConsoleService;

pub struct WorldComponent {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    link: ComponentLink<Self>,
    world_id: String,
}

#[derive(Properties, Clone, PartialEq)]
pub struct WorldProps {
    pub(crate) world_id: String,
}

impl Component for WorldComponent {
    type Message = ();
    type Properties = WorldProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            world_id: props.world_id,
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        if self.world_id.ne(&props.world_id) {
            self.world_id = props.world_id;
            true
        } else {
            false
        }
    }

    fn view(&self) -> Html {
        let id = self.world_id.clone();
        ConsoleService::log("Hewwo");
        let render: Html = html! {
            <div id={"engine"} data-world-id=id></div>
        };
        render
    }
}
