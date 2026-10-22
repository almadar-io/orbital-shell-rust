use super::Host;
use orbital_core::Value;
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub struct BackgroundServer {
    runtime: Arc<Host>,
    address: String,
    port: u16,
}

impl BackgroundServer {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_url(&self) -> String {
        let address = if self.address.contains(':') {
            format!("[{}]", self.address)
        } else {
            self.address.clone()
        };
        format!("http://{address}:{}", self.port)
    }

    pub fn runtime(&self) -> &Arc<Host> {
        &self.runtime
    }
}

impl Drop for BackgroundServer {
    fn drop(&mut self) {
        self.runtime.cancel_open_calls();
    }
}

fn listening(payload: &Value) -> anyhow::Result<(String, u16)> {
    let address = payload.get_path("host").and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("HTTP1_LISTENING has no host"))?;
    let port = payload.get_path("port").and_then(Value::as_number)
        .filter(|port| port.is_finite() && *port >= 1.0 && *port <= f64::from(u16::MAX) && port.fract() == 0.0)
        .ok_or_else(|| anyhow::anyhow!("HTTP1_LISTENING has no bound port"))?;
    Ok((address.to_string(), port as u16))
}

pub fn serve_background(runtime: Arc<Host>) -> anyhow::Result<BackgroundServer> {
    let (sender, receiver) = mpsc::channel();
    let ready = sender.clone();
    let bus = runtime.event_bus();
    let mut subscriptions = vec![bus.on("HTTP1_LISTENING", move |event| {
        let _ = ready.send(listening(&event.payload));
    })];
    for event in ["BOOT_REFUSED", "LISTEN_FAILED"] {
        let failed = sender.clone();
        subscriptions.push(bus.on(event, move |message| {
            let _ = failed.send(Err(anyhow::anyhow!("{event}: {}", message.payload)));
        }));
    }
    let started = super::run::mount_init(&runtime)
        .map_err(anyhow::Error::from)
        .and_then(|()| receiver.recv_timeout(Duration::from_secs(5))
            .map_err(|error| anyhow::anyhow!("waiting for HTTP1_LISTENING: {error}"))?);
    for subscription in subscriptions {
        bus.off(subscription);
    }
    match started {
        Ok((address, port)) => Ok(BackgroundServer { runtime, address, port }),
        Err(error) => {
            runtime.cancel_open_calls();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_uses_the_bound_address_and_validates_the_port() {
        let payload = |host: &str, port: f64| Value::Object([
            ("host".to_string(), Value::String(host.to_string())),
            ("port".to_string(), Value::Number(port)),
        ].into_iter().collect());
        assert_eq!(listening(&payload("::1", 1234.0)).unwrap(), ("::1".to_string(), 1234));
        for port in [0.0, -1.0, 1.5, 65536.0, f64::NAN] {
            assert!(listening(&payload("127.0.0.1", port)).is_err());
        }
    }
}
