use anyhow::Result;
use tokio::signal::unix::{signal, Signal as UnixSignal, SignalKind};
use tracing::debug;

/// Signals that the daemon can receive
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// SIGTERM or SIGINT - graceful shutdown
    Terminate,
    /// SIGHUP - reload configuration
    Reload,
}

/// Handler for Unix signals
pub struct SignalHandler {
    sigterm: UnixSignal,
    sigint: UnixSignal,
    sighup: UnixSignal,
}

impl SignalHandler {
    /// Create a new signal handler
    pub fn new() -> Result<Self> {
        let sigterm = signal(SignalKind::terminate())?;
        let sigint = signal(SignalKind::interrupt())?;
        let sighup = signal(SignalKind::hangup())?;

        debug!("Signal handlers registered");

        Ok(Self {
            sigterm,
            sigint,
            sighup,
        })
    }

    /// Wait for the next signal
    pub async fn recv(&mut self) -> Option<Signal> {
        tokio::select! {
            _ = self.sigterm.recv() => {
                debug!("Received SIGTERM");
                Some(Signal::Terminate)
            }
            _ = self.sigint.recv() => {
                debug!("Received SIGINT");
                Some(Signal::Terminate)
            }
            _ = self.sighup.recv() => {
                debug!("Received SIGHUP");
                Some(Signal::Reload)
            }
        }
    }
}
