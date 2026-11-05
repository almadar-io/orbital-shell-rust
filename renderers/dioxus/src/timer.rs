use std::time::Duration;
use web_time::Instant;

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub async fn animate(duration: Duration, mut frame: impl FnMut(f64) -> bool) {
    if duration.is_zero() {
        frame(1.0);
        return;
    }
    let started = Instant::now();
    loop {
        let progress = (started.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
        if !frame(progress) || progress >= 1.0 {
            break;
        }
        Delay::new(FRAME_INTERVAL).await;
    }
}

pub fn ease_out_cubic(progress: f64) -> f64 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

pub struct Delay;

impl Delay {
    pub async fn new(duration: Duration) {
        dioxus_sdk_time::sleep(duration).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_cubic_is_bounded_and_monotonic() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
        assert!(ease_out_cubic(0.25) < ease_out_cubic(0.5));
    }
}
