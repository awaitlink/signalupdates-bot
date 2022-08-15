use std::{
    io,
    sync::{mpsc, Mutex},
};

use tracing::{debug, metadata::LevelFilter};
use tracing_subscriber::prelude::*;

struct MpscWriter {
    sender: mpsc::Sender<String>,
}

impl MpscWriter {
    fn new(sender: mpsc::Sender<String>) -> Self {
        Self { sender }
    }
}

impl io::Write for MpscWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sender
            .send(String::from_utf8_lossy(buf).to_string())
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn configure() -> (mpsc::Receiver<String>, impl tracing::Subscriber) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(LevelFilter::TRACE)
        .without_time()
        .with_ansi(false);

    let writer = move || MpscWriter::new(tx.lock().expect("should be able to lock mutex").clone());

    #[cfg(not(target_family = "wasm"))]
    let subscriber = subscriber.with_writer(io::stderr.and(writer));
    #[cfg(target_family = "wasm")]
    let subscriber = subscriber.with_writer(writer);

    let subscriber = subscriber.finish();

    #[cfg(target_family = "wasm")]
    let subscriber = subscriber.with(tracing_wasm::WASMLayer::new(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_console_config(tracing_wasm::ConsoleConfig::ReportWithoutConsoleColor)
            .set_max_level(tracing::Level::TRACE)
            .set_report_logs_in_timings(false)
            .build(),
    ));

    (rx, subscriber)
}

pub fn recv_log(rx: mpsc::Receiver<String>) -> String {
    let mut log = Vec::new();
    while let Ok(message) = rx.try_recv() {
        log.push(message);
    }

    log.join("")
}

pub fn separator() {
    debug!("----------------------------------------------------------------------");
}
