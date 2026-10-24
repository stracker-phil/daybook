//! Building a command: `Command::new(id, title)`, its flags, then its work.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::Host;
use crate::error::AppResult;

pub type RunFuture = Pin<Box<dyn Future<Output = AppResult<()>> + Send>>;
pub(super) type Run<H> = Arc<dyn Fn(H, String) -> RunFuture + Send + Sync>;
type Check<H> = Arc<dyn Fn(&H) -> bool + Send + Sync>;
type ChoicesFn<H> = Arc<dyn Fn(&H) -> Vec<ArgChoice> + Send + Sync>;
/// The argument from the command line's words; `Err` says why they don't fit.
pub type FromWords = fn(&[String]) -> Result<String, String>;

/// A family's argument as the command line offers it: `word` is typed, `argument` is run.
#[derive(Clone, Debug, PartialEq)]
pub struct ArgChoice {
    pub word: String,
    pub title: Option<String>,
    pub argument: String,
}

impl ArgChoice {
    pub fn new(word: impl Into<String>) -> Self {
        let word = word.into();
        Self {
            argument: word.clone(),
            word,
            title: None,
        }
    }
}

pub struct Info<H> {
    pub title: &'static str,
    /// One line for `daybook help`.
    pub help: &'static str,
    /// Left out of help and the command list (e.g. closing whichever window has focus).
    pub hidden: bool,
    /// Without a vault it opens Settings instead.
    pub needs_vault: bool,
    /// The app is gone once it ran, so a caller answers first.
    pub quits: bool,
    pub(super) enabled: Option<Check<H>>,
}

impl<H> Info<H> {
    /// The first word of the id: `hub` for `hub:open`.
    pub fn group<'a>(&self, id: &'a str) -> &'a str {
        id.split(':').next().unwrap_or(id)
    }
}

/// A command before it gets its `run`.
pub struct Spec<H> {
    pub(super) id: &'static str,
    pub(super) info: Info<H>,
}

impl<H: Host> Spec<H> {
    pub fn help(mut self, help: &'static str) -> Self {
        self.info.help = help;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.info.hidden = true;
        self
    }

    pub fn needs_vault(mut self) -> Self {
        self.info.needs_vault = true;
        self
    }

    pub fn quits(mut self) -> Self {
        self.info.quits = true;
        self
    }

    /// A disabled command doesn't run, is greyed in menus and left out of the palette.
    pub fn enabled_when(mut self, check: impl Fn(&H) -> bool + Send + Sync + 'static) -> Self {
        self.info.enabled = Some(Arc::new(check));
        self
    }

    pub fn run<F, Fut>(self, run: F) -> Command<H>
    where
        F: Fn(H) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AppResult<()>> + Send + 'static,
    {
        Command {
            id: self.id,
            info: self.info,
            run: Arc::new(move |host, _| Box::pin(run(host))),
        }
    }

    /// For work that returns at once.
    pub fn run_now(self, run: impl Fn(&H) + Send + Sync + 'static) -> Command<H> {
        let run = Arc::new(run);
        self.run(move |host| {
            let run = Arc::clone(&run);
            async move {
                run(&host);
                Ok(())
            }
        })
    }

    /// A family: the id is the prefix, `prefix:` + argument runs it.
    pub fn family(self, arg: &'static str) -> FamilySpec<H> {
        FamilySpec {
            prefix: self.id,
            arg,
            info: self.info,
            choices: None,
            from_words: None,
        }
    }
}

pub struct Command<H> {
    pub(super) id: &'static str,
    pub(super) info: Info<H>,
    pub(super) run: Run<H>,
}

impl<H: Host> Command<H> {
    /// A command is built in two steps: its spec, then `run` adds the work.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: &'static str, title: &'static str) -> Spec<H> {
        Spec {
            id,
            info: Info {
                title,
                help: title,
                hidden: false,
                needs_vault: false,
                quits: false,
                enabled: None,
            },
        }
    }
}

pub struct FamilySpec<H> {
    pub(super) prefix: &'static str,
    pub(super) arg: &'static str,
    pub(super) info: Info<H>,
    pub(super) choices: Option<ChoicesFn<H>>,
    pub(super) from_words: Option<FromWords>,
}

impl<H: Host> FamilySpec<H> {
    /// What the argument can be right now; the command line accepts nothing else.
    pub fn choices(
        mut self,
        choices: impl Fn(&H) -> Vec<ArgChoice> + Send + Sync + 'static,
    ) -> Self {
        self.choices = Some(Arc::new(choices));
        self
    }

    /// The argument from the command line's words, when it isn't simply the words.
    pub fn from_words(mut self, from_words: FromWords) -> Self {
        self.from_words = Some(from_words);
        self
    }

    pub fn run<F, Fut>(self, run: F) -> Family<H>
    where
        F: Fn(H, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AppResult<()>> + Send + 'static,
    {
        Family {
            prefix: self.prefix,
            arg: self.arg,
            info: self.info,
            choices: self.choices,
            from_words: self.from_words,
            run: Arc::new(move |host, argument| Box::pin(run(host, argument))),
        }
    }
}

pub struct Family<H> {
    pub(super) prefix: &'static str,
    /// The argument as help shows it, e.g. `<context>`.
    pub arg: &'static str,
    pub(super) info: Info<H>,
    pub(super) choices: Option<ChoicesFn<H>>,
    pub from_words: Option<FromWords>,
    pub(super) run: Run<H>,
}

impl<H: Host> Family<H> {
    pub fn choices(&self, host: &H) -> Option<Vec<ArgChoice>> {
        self.choices.as_ref().map(|choices| choices(host))
    }
}
