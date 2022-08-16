use std::time::{Duration, SystemTime};

use anyhow::Context;
use chrono::prelude::*;
use sha2::{Digest, Sha256};
use strum::IntoEnumIterator;
use tracing::debug;
use worker::Delay;

use crate::platform::Platform;

pub fn sha256_string(input: &str) -> String {
    let result = Sha256::digest(input.as_bytes());
    base16ct::lower::encode_string(&result)
}

/// Asynchronously waits for the specified number of milliseconds.
pub async fn delay(milliseconds: u64) {
    debug!("waiting {milliseconds} milliseconds");

    Delay::from(Duration::from_millis(milliseconds)).await;

    debug!("done waiting {milliseconds} milliseconds");
}

pub fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(worker::Date::now().as_millis())
}

pub fn platforms_order(time: NaiveTime) -> anyhow::Result<Vec<Platform>> {
    let platforms = Platform::iter().collect::<Vec<_>>();

    let index = (time.minute() / 10)
        .try_into()
        .context("should be able to convert to usize")?;

    let platforms = permute::permutations_of(&platforms)
        .nth(index)
        .context("there should be >= 6 permutations")?
        .copied()
        .collect::<Vec<_>>();

    Ok(platforms)
}

pub fn escape_html(string: &str) -> String {
    askama_escape::escape(string, askama_escape::Html).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq;
    use test_case::test_case;

    use super::*;

    #[test]
    fn platforms_order_len() {
        let mut set = HashSet::new();

        for minute in 0..=59 {
            set.insert(platforms_order(NaiveTime::from_hms(0, minute, 0)).unwrap());
        }

        assert_eq!(set.len(), 6);
    }

    #[test_case(
        "Test commit & message <HtmlTag/>'s \"continuation\"",
        "Test commit &amp; message &lt;HtmlTag/&gt;&#x27;s &quot;continuation&quot;";
        "basic"
    )]
    fn escape_html_ok(input: &str, output: &str) {
        assert_eq!(escape_html(input), output);
    }
}
