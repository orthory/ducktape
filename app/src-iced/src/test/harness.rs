//! The screen-test toolkit: render a view headless, address widgets by the
//! agent's semantic layer, and collect the messages interactions emit.
//!
//! ```ignore
//! let mut ui = sim(settings::view(&state));           // render()
//! ui.click(by::role(Role::Button, "Open Account"))?;  // fireEvent
//! assert!(emitted(ui, &Message::OpenAccount));        // callback fired
//! ```

pub use iced_agent_plugin::Role;
pub use iced_agent_plugin::selector::by;
pub use iced_test::simulator as sim;

/// Did the interaction emit this exact message?
pub fn emitted<Message: PartialEq>(
    ui: iced_test::Simulator<'_, Message>,
    expected: &Message,
) -> bool {
    ui.into_messages().any(|message| message == *expected)
}

/// Does the rendered view contain a node of this role and name?
pub fn has<Message>(
    ui: &mut iced_test::Simulator<'_, Message>,
    role: Role,
    name: &str,
) -> bool {
    ui.find(by::role(role, name)).is_ok()
}
