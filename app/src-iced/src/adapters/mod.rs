//! Trusted host adapters.
//!
//! Only this side of the view boundary receives backend, node transport, OS,
//! filesystem, or browser capabilities.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::backend::{Backend, Workspace};
use crate::module_host::{ChatEffect, Effect, Event};
use crate::screen_service;
use crate::screens::user;
use crate::transport::NodeClient;
use crate::view_api::DropToken;

#[derive(Default)]
pub(crate) struct DropRegistry {
    paths: HashMap<DropToken, PathBuf>,
}

impl DropRegistry {
    pub(crate) fn mint(&mut self, path: PathBuf) -> Result<DropToken, String> {
        loop {
            let mut bytes = [0; 16];
            getrandom::getrandom(&mut bytes)
                .map_err(|_| "could not mint a native drop token".to_string())?;
            let token = DropToken::from_host(bytes);
            if let std::collections::hash_map::Entry::Vacant(entry) = self.paths.entry(token) {
                entry.insert(path);
                return Ok(token);
            }
        }
    }

    pub(crate) fn consume(&mut self, token: DropToken) -> Result<PathBuf, String> {
        self.paths
            .remove(&token)
            .ok_or_else(|| "native drop token is invalid or was already used".to_string())
    }

    pub(crate) fn discard(&mut self, token: DropToken) {
        self.paths.remove(&token);
    }
}

pub(crate) async fn execute_user(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    node: Option<NodeClient>,
    command: user::Command,
) -> user::ServiceEvent {
    screen_service::execute(backend, workspace, node, command).await
}

pub(crate) async fn execute_module(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    node: Option<NodeClient>,
    effect: Effect,
) -> Event {
    match effect {
        Effect::Home(effect) => {
            Event::home(execute_user(backend, workspace, node, effect.into_command()).await)
        }
        Effect::Chat(ChatEffect::Command(effect)) => {
            Event::chat(execute_user(backend, workspace, node, effect.into_command()).await)
        }
        Effect::Chat(ChatEffect::Intent(_)) => Event::chat(user::ServiceEvent::ActionFinished {
            screen: user::Screen::Chat,
            result: Err("application intent was not handled by the desktop shell".into()),
        }),
        Effect::Pages(effect) => {
            Event::pages(execute_user(backend, workspace, node, effect.into_command()).await)
        }
        Effect::Files(effect) => {
            Event::files(execute_user(backend, workspace, node, effect.into_command()).await)
        }
    }
}

pub(crate) async fn execute_drop(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    target: String,
    source: Result<PathBuf, String>,
) -> Event {
    let event = match source {
        Ok(source) => screen_service::execute_drop(backend, node, target, source).await,
        Err(error) => user::ServiceEvent::ActionFinished {
            screen: user::Screen::Files,
            result: Err(error),
        },
    };
    Event::files(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_tokens_are_registry_bound_and_one_shot() {
        let path = PathBuf::from("/private/drop.txt");
        let mut owner = DropRegistry::default();
        let mut stranger = DropRegistry::default();
        let token = owner.mint(path.clone()).unwrap();

        assert!(stranger.consume(token).is_err());
        assert_eq!(owner.consume(token).unwrap(), path);
        assert!(owner.consume(token).is_err());
    }
}
