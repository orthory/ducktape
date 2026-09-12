//! Per-instance request broker. Futures and subscriptions own their request;
//! dropping either cancels the host operation even outside a driver tick.

use futures::{Stream, StreamExt, channel::mpsc};
use std::{cell::RefCell, collections::BTreeMap, pin::Pin, rc::Rc, task::{Context, Poll}};
use crate::wire;

pub type Answer = Result<Vec<u8>, String>;

#[derive(Default)]
struct Broker {
    next_id: u64,
    pending: BTreeMap<u64, mpsc::Sender<Answer>>,
    requests: Vec<wire::Request>,
    cancels: Vec<u64>,
}

#[derive(Clone, Default)]
pub(crate) struct Host(Rc<RefCell<Broker>>);

thread_local! {
    static CURRENT: RefCell<Option<Host>> = const { RefCell::new(None) };
}

pub(crate) struct Scope(Option<Host>);
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with_borrow_mut(|current| *current = self.0.take());
    }
}

impl Host {
    pub(crate) fn enter(&self) -> Scope {
        Scope(CURRENT.with_borrow_mut(|current| current.replace(self.clone())))
    }

    pub(crate) fn deliver(&self, id: u64, result: Answer, done: bool) -> Result<(), String> {
        let mut broker = self.0.borrow_mut();
        let size = match &result { Ok(bytes) => bytes.len(), Err(error) => error.len() };
        if size > wire::MAX_FRAME_BYTES {
            return Err("host response exceeds byte budget".into());
        }
        if let Some(sender) = broker.pending.get_mut(&id) {
            sender.try_send(result).map_err(|_| "host response queue exceeded")?;
        }
        if done { broker.pending.remove(&id); }
        Ok(())
    }

    pub(crate) fn drain(&self) -> (Vec<wire::Request>, Vec<u64>) {
        let mut broker = self.0.borrow_mut();
        (std::mem::take(&mut broker.requests), std::mem::take(&mut broker.cancels))
    }
}

pub struct Subscription {
    host: Host,
    id: u64,
    receive: mpsc::Receiver<Answer>,
}

impl Stream for Subscription {
    type Item = Answer;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Answer>> {
        self.receive.poll_next_unpin(cx)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut broker = self.host.0.borrow_mut();
        if broker.pending.remove(&self.id).is_some() {
            broker.cancels.push(self.id);
        }
    }
}

pub fn subscribe(kind: &str, payload: &[u8]) -> Subscription {
    let host = CURRENT.with_borrow(|current| current.clone()).expect("host request outside view tick");
    let (sender, receive) = mpsc::channel(32);
    let id = {
        let mut broker = host.0.borrow_mut();
        assert!(broker.pending.len() < wire::MAX_REQUESTS, "too many active view requests");
        assert!(broker.requests.len() < wire::MAX_REQUESTS, "too many requests this tick");
        let id = broker.next_id;
        broker.next_id = id.checked_add(1).expect("view request ids exhausted");
        let request = wire::Request { id, kind: kind.into(), payload: payload.to_vec() };
        request.validate().expect("invalid guest request");
        broker.pending.insert(id, sender);
        broker.requests.push(request);
        id
    };
    Subscription { host, id, receive }
}

pub async fn request(kind: &str, payload: &[u8]) -> Answer {
    subscribe(kind, payload).next().await.unwrap_or_else(|| Err("host closed request".into()))
}
