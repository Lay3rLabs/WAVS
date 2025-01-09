use layer_wasi::Reactor;

use crate::{session::Session, TaskInput};

pub mod groq;
pub mod ollama;

pub trait Provider {
    /// the name of the provider that the env var will match
    const NAME: &'static str;

    /// the environment variables that the provider needs
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// the function that processes the input and returns the session
    async fn process(&self, reactor: &Reactor, input: &TaskInput) -> Result<Session, String>;
}
