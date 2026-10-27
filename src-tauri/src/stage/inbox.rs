//! Replies from several pages to one request: a request opens a mailbox under a fresh id, the pages'
//! replies with that id land in it, and closing it (or dropping the ticket) refuses later ones.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::lock::MutexExt;

type Senders<T> = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<T>>>>;

pub struct Inbox<T> {
    open: Senders<T>,
}

impl<T> Default for Inbox<T> {
    fn default() -> Self {
        Self {
            open: Arc::default(),
        }
    }
}

/// An open mailbox; dropping it closes it.
pub struct Ticket<T> {
    pub id: String,
    pub replies: mpsc::UnboundedReceiver<T>,
    open: Senders<T>,
}

impl<T> Drop for Ticket<T> {
    fn drop(&mut self) {
        self.open.lock_unpoisoned().remove(&self.id);
    }
}

impl<T> Inbox<T> {
    pub fn open(&self) -> Ticket<T> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, replies) = mpsc::unbounded_channel();
        self.open.lock_unpoisoned().insert(id.clone(), tx);
        Ticket {
            id,
            replies,
            open: Arc::clone(&self.open),
        }
    }

    /// A reply for a mailbox that is closed (finished, timed out or unknown) is dropped.
    pub fn deliver(&self, id: &str, reply: T) {
        if let Some(tx) = self.open.lock_unpoisoned().get(id) {
            let _ = tx.send(reply);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_reach_the_open_mailbox_in_order() {
        let inbox = Inbox::default();
        let mut ticket = inbox.open();

        inbox.deliver(&ticket.id, 1);
        inbox.deliver(&ticket.id, 2);

        assert_eq!(ticket.replies.try_recv().ok(), Some(1));
        assert_eq!(ticket.replies.try_recv().ok(), Some(2));
        assert_eq!(ticket.replies.try_recv().ok(), None);
    }

    #[test]
    fn a_reply_for_an_unknown_request_is_refused() {
        let inbox = Inbox::default();
        let mut ticket = inbox.open();

        inbox.deliver("no-such-request", 1);
        assert_eq!(ticket.replies.try_recv().ok(), None);
    }

    #[test]
    fn a_closed_mailbox_refuses_later_replies() {
        let inbox = Inbox::default();
        let ticket = inbox.open();
        let id = ticket.id.clone();

        drop(ticket);
        let mut other = inbox.open();

        inbox.deliver(&id, 1);

        assert_eq!(other.replies.try_recv().ok(), None);
    }

    #[test]
    fn every_request_gets_its_own_mailbox() {
        let inbox = Inbox::default();
        let mut first = inbox.open();
        let mut second = inbox.open();

        assert_ne!(first.id, second.id);
        inbox.deliver(&second.id, 7);

        assert_eq!(first.replies.try_recv().ok(), None);
        assert_eq!(second.replies.try_recv().ok(), Some(7));
    }

    #[test]
    fn closing_one_mailbox_leaves_the_others_open() {
        let inbox = Inbox::default();
        let first = inbox.open();
        let mut second = inbox.open();

        drop(first);

        inbox.deliver(&second.id, 3);
        assert_eq!(second.replies.try_recv().ok(), Some(3));
    }
}
