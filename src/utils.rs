use std::time::{Duration, SystemTime};

use anyhow::Context;
use chrono::prelude::*;
use factorial::Factorial;
use sha2::{Digest, Sha256};
use worker::Delay;

use crate::platform::Platform;

pub fn sha256_string(input: &str) -> String {
    let result = Sha256::digest(input.as_bytes());
    base16ct::lower::encode_string(&result)
}

/// Asynchronously waits for the specified number of milliseconds.
pub async fn delay(milliseconds: u64) {
    tracing::trace!(milliseconds, "waiting");
    Delay::from(Duration::from_millis(milliseconds)).await;
    tracing::trace!(milliseconds, "done waiting");
}

pub fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(worker::Date::now().as_millis())
}

pub fn platforms_order(
    all_platforms: &[Platform],
    time: NaiveTime,
) -> anyhow::Result<Vec<Platform>> {
    if all_platforms.is_empty() {
        return Ok(vec![]);
    }

    let permutation_count = all_platforms.len().factorial();

    let index: usize = (time.minute() / 10)
        .try_into()
        .context("should be able to convert to usize")?;

    let index = index % permutation_count;

    let platforms = permute::permutations_of(all_platforms)
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
    use strum::IntoEnumIterator;
    use test_case::test_case;

    use super::*;

    #[test_case(&Platform::iter().collect::<Vec<Platform>>(), 6; "all")]
    #[test_case(&[Platform::Android, Platform::Ios], 2; "two")]
    #[test_case(&[Platform::Android], 1; "one")]
    #[test_case(&[], 1; "none")]
    fn platforms_order_len(all_platforms: &[Platform], result_len: usize) {
        let mut set = HashSet::new();

        for minute in 0..=59 {
            set.insert(
                platforms_order(
                    all_platforms,
                    NaiveTime::from_hms_opt(0, minute, 0).unwrap(),
                )
                .unwrap(),
            );
        }

        assert_eq!(set.len(), result_len);
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
