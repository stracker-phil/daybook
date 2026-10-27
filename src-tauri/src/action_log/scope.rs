//! Which trigger a line belongs to: a `Trigger` handle set for an async task (tokio task-local) or for a
//! stretch of sync code on one thread (`Trigger::enter`). Work handed to another task or thread takes the
//! current trigger along with `carry` and `carry_sync`.

use std::cell::RefCell;
use std::marker::PhantomData;

use chrono::{DateTime, Local};

use super::model::{LineRef, TRIGGER_OPEN};

/// An open event, or a "Daybook" line in its parent's event when it was begun inside another trigger.
#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub(super) event: String,
    pub(super) started: DateTime<Local>,
    pub(super) nested: Option<LineRef>,
}

tokio::task_local! {
    static TASK: Trigger;
}

thread_local! {
    static THREAD: RefCell<Vec<Trigger>> = const { RefCell::new(Vec::new()) };
}

impl Trigger {
    /// Whether lines at `now` still join it.
    pub(super) fn open_at(&self, now: DateTime<Local>) -> bool {
        now - self.started < TRIGGER_OPEN
    }

    /// Runs `work` with this trigger current.
    pub async fn scope<F: Future>(self, work: F) -> F::Output {
        TASK.scope(self, work).await
    }

    /// Makes this trigger current on this thread until the guard drops; not across an `await`.
    pub fn enter(&self) -> Entered {
        THREAD.with(|stack| stack.borrow_mut().push(self.clone()));
        Entered(PhantomData)
    }
}

/// Holds a trigger current on its thread.
pub struct Entered(PhantomData<*const ()>);

impl Drop for Entered {
    fn drop(&mut self) {
        THREAD.with(|stack| stack.borrow_mut().pop());
    }
}

/// The trigger entered on this thread (sync code inside a task enters the narrower one), else this
/// task's.
pub fn current() -> Option<Trigger> {
    THREAD
        .with(|stack| stack.borrow().last().cloned())
        .or_else(|| TASK.try_with(Clone::clone).ok())
}

/// Runs `work` with `trigger` current, or as it is without one.
pub async fn within<F: Future>(trigger: Option<Trigger>, work: F) -> F::Output {
    match trigger {
        Some(trigger) => trigger.scope(work).await,
        None => work.await,
    }
}

/// `work` with the current trigger, for another task.
pub fn carry<F: Future>(work: F) -> impl Future<Output = F::Output> {
    within(current(), work)
}

/// `work` with the current trigger, for another thread.
pub fn carry_sync<R>(work: impl FnOnce() -> R) -> impl FnOnce() -> R {
    let trigger = current();
    move || {
        let _entered = trigger.as_ref().map(Trigger::enter);
        work()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn trigger(event: &str) -> Trigger {
        Trigger {
            event: event.to_string(),
            started: Local::now(),
            nested: None,
        }
    }

    #[test]
    fn there_is_no_current_trigger_outside_any_scope() {
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn a_scope_makes_its_trigger_current_for_the_work_inside() {
        let a = trigger("a");
        let inside = a.clone().scope(async { current() }).await;
        assert_eq!(inside, Some(a));
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn an_entered_trigger_is_current_until_its_guard_drops() {
        let a = trigger("a");
        {
            let _entered = a.enter();
            assert_eq!(current(), Some(a.clone()));
        }
        assert_eq!(current(), None);
    }

    #[tokio::test]
    async fn the_entered_trigger_wins_over_the_task_trigger() {
        let task = trigger("task");
        let narrow = trigger("narrow");
        let seen = task
            .clone()
            .scope(async {
                let before = current();
                let during = {
                    let _entered = narrow.enter();
                    current()
                };
                (before, during, current())
            })
            .await;
        assert_eq!(seen, (Some(task.clone()), Some(narrow), Some(task)));
    }

    #[test]
    fn nested_entered_triggers_restore_the_outer_one_when_they_end() {
        let outer = trigger("outer");
        let inner = trigger("inner");
        let _outer = outer.enter();
        {
            let _inner = inner.enter();
            assert_eq!(current(), Some(inner));
        }
        assert_eq!(current(), Some(outer));
    }

    #[tokio::test]
    async fn within_a_trigger_runs_the_work_with_it_current() {
        let a = trigger("a");
        assert_eq!(within(Some(a.clone()), async { current() }).await, Some(a));
    }

    #[tokio::test]
    async fn within_no_trigger_runs_the_work_as_it_is() {
        assert_eq!(within(None, async { current() }).await, None);
        let a = trigger("a");
        let inside = a
            .clone()
            .scope(async { within(None, async { current() }).await })
            .await;
        assert_eq!(inside, Some(a));
    }

    #[tokio::test]
    async fn carry_takes_the_current_trigger_to_another_task() {
        let a = trigger("a");
        let (carried, plain) = a
            .clone()
            .scope(async {
                let carried = tokio::spawn(carry(async { current() }));
                let plain = tokio::spawn(async { current() });
                (carried.await.unwrap(), plain.await.unwrap())
            })
            .await;
        assert_eq!(carried, Some(a));
        assert_eq!(plain, None);
    }

    #[tokio::test]
    async fn carry_without_a_current_trigger_runs_the_work_without_one() {
        assert_eq!(
            tokio::spawn(carry(async { current() })).await.unwrap(),
            None
        );
    }

    #[test]
    fn carry_sync_takes_the_current_trigger_to_another_thread() {
        let a = trigger("a");
        let (carried, plain) = {
            let _entered = a.enter();
            let carried = thread::spawn(carry_sync(current));
            let plain = thread::spawn(current);
            (carried.join().unwrap(), plain.join().unwrap())
        };
        assert_eq!(carried, Some(a));
        assert_eq!(plain, None);
    }

    #[tokio::test]
    async fn carry_sync_takes_a_task_trigger_to_a_thread() {
        let a = trigger("a");
        let carried = a
            .clone()
            .scope(async { thread::spawn(carry_sync(current)).join().unwrap() })
            .await;
        assert_eq!(carried, Some(a));
    }

    #[test]
    fn carry_sync_without_a_current_trigger_runs_the_work_without_one() {
        assert_eq!(thread::spawn(carry_sync(current)).join().unwrap(), None);
    }
}
