//! Lokaler Passwort Generator mit dem Zufallsgenerator des Betriebssystems.

use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use serde::Deserialize;

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!#$%&()*+,-./:;<=>?@[]^_{|}~";
/// Zeichen, die sich leicht verwechseln lassen.
const AMBIGUOUS: &str = "Il1O0o|";

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { length: 20, lowercase: true, uppercase: true, digits: true, symbols: true, avoid_ambiguous: false }
    }
}

pub fn generate(options: &Options) -> String {
    let length = options.length.clamp(4, 128);
    let mut classes: Vec<Vec<char>> = [
        (options.lowercase, LOWER),
        (options.uppercase, UPPER),
        (options.digits, DIGITS),
        (options.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, chars)| chars.chars().filter(|c| !options.avoid_ambiguous || !AMBIGUOUS.contains(*c)).collect())
    .collect();
    if classes.is_empty() {
        classes.push(LOWER.chars().collect());
    }

    let rng = &mut OsRng;
    // Aus jeder gewählten Gruppe mindestens ein Zeichen, der Rest aus allen Gruppen.
    let mut out: Vec<char> = classes.iter().map(|class| *class.choose(rng).unwrap()).collect();
    let all: Vec<char> = classes.concat();
    while out.len() < length {
        out.push(*all.choose(rng).unwrap());
    }
    out.truncate(length);
    out.shuffle(rng);
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_length_and_classes() {
        for _ in 0..200 {
            let pw = generate(&Options::default());
            assert_eq!(pw.chars().count(), 20);
            assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
            assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
            assert!(pw.chars().any(|c| c.is_ascii_digit()));
            assert!(pw.chars().any(|c| SYMBOLS.contains(c)));
        }
    }

    #[test]
    fn only_digits_without_ambiguous() {
        let options = Options { length: 12, lowercase: false, uppercase: false, digits: true, symbols: false, avoid_ambiguous: true };
        let pw = generate(&options);
        assert_eq!(pw.len(), 12);
        assert!(pw.chars().all(|c| c.is_ascii_digit() && c != '0' && c != '1'));
    }

    #[test]
    fn falls_back_when_nothing_selected() {
        let options = Options { length: 8, lowercase: false, uppercase: false, digits: false, symbols: false, avoid_ambiguous: false };
        assert!(generate(&options).chars().all(|c| c.is_ascii_lowercase()));
    }
}
