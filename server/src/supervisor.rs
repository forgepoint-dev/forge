use std::future::Future;

use anyhow::{Error, Result};
use tokio::task::{JoinError, JoinSet};
use tokio_util::sync::CancellationToken;

pub struct Supervisor {
    shutdown: CancellationToken,
    tasks: JoinSet<(String, Result<()>)>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            shutdown: CancellationToken::new(),
            tasks: JoinSet::new(),
        }
    }

    pub fn spawn<F, Fut>(&mut self, name: &'static str, factory: F)
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let shutdown = self.shutdown.child_token();
        self.tasks.spawn(async move {
            let result = factory(shutdown).await;
            (name.to_string(), result)
        });
    }

    pub async fn run(mut self) -> Result<()> {
        let mut first_err: Option<Error> = None;

        while !self.tasks.is_empty() {
            tokio::select! {
                Some(outcome) = self.tasks.join_next() => {
                    self.handle_task_outcome(&mut first_err, outcome);
                }
                _ = tokio::signal::ctrl_c(), if !self.shutdown.is_cancelled() => {
                    self.shutdown.cancel();
                }
            }
        }

        if let Some(err) = first_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn handle_task_outcome(
        &self,
        first_err: &mut Option<Error>,
        outcome: std::result::Result<(String, Result<()>), JoinError>,
    ) {
        match outcome {
            Ok((name, Ok(()))) => {
                eprintln!("child `{name}` exited gracefully");
            }
            Ok((name, Err(err))) => {
                eprintln!("child `{name}` exited with error: {err}");
                if first_err.is_none() {
                    *first_err = Some(err);
                }
                if !self.shutdown.is_cancelled() {
                    eprintln!("supervisor shutting down");
                    self.shutdown.cancel();
                }
            }
            Err(join_err) => {
                eprintln!("child panicked: {join_err:?}");
                if first_err.is_none() {
                    *first_err = Some(join_err.into());
                }
                if !self.shutdown.is_cancelled() {
                    eprintln!("supervisor shutting down");
                    self.shutdown.cancel();
                }
            }
        }
    }
}
