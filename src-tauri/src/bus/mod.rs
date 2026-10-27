//! Every action is a command with an id. Menus, global shortcuts, the palette and automation run commands
//! by id, so callers never import the features; each feature registers its commands here.
//!
//! The bus is generic over its host (the app, or a test double), which knows whether a vault is set and
//! reports failures.

pub mod app;
mod command;
mod commands;

pub use app::{Commands, spawn};
pub use command::{ArgChoice, Command, Family, FromWords, Info, RunFuture};
pub use commands::{register, register_aliases};

use crate::error::{AppError, AppResult};

use self::command::Run;

/// What the bus needs from the app around it.
pub trait Host: Clone + Send + Sync + 'static {
    fn has_vault(&self) -> bool;
    /// What a command that needs a vault does without one: open Settings to choose it.
    fn choose_vault(&self);
    fn report_failure(&self, id: &str, error: &AppError);
}

/// A command or family in registration order.
pub enum Listed<'a, H> {
    Command { id: &'a str, info: &'a Info<H> },
    Family(&'a Family<H>),
}

impl<'a, H> Listed<'a, H> {
    pub fn id(&self) -> &'a str {
        match self {
            Self::Command { id, .. } => id,
            Self::Family(family) => family.prefix,
        }
    }

    pub fn info(&self) -> &'a Info<H> {
        match self {
            Self::Command { info, .. } => info,
            Self::Family(family) => &family.info,
        }
    }
}

enum Entry<H> {
    Command(Command<H>),
    Family(Family<H>),
}

struct Found<'a, H> {
    info: &'a Info<H>,
    run: &'a Run<H>,
    /// Where the argument starts in the id.
    argument: usize,
}

pub struct CommandBus<H> {
    entries: Vec<Entry<H>>,
}

impl<H> Default for CommandBus<H> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<H: Host> CommandBus<H> {
    /// A later registration of the same id replaces the earlier one.
    pub fn add(&mut self, command: Command<H>) {
        self.entries
            .retain(|entry| !matches!(entry, Entry::Command(c) if c.id == command.id));
        self.entries.push(Entry::Command(command));
    }

    pub fn add_family(&mut self, family: Family<H>) {
        self.entries
            .retain(|entry| !matches!(entry, Entry::Family(f) if f.prefix == family.prefix));
        self.entries.push(Entry::Family(family));
    }

    /// An exact id first, else the family with the longest matching prefix.
    fn find(&self, id: &str) -> Option<Found<'_, H>> {
        let exact = self.entries.iter().find_map(|entry| match entry {
            Entry::Command(c) if c.id == id => Some(Found {
                info: &c.info,
                run: &c.run,
                argument: id.len(),
            }),
            _ => None,
        });
        exact.or_else(|| {
            self.entries
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Family(f) if id.starts_with(f.prefix) => Some(f),
                    _ => None,
                })
                .max_by_key(|f| f.prefix.len())
                .map(|f| Found {
                    info: &f.info,
                    run: &f.run,
                    argument: f.prefix.len(),
                })
        })
    }

    pub fn available<'a>(&'a self, host: &'a H) -> Available<'a, H> {
        Available { bus: self, host }
    }

    pub fn knows(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    pub fn info(&self, id: &str) -> Option<&Info<H>> {
        self.find(id).map(|found| found.info)
    }

    /// Known and not switched off by its enabled check.
    pub fn is_enabled(&self, host: &H, id: &str) -> bool {
        self.find(id).is_some_and(|found| enabled(found.info, host))
    }

    pub fn family(&self, prefix: &str) -> Option<&Family<H>> {
        self.entries.iter().find_map(|entry| match entry {
            Entry::Family(f) if f.prefix == prefix => Some(f),
            _ => None,
        })
    }

    pub fn list(&self) -> impl Iterator<Item = Listed<'_, H>> {
        self.entries.iter().map(|entry| match entry {
            Entry::Command(c) => Listed::Command {
                id: c.id,
                info: &c.info,
            },
            Entry::Family(f) => Listed::Family(f),
        })
    }

    /// The command's work, ready to await; refused when it is unknown or disabled. Without a vault a
    /// command that needs one opens Settings instead.
    pub fn start(&self, host: H, id: &str) -> AppResult<RunFuture> {
        let found = self
            .find(id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown command “{id}”.")))?;
        if !enabled(found.info, &host) {
            return Err(AppError::InvalidInput(format!(
                "“{id}” isn't available right now."
            )));
        }
        if found.info.needs_vault && !host.has_vault() {
            host.choose_vault();
            return Ok(Box::pin(async { Ok(()) }));
        }
        Ok((found.run)(host, id[found.argument..].to_string()))
    }

    /// Returns the failure instead of reporting it, for callers that report it themselves.
    pub async fn run_or_err(&self, host: H, id: &str) -> AppResult<()> {
        self.start(host, id)?.await
    }

    /// Logs an unknown id; a failure is logged and reported by the host.
    pub async fn run(&self, host: H, id: &str) {
        if !self.knows(id) {
            log::warn!("Unknown command: {id}");
            return;
        }
        if let Err(e) = self.run_or_err(host.clone(), id).await {
            log::error!("{id} failed: {e}");
            host.report_failure(id, &e);
        }
    }
}

/// Which commands exist and can run now; menus leave out unknown ones and grey disabled ones.
pub trait CommandStatus {
    fn knows(&self, id: &str) -> bool;
    fn is_enabled(&self, id: &str) -> bool;
}

/// The bus seen from one host.
pub struct Available<'a, H> {
    bus: &'a CommandBus<H>,
    host: &'a H,
}

impl<H: Host> CommandStatus for Available<'_, H> {
    fn knows(&self, id: &str) -> bool {
        self.bus.knows(id)
    }

    fn is_enabled(&self, id: &str) -> bool {
        self.bus.is_enabled(self.host, id)
    }
}

fn enabled<H>(info: &Info<H>, host: &H) -> bool {
    info.enabled.as_ref().is_none_or(|check| check(host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct TestHost {
        vault: bool,
        enabled: Arc<AtomicBool>,
        vault_chosen: Arc<AtomicBool>,
        failures: Arc<Mutex<Vec<(String, AppError)>>>,
        ran: Arc<Mutex<Vec<String>>>,
    }

    impl TestHost {
        fn with_vault() -> Self {
            Self {
                vault: true,
                enabled: Arc::new(AtomicBool::new(true)),
                ..Self::default()
            }
        }

        fn without_vault() -> Self {
            Self {
                vault: false,
                ..Self::with_vault()
            }
        }

        fn ran(&self) -> Vec<String> {
            self.ran.lock().unwrap().clone()
        }

        fn failures(&self) -> Vec<(String, AppError)> {
            self.failures.lock().unwrap().clone()
        }

        fn vault_chosen(&self) -> bool {
            self.vault_chosen.load(Ordering::SeqCst)
        }
    }

    impl Host for TestHost {
        fn has_vault(&self) -> bool {
            self.vault
        }

        fn choose_vault(&self) {
            self.vault_chosen.store(true, Ordering::SeqCst);
        }

        fn report_failure(&self, id: &str, error: &AppError) {
            self.failures
                .lock()
                .unwrap()
                .push((id.to_string(), error.clone()));
        }
    }

    fn logging(id: &'static str) -> Command<TestHost> {
        Command::new(id, "Title").run_now(move |host: &TestHost| {
            host.ran.lock().unwrap().push(id.to_string());
        })
    }

    fn logging_family(prefix: &'static str) -> Family<TestHost> {
        Command::new(prefix, "Title").family("<arg>").run(
            move |host: TestHost, argument| async move {
                host.ran.lock().unwrap().push(format!("{prefix}{argument}"));
                Ok(())
            },
        )
    }

    fn failing(id: &'static str) -> Command<TestHost> {
        Command::new(id, "Title").run(|_| async { Err(AppError::Io("boom".into())) })
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found() {
        let bus = CommandBus::<TestHost>::default();
        let error = bus
            .run_or_err(TestHost::with_vault(), "nope:nothing")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::NotFound(_)), "{error:?}");
        assert!(!bus.knows("nope:nothing"));
        assert!(bus.info("nope:nothing").is_none());
    }

    #[tokio::test]
    async fn a_registered_command_runs() {
        let mut bus = CommandBus::default();
        bus.add(logging("hub:open"));
        let host = TestHost::with_vault();

        assert_eq!(bus.run_or_err(host.clone(), "hub:open").await, Ok(()));

        assert!(bus.knows("hub:open"));
        assert_eq!(host.ran(), ["hub:open"]);
    }

    #[tokio::test]
    async fn a_failing_command_returns_its_error() {
        let mut bus = CommandBus::default();
        bus.add(failing("hub:open"));
        assert_eq!(
            bus.run_or_err(TestHost::with_vault(), "hub:open").await,
            Err(AppError::Io("boom".into()))
        );
    }

    #[tokio::test]
    async fn run_hands_a_failure_to_the_host() {
        let mut bus = CommandBus::default();
        bus.add(failing("hub:open"));
        let host = TestHost::with_vault();

        bus.run(host.clone(), "hub:open").await;

        assert_eq!(
            host.failures(),
            [("hub:open".to_string(), AppError::Io("boom".into()))]
        );
    }

    #[tokio::test]
    async fn run_does_not_report_an_unknown_id() {
        let bus = CommandBus::<TestHost>::default();
        let host = TestHost::with_vault();
        bus.run(host.clone(), "nope:nothing").await;
        assert!(host.failures().is_empty());
    }

    #[tokio::test]
    async fn run_does_not_report_a_success() {
        let mut bus = CommandBus::default();
        bus.add(logging("hub:open"));
        let host = TestHost::with_vault();
        bus.run(host.clone(), "hub:open").await;
        assert_eq!(host.ran(), ["hub:open"]);
        assert!(host.failures().is_empty());
    }

    #[tokio::test]
    async fn a_family_runs_with_the_argument_after_its_prefix() {
        let mut bus = CommandBus::default();
        bus.add_family(logging_family("context:set:"));
        let host = TestHost::with_vault();

        bus.run_or_err(host.clone(), "context:set:Deep work")
            .await
            .unwrap();

        assert!(bus.knows("context:set:Deep work"));
        assert_eq!(host.ran(), ["context:set:Deep work"]);
    }

    #[tokio::test]
    async fn the_longest_matching_family_prefix_wins() {
        let mut bus = CommandBus::default();
        bus.add_family(Command::new("settings:", "Settings").family("<page>").run(
            |host: TestHost, argument| async move {
                host.ran.lock().unwrap().push(format!("page={argument}"));
                Ok(())
            },
        ));
        bus.add_family(
            Command::new("settings:open:", "Settings")
                .family("<page>")
                .run(|host: TestHost, argument| async move {
                    host.ran.lock().unwrap().push(format!("open={argument}"));
                    Ok(())
                }),
        );
        let host = TestHost::with_vault();

        bus.run_or_err(host.clone(), "settings:open:vault")
            .await
            .unwrap();
        bus.run_or_err(host.clone(), "settings:vault")
            .await
            .unwrap();

        assert_eq!(host.ran(), ["open=vault", "page=vault"]);
    }

    #[tokio::test]
    async fn an_exact_command_beats_a_family() {
        let mut bus = CommandBus::default();
        bus.add_family(logging_family("settings:"));
        bus.add(logging("settings:open"));
        let host = TestHost::with_vault();

        bus.run_or_err(host.clone(), "settings:open").await.unwrap();

        assert_eq!(host.ran(), ["settings:open"]);
    }

    #[test]
    fn a_family_exposes_its_argument_choices_and_word_parsing() {
        let mut bus = CommandBus::<TestHost>::default();
        bus.add_family(
            Command::new("context:set:", "Context")
                .family("<context>")
                .choices(|_| vec![ArgChoice::new("work"), ArgChoice::new("home")])
                .argument_from_words(|words| {
                    if words.is_empty() {
                        Err("Needs a context.".into())
                    } else {
                        Ok(words.join(" "))
                    }
                })
                .run(|_, _| async { Ok(()) }),
        );
        bus.add_family(logging_family("plain:"));

        let family = bus.family("context:set:").unwrap();
        assert_eq!(family.arg, "<context>");
        assert_eq!(
            family.choices(&TestHost::with_vault()),
            Some(vec![ArgChoice::new("work"), ArgChoice::new("home")])
        );
        let from_words = family.from_words.unwrap();
        assert_eq!(
            from_words(&["deep".to_string(), "work".to_string()]),
            Ok("deep work".to_string())
        );
        assert_eq!(from_words(&[]), Err("Needs a context.".to_string()));

        let plain = bus.family("plain:").unwrap();
        assert_eq!(plain.choices(&TestHost::with_vault()), None);
        assert!(plain.from_words.is_none());
        assert!(bus.family("missing:").is_none());
    }

    #[tokio::test]
    async fn a_disabled_command_is_refused_and_does_not_run() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::new("status:toggle", "Status")
                .enabled_when(|host: &TestHost| host.enabled.load(Ordering::SeqCst))
                .run_now(|host| host.ran.lock().unwrap().push("status:toggle".into())),
        );
        let host = TestHost::with_vault();
        host.enabled.store(false, Ordering::SeqCst);

        assert!(bus.knows("status:toggle"));
        assert!(!bus.is_enabled(&host, "status:toggle"));
        let error = bus
            .run_or_err(host.clone(), "status:toggle")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::InvalidInput(_)), "{error:?}");
        assert!(host.ran().is_empty());

        host.enabled.store(true, Ordering::SeqCst);
        assert!(bus.is_enabled(&host, "status:toggle"));
        bus.run_or_err(host.clone(), "status:toggle").await.unwrap();
        assert_eq!(host.ran(), ["status:toggle"]);
    }

    #[test]
    fn an_unknown_command_is_not_enabled() {
        let bus = CommandBus::<TestHost>::default();
        assert!(!bus.is_enabled(&TestHost::with_vault(), "nope:nothing"));
    }

    #[tokio::test]
    async fn a_command_that_needs_a_vault_asks_for_one_instead_of_running() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::new("search:open", "Search")
                .needs_vault()
                .run_now(|host: &TestHost| host.ran.lock().unwrap().push("search:open".into())),
        );
        let host = TestHost::without_vault();

        bus.run_or_err(host.clone(), "search:open").await.unwrap();

        assert!(host.vault_chosen());
        assert!(host.ran().is_empty());
    }

    #[tokio::test]
    async fn a_command_that_needs_a_vault_runs_with_one() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::new("search:open", "Search")
                .needs_vault()
                .run_now(|host: &TestHost| host.ran.lock().unwrap().push("search:open".into())),
        );
        let host = TestHost::with_vault();

        bus.run_or_err(host.clone(), "search:open").await.unwrap();

        assert!(!host.vault_chosen());
        assert_eq!(host.ran(), ["search:open"]);
    }

    #[tokio::test]
    async fn a_command_without_the_vault_flag_runs_without_a_vault() {
        let mut bus = CommandBus::default();
        bus.add(logging("app:settings"));
        let host = TestHost::without_vault();

        bus.run_or_err(host.clone(), "app:settings").await.unwrap();

        assert!(!host.vault_chosen());
        assert_eq!(host.ran(), ["app:settings"]);
    }

    #[tokio::test]
    async fn registering_an_id_again_replaces_the_command() {
        let mut bus = CommandBus::default();
        bus.add(failing("hub:open"));
        bus.add(logging("hub:open"));
        let host = TestHost::with_vault();

        bus.run_or_err(host.clone(), "hub:open").await.unwrap();

        assert_eq!(host.ran(), ["hub:open"]);
        assert_eq!(bus.list().count(), 1);
    }

    #[test]
    fn list_keeps_registration_order_across_commands_and_families() {
        let mut bus = CommandBus::<TestHost>::default();
        bus.add(logging("hub:open"));
        bus.add_family(logging_family("context:set:"));
        bus.add(logging("app:settings"));

        let ids: Vec<&str> = bus.list().map(|listed| listed.id()).collect();
        assert_eq!(ids, ["hub:open", "context:set:", "app:settings"]);
        let kinds: Vec<bool> = bus
            .list()
            .map(|listed| matches!(listed, Listed::Family(_)))
            .collect();
        assert_eq!(kinds, [false, true, false]);
    }

    #[test]
    fn info_carries_the_flags_and_the_group_is_the_first_word_of_the_id() {
        let mut bus = CommandBus::<TestHost>::default();
        bus.add(
            Command::new("hub:open", "Open Daybook")
                .help("Opens the window.")
                .hidden()
                .needs_vault()
                .quits()
                .run_now(|_| {}),
        );
        bus.add(logging("plain"));

        let info = bus.info("hub:open").unwrap();
        assert_eq!(info.title, "Open Daybook");
        assert_eq!(info.help, "Opens the window.");
        assert!(info.hidden && info.needs_vault && info.quits);
        let defaults = bus.info("plain").unwrap();
        assert_eq!(defaults.help, "Title");
        assert!(!defaults.hidden && !defaults.needs_vault && !defaults.quits);
    }

    #[test]
    fn available_reports_what_exists_and_what_is_enabled_for_the_host() {
        let mut bus = CommandBus::default();
        bus.add(logging("hub:open"));
        bus.add(
            Command::new("status:toggle", "Status")
                .enabled_when(|host: &TestHost| host.enabled.load(Ordering::SeqCst))
                .run_now(|_| {}),
        );
        let host = TestHost::with_vault();
        host.enabled.store(false, Ordering::SeqCst);

        let status = bus.available(&host);

        assert!(status.knows("hub:open"));
        assert!(status.is_enabled("hub:open"));
        assert!(status.knows("status:toggle"));
        assert!(!status.is_enabled("status:toggle"));
        assert!(!status.knows("nope:nothing"));
        assert!(!status.is_enabled("nope:nothing"));
    }
}
