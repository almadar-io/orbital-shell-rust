use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct Delay(Duration, bool);

impl Delay {
    pub fn new(duration: Duration) -> Self {
        Self(duration, false)
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.1 {
            return Poll::Ready(());
        }
        self.1 = true;
        let waker = cx.waker().clone();
        let duration = self.0;
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            waker.wake();
        });
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::task::{Wake, Waker};

    struct TestWake(mpsc::Sender<()>);

    impl Wake for TestWake {
        fn wake(self: Arc<Self>) {
            self.0.send(()).unwrap();
        }
    }

    #[test]
    fn delay_wakes_once_after_its_clock_interval() {
        let (sender, receiver) = mpsc::channel();
        let waker = Waker::from(Arc::new(TestWake(sender)));
        let mut context = Context::from_waker(&waker);
        let mut delay = Box::pin(Delay::new(Duration::from_millis(2)));
        assert!(matches!(delay.as_mut().poll(&mut context), Poll::Pending));
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(delay.as_mut().poll(&mut context), Poll::Ready(())));
    }
}
