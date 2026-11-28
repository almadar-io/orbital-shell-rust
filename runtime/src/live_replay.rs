use orbital_core::LiveBroadcastItem;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Frames a connection was already sent that stay for a reopen naming an earlier one: what a dying socket swallowed.
pub(super) const SENT_KEPT: u64 = 256;

/// One client's numbered live frames. It outlives the client's streams, so a frame made while no stream is open, or
/// written to a stream that was already gone, is sent when the client reopens after the last frame it applied.
struct Mailbox {
    receiver: broadcast::Receiver<LiveBroadcastItem>,
    last: u64,
    sent: u64,
    frames: VecDeque<(u64, LiveBroadcastItem)>,
    streams: usize,
}

pub(super) struct LiveMailboxes {
    boot: String,
    capacity: usize,
    boxes: Mutex<HashMap<String, Arc<Mutex<Mailbox>>>>,
}

pub(super) enum MailPoll {
    Frame(String, Box<LiveBroadcastItem>),
    Idle,
    Ended,
}

pub(super) struct MailStream {
    mailbox: Arc<Mutex<Mailbox>>,
    client: String,
    boot: String,
    capacity: usize,
    cursor: u64,
}

fn parse_since(since: &str) -> Option<(&str, u64)> {
    let (boot, n) = since.rsplit_once('-')?;
    Some((boot, n.parse().ok()?))
}

impl LiveMailboxes {
    pub(super) fn new(capacity: usize) -> Self {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        Self { boot: format!("{millis:x}"), capacity, boxes: Mutex::default() }
    }

    /// A stream of `client`'s frames, resumed after the frame `since` names (`<boot>-<n>`, the id of the last frame the
    /// client applied); with none, or one of another boot, after the last frame any of its streams was sent. A mailbox
    /// with no stream whose session `ended` is dropped.
    pub(super) fn open(
        &self,
        client: &str,
        since: Option<&str>,
        subscribe: impl FnOnce() -> broadcast::Receiver<LiveBroadcastItem>,
        ended: impl Fn(&str) -> bool,
    ) -> MailStream {
        let mut boxes = self.boxes.lock();
        boxes.retain(|name, mailbox| name == client || mailbox.lock().streams > 0 || !ended(name));
        let mailbox = Arc::clone(boxes.entry(client.to_string()).or_insert_with(|| {
            Arc::new(Mutex::new(Mailbox { receiver: subscribe(), last: 0, sent: 0, frames: VecDeque::new(), streams: 0 }))
        }));
        drop(boxes);
        let cursor = {
            let mut held = mailbox.lock();
            held.streams += 1;
            match since.and_then(parse_since) {
                Some((boot, n)) if boot == self.boot && n <= held.last => {
                    let oldest = held.frames.front().map_or(held.last + 1, |frame| frame.0);
                    if n + 1 < oldest {
                        tracing::error!(target: orbital_core::logging::ns::SERVER_DATA, session = client, since = n, oldest, "session live stream resumed past its kept frames: items were dropped");
                    }
                    while held.frames.front().is_some_and(|frame| frame.0 <= n) {
                        held.frames.pop_front();
                    }
                    n
                }
                _ => held.sent,
            }
        };
        MailStream { mailbox, client: client.to_string(), boot: self.boot.clone(), capacity: self.capacity, cursor }
    }
}

impl MailStream {
    /// The next frame after this stream's last, numbering every item `admit` lets through as it arrives.
    pub(super) fn poll(&mut self, mut admit: impl FnMut(&LiveBroadcastItem) -> bool) -> MailPoll {
        let mut held = self.mailbox.lock();
        let mut closed = false;
        loop {
            match held.receiver.try_recv() {
                Ok(item) => {
                    if admit(&item) {
                        held.last += 1;
                        let n = held.last;
                        held.frames.push_back((n, item));
                    }
                }
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    tracing::error!(target: orbital_core::logging::ns::SERVER_DATA, skipped, session = self.client.as_str(), "session live stream lagged: items were dropped");
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    closed = true;
                    break;
                }
            }
        }
        while held.frames.len() > self.capacity {
            if let Some((n, _)) = held.frames.pop_front() {
                if n > held.sent {
                    tracing::error!(target: orbital_core::logging::ns::SERVER_DATA, frame = n, session = self.client.as_str(), "session live mailbox is full: an unsent item was dropped");
                }
            }
        }
        let at = held.frames.partition_point(|frame| frame.0 <= self.cursor);
        let Some((n, item)) = held.frames.get(at).cloned() else {
            return if closed { MailPoll::Ended } else { MailPoll::Idle };
        };
        self.cursor = n;
        held.sent = held.sent.max(n);
        let sent = held.sent;
        while held.frames.front().is_some_and(|frame| frame.0 + SENT_KEPT <= sent) {
            held.frames.pop_front();
        }
        MailPoll::Frame(format!("{}-{n}", self.boot), Box::new(item))
    }
}

impl Drop for MailStream {
    fn drop(&mut self) {
        let mut held = self.mailbox.lock();
        held.streams = held.streams.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_core::Value;

    fn item(event: &str) -> LiveBroadcastItem {
        LiveBroadcastItem::call_message(event.to_string(), Value::Null, "Feed".to_string(), Some("c1".to_string()))
    }

    fn frame(stream: &mut MailStream) -> Option<(String, String)> {
        match stream.poll(|_| true) {
            MailPoll::Frame(id, item) => Some((id, item.event)),
            _ => None,
        }
    }

    fn never(_: &str) -> bool {
        false
    }

    #[test]
    fn g_cross_853_a_frame_made_while_no_stream_is_open_is_sent_when_the_client_reopens() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        let mut first = mail.open("c1", None, || tx.subscribe(), never);
        let _ = tx.send(item("ONE"));
        let (id, event) = frame(&mut first).expect("the open stream carries it");
        assert_eq!(event, "ONE");
        drop(first);
        let _ = tx.send(item("TWO"));
        let mut again = mail.open("c1", Some(&id), || tx.subscribe(), never);
        assert_eq!(frame(&mut again).map(|f| f.1).as_deref(), Some("TWO"), "the frame made while closed arrives after the reopen");
        assert!(frame(&mut again).is_none(), "and the frame the client applied is not sent again");
    }

    #[test]
    fn a_frame_written_to_a_stream_that_was_gone_is_sent_again_after_the_frame_the_client_names() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        let mut first = mail.open("c1", None, || tx.subscribe(), never);
        let _ = tx.send(item("ONE"));
        let _ = tx.send(item("TWO"));
        let (applied, _) = frame(&mut first).expect("one");
        assert_eq!(frame(&mut first).map(|f| f.1).as_deref(), Some("TWO"), "written, never applied by the client");
        drop(first);
        let mut again = mail.open("c1", Some(&applied), || tx.subscribe(), never);
        assert_eq!(frame(&mut again).map(|f| f.1).as_deref(), Some("TWO"));
    }

    #[test]
    fn control_a_reopen_naming_no_frame_starts_after_the_last_frame_sent() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        let mut first = mail.open("c1", None, || tx.subscribe(), never);
        let _ = tx.send(item("ONE"));
        assert!(frame(&mut first).is_some());
        drop(first);
        let _ = tx.send(item("TWO"));
        let mut again = mail.open("c1", None, || tx.subscribe(), never);
        assert_eq!(frame(&mut again).map(|f| f.1).as_deref(), Some("TWO"));
        assert!(frame(&mut again).is_none());
    }

    #[test]
    fn edge_a_frame_id_of_another_boot_resumes_like_no_id() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        let mut first = mail.open("c1", None, || tx.subscribe(), never);
        let _ = tx.send(item("ONE"));
        assert!(frame(&mut first).is_some());
        drop(first);
        let _ = tx.send(item("TWO"));
        let mut again = mail.open("c1", Some("otherboot-7"), || tx.subscribe(), never);
        assert_eq!(frame(&mut again).map(|f| f.1).as_deref(), Some("TWO"));
    }

    #[test]
    fn edge_an_item_the_filter_refuses_takes_no_number() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        let mut stream = mail.open("c1", None, || tx.subscribe(), never);
        let _ = tx.send(item("HIDDEN"));
        let _ = tx.send(item("SHOWN"));
        let MailPoll::Frame(id, shown) = stream.poll(|item| item.event == "SHOWN") else { panic!("a frame") };
        assert_eq!(shown.event, "SHOWN");
        assert!(id.ends_with("-1"), "{id}");
    }

    #[test]
    fn edge_sent_frames_beyond_the_kept_window_are_dropped_and_unsent_ones_stay() {
        let (tx, _) = broadcast::channel(1024);
        let mail = LiveMailboxes::new(1024);
        let mut stream = mail.open("c1", None, || tx.subscribe(), never);
        for n in 0..(SENT_KEPT + 40) {
            let _ = tx.send(item(&format!("E{n}")));
            assert!(frame(&mut stream).is_some());
        }
        assert_eq!(stream.mailbox.lock().frames.len() as u64, SENT_KEPT);
    }

    #[test]
    fn a_mailbox_with_no_stream_goes_once_its_session_ended_and_stays_while_it_lives() {
        let (tx, _) = broadcast::channel(16);
        let mail = LiveMailboxes::new(16);
        drop(mail.open("gone", None, || tx.subscribe(), never));
        drop(mail.open("alive", None, || tx.subscribe(), never));
        let _open = mail.open("c1", None, || tx.subscribe(), |session| session == "gone");
        let boxes = mail.boxes.lock();
        assert!(!boxes.contains_key("gone"));
        assert!(boxes.contains_key("alive"), "control: a live session keeps its frames");
    }
}
