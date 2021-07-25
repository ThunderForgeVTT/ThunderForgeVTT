use crate::utils::url_base;
use rocket::{
    fairing::AdHoc,
    form::Form,
    response::stream::{Event, EventStream},
    tokio::{
        select,
        sync::broadcast::{channel, error::RecvError, Sender},
    },
    Shutdown, State,
};
use thunderforge_core::events::WorldEvent;

#[get("/all")]
fn all_worlds() {}

// #[get("/<id>")]
// fn world_by_id(_id: String) {}
//
// #[post("/<id>")]
// fn switch_world(_id: String) {}

#[get("/<id>/events")]
async fn world_events_by_id(
    id: String,
    queue: &State<Sender<WorldEvent>>,
    mut end: Shutdown,
) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };

            yield Event::json(&msg);
        }
    }
}

/// Receive a message from a form submission and broadcast it to any receivers.
#[post("/<id>/event", data = "<form>")]
fn world_event_by_id(form: Form<WorldEvent>, id: String, queue: &State<Sender<WorldEvent>>) {
    let event = WorldEvent::new(id);

    // A send 'fails' if there are no active subscribers. That's okay.
    let updated_form = Form::from(event);
    let _res = queue.send(updated_form.into_inner());
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Manage Worlds", |rocket| async {
        rocket.manage(channel::<WorldEvent>(1024).0).mount(
            url_base("world"),
            routes![
                all_worlds,
                // world_by_id,
                // switch_world,
                world_events_by_id,
                world_event_by_id
            ],
        )
    })
}
