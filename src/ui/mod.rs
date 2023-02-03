pub mod message;
pub mod timeout;
pub mod view;

use self::timeout::TimeoutSleeper;
pub use self::{
    message::{MessageContext, MessageHandle, MessageInterface},
    timeout::{Timeout, TimeoutKind},
    view::ViewBuilder,
};

use futures::{Future, FutureExt};
use poise::async_trait;
use std::{pin::Pin, str::FromStr, fmt::Display};

pub type UiResult<C> = Result<<C as Component>::Output, UiError>;

type Receiver<T> = tokio::sync::mpsc::UnboundedReceiver<T>;
type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;

pub trait Component: Sized + Send + Sync + 'static {
    type Action: FromStr<Err = Self::ActionErr> + Send;
    type ActionErr: Display + Send;
    type Output: Send;

    fn render(&self, view: &mut ViewBuilder);
    fn update(&mut self, action: Self::Action) -> Option<Update>;
    fn conclude(self) -> Option<Self::Output>;

    fn render_output(_output: &Self::Output, view: &mut ViewBuilder) {
        view.add_content("*interaction ended*");
    }

    fn to_view(&self) -> ViewBuilder {
        let mut view = ViewBuilder::new();
        self.render(&mut view);
        view
    }
}

#[async_trait]
pub trait UserInterface<'a>: Sized + 'a {
    async fn run<C: Component>(
        self,
        component: C,
        config: UiConfig,
        signals: Receiver<Signal<C>>,
    ) -> UiResult<C>;

    fn mount<C: Component>(self, component: C, config: UiConfig) -> UiHandle<'a, C> {
        let (emitter, signals) = tokio::sync::mpsc::unbounded_channel();
        UiHandle {
            emitter,
            task: Box::pin(self.run(component, config, signals)),
        }
    }
}

#[derive(Clone, Default)]
pub struct UiConfig {
    pub ephemeral: bool,
    pub timeout: Option<Timeout>,
}

pub enum UiError {
    Timeout,
    Incomplete,
    Custom(anyhow::Error),
}

pub struct UiHandle<'a, C: Component> {
    emitter: Sender<Signal<C>>,
    task: Pin<Box<dyn Future<Output = UiResult<C>> + Send + 'a>>,
}

pub enum Signal<C: Component> {
    Action(C::Action),
    Halt,
}

pub enum Update {
    Render,
    Halt,
}

impl UiConfig {
    pub fn sleeper(&self) -> Option<TimeoutSleeper> {
        self.timeout.map(TimeoutSleeper::from)
    }
}

impl<'a, C: Component> UiHandle<'a, C> {
    pub fn halt(&self) {
        let _ = self.emitter.send(Signal::Halt).ok();
    }

    pub fn emitter(&self) -> &Sender<Signal<C>> {
        &self.emitter
    }
}

impl<'a, C: Component> Future for UiHandle<'a, C> {
    type Output = UiResult<C>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.task.poll_unpin(cx)
    }
}

impl<E: std::error::Error + Send + Sync + 'static> From<E> for UiError {
    fn from(value: E) -> Self {
        Self::Custom(value.into())
    }
}
