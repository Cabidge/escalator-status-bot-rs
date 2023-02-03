use futures::Future;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::Instant;

pub struct TimeoutSleeper {
    timeout: Timeout,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Timeout {
    pub kind: TimeoutKind,
    pub duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeoutKind {
    /// The Timeout can reset upon an interaction
    Refresh,
    /// The Timeout will not reset the time
    Oneshot,
}

impl TimeoutSleeper {
    /// Alerts the sleeper to reset the timer if the timeout is on repeating mode.
    pub fn notify(&mut self) {
        if self.timeout.kind == TimeoutKind::Refresh {
            self.reset();
        }
    }

    /// Resets the sleep.
    fn reset(&mut self) {
        self.sleep.set(tokio::time::sleep(self.timeout.duration));
    }

    pub fn time_left(&self) -> Duration {
        self.sleep.deadline().duration_since(Instant::now())
    }
}

impl Future for TimeoutSleeper {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.sleep.as_mut().poll(cx)
    }
}

impl From<Timeout> for TimeoutSleeper {
    fn from(timeout: Timeout) -> Self {
        let sleep = tokio::time::sleep(timeout.duration);
        TimeoutSleeper {
            timeout,
            sleep: Box::pin(sleep),
        }
    }
}
