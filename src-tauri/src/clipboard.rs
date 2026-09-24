//! Zwischenablage mit automatischem Leeren.
//!
//! Unter Linux (X11) gehört der Inhalt dem Prozess, der ihn gesetzt hat, und verschwindet,
//! sobald das `arboard::Clipboard` Objekt freigegeben wird. Deshalb lebt es in einem eigenen Thread.

use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use zeroize::Zeroizing;

pub const CLEAR_AFTER: Duration = Duration::from_secs(30);

enum Command {
    Set(Zeroizing<String>),
    ClearIf(Zeroizing<String>),
}

pub struct Clipboard {
    tx: Sender<Command>,
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
        Self { tx }
    }

    pub fn copy(&self, text: String) {
        let text = Zeroizing::new(text);
        let _ = self.tx.send(Command::Set(text.clone()));
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(CLEAR_AFTER);
            let _ = tx.send(Command::ClearIf(text));
        });
    }
}
