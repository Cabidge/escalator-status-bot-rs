pub mod message;
pub mod timeout;
pub mod view;

pub use self::{
    message::{MessageContext, MessageHandle, MessageInterface},
    timeout::{Timeout, TimeoutKind},
    view::ViewBuilder,
};

use self::timeout::TimeoutSleeper;

use futures::{Future, FutureExt};
use poise::async_trait;
use std::{fmt::Display, pin::Pin, str::FromStr};

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
pub trait UserInterface: Sized {
    async fn run<C: Component>(
        &self,
        component: C,
        config: UiConfig,
        signals: Receiver<Signal<C>>,
    ) -> UiResult<C>;

    fn mount<C: Component>(&self, component: C, config: UiConfig) -> UiHandle<C> {
        let (emitter, signals) = tokio::sync::mpsc::unbounded_channel();
        UiHandle {
            emitter,
            task: Box::pin(self.run(component, config, signals)),
        }
    }
}

#[derive(Clone, Default)]
pub struct UiConfig {
    pub timeout: Option<Timeout>,
}

#[derive(Debug)]
pub enum UiError {
    Timeout,
    Incomplete,
    Custom(CustomError),
}

#[derive(Debug)]
pub struct CustomError(anyhow::Error);

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

impl Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::Timeout => write!(f, "Interface timed out"),
            UiError::Incomplete => write!(f, "Interface did not complete"),
            UiError::Custom(err) => err.fmt(f),
        }
    }
}

impl CustomError {
    pub fn new(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(anyhow::Error::new(e))
    }
}

impl Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<CustomError> for UiError {
    fn from(value: CustomError) -> Self {
        Self::Custom(value)
    }
}

impl std::error::Error for UiError {}

impl std::error::Error for CustomError {}
