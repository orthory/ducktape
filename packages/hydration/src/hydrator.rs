use crate::config::Config;
use crate::journal::{InsertResult, Journal, Batch};
use tokio::time;

enum ControlSequence {
    InsertOp(operation::Operation),
    Drain,
    Register(OnHydrate),
    Destroy,
}

#[derive(thiserror::Error, Debug)]
pub enum HydratorError {
    #[error("hydrator is no longer running")]
    Closed
}

pub type OnHydrate = Box<dyn Fn(&Batch) + Send + Sync>;

#[derive(Clone)]
pub struct Hydrator {
    tx: tokio::sync::mpsc::UnboundedSender<ControlSequence>,
}

/// main controller
impl Hydrator {
    pub fn new_with_config(config: Config) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let inner = HydratorInner {
            journal: Journal::new(&config.journal),
            config: config,
            tx: tx.clone(),
            rx: rx,
            on_hydrate_callbacks: Vec::new(),
        };

        inner.start_receiver();

        Self {
            tx,
        }
    }

    pub fn register_on_hydrate(&self, on_hydrate: OnHydrate) -> Result<(), HydratorError> {
        self.tx
            .send(ControlSequence::Register(on_hydrate))
            .map_err(|_| HydratorError::Closed)
    }

    pub fn insert_op(&self, op: operation::Operation) -> Result<(), HydratorError> {
        self.tx
            .send(ControlSequence::InsertOp(op))
            .map_err(|_| HydratorError::Closed)
    }

    pub fn destroy(&self) -> Result<(), HydratorError> {
        self.tx
            .send(ControlSequence::Destroy)
            .map_err(|_| HydratorError::Closed)
    }
}

struct HydratorInner {
    config: Config,
    journal: Journal,
    on_hydrate_callbacks: Vec<OnHydrate>,
    tx: tokio::sync::mpsc::UnboundedSender<ControlSequence>,
    rx: tokio::sync::mpsc::UnboundedReceiver<ControlSequence>,
}

impl HydratorInner {
    fn flush(&self, batch: Batch) {
        for f in self.on_hydrate_callbacks.iter() {
            f(&batch)
        }
    }
    
    fn start_receiver(mut self) {
        // start interval; force drain at every inerval
        let cadence = self.config.cadence.interval;
        let tx_interval = self.tx.clone();
        let mut interval = time::interval(time::Duration::from_millis(cadence));
        tokio::spawn(async move {
           loop {
               interval.tick().await;
               let Ok(_) = tx_interval.send(ControlSequence::Drain) else {
                   break;
               };
           }
        });

        tokio::spawn(async move {
            while let Some(control) = self.rx.recv().await {
                match control {
                    ControlSequence::InsertOp(op) => {
                        let insert_res = self.journal.insert_op(op);
                        match insert_res {
                            InsertResult::Inserted => {
                                // noop
                            },
                            InsertResult::Flush(batch) => {
                                self.flush(batch);
                            },
                        }
                    },
                    ControlSequence::Drain => {
                        let drainable_batch = self.journal.drain();
                        self.flush(drainable_batch);
                    },
                    ControlSequence::Register(cb) => {
                        self.on_hydrate_callbacks.push(cb);
                    },
                    ControlSequence::Destroy => {
                        self.rx.close();
                    },
                }
            }
        });
    }
}
