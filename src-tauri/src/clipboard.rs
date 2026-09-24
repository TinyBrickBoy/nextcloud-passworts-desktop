//! Zwischenablage mit automatischem Leeren.
//!
//! Unter Linux (X11) gehört der Inhalt dem Prozess, der ihn gesetzt hat, und verschwindet,
//! sobald das `arboard::Clipboard` Objekt freigegeben wird. Deshalb lebt es in einem eigenen Thread.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::time::Duration;

use zeroize::Zeroizing;

enum Command {
    Set(Zeroizing<String>),
    ClearIf(Zeroizing<String>),
}

pub struct Clipboard {
    tx: Sender<Command>,
    /// Sekunden bis zum Leeren, 0 = nie.
    clear_after: Arc<AtomicU32>,
}

impl Clipboard {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        std::thread::spawn(move || {
            let mut clipboard = arboard::Clipboard::new().ok();
            for command in rx {
                let Some(cb) = clipboard.as_mut() else {
                    clipboard = arboard::Clipboard::new().ok();
                    continue;
                };
                match command {
                    Command::Set(text) => {
                        let _ = cb.set_text(text.as_str());
                    }
                    Command::ClearIf(text) => {
                        // Nur leeren, wenn der Benutzer inzwischen nichts anderes kopiert hat.
                        if cb.get_text().map(|t| t == *text).unwrap_or(false) {
                            let _ = cb.clear();
                        }
                    }
                }
            }
        });
        Self { tx, clear_after: Arc::new(AtomicU32::new(30)) }
    }

    pub fn set_clear_after(&self, seconds: u32) {
        self.clear_after.store(seconds, Ordering::Relaxed);
    }

    pub fn copy(&self, text: String) {
        let text = Zeroizing::new(text);
        let _ = self.tx.send(Command::Set(text.clone()));
        let seconds = self.clear_after.load(Ordering::Relaxed);
        if seconds == 0 {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds as u64));
            let _ = tx.send(Command::ClearIf(text));
        });
    }
}
