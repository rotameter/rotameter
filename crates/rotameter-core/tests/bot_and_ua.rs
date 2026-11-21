#![forbid(unsafe_code)]
//! Integration tests for bot/prefetch filtering and coarse User-Agent classification.

use rotameter_core::bot::{BotDropReason, BotFilterDecision, PrefetchHeaders, classify_traffic};
use rotameter_core::types::DeviceType;
use rotameter_core::ua::classify_user_agent;

#[test]
fn drops_prefetch_empty_and_known_bots_while_allowing_humans() {
    assert_eq!(
        classify_traffic(
            Some(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0) AppleWebKit/605.1.15 Safari/605.1.15"
            ),
            PrefetchHeaders {
                sec_purpose: Some("prefetch"),
                ..Default::default()
            },
            true,
            false
        ),
        BotFilterDecision::Drop(BotDropReason::Prefetch)
    );

    assert_eq!(
        classify_traffic(Some("   "), PrefetchHeaders::default(), true, false),
        BotFilterDecision::Drop(BotDropReason::MissingUserAgent)
    );
    assert_eq!(
        classify_traffic(None, PrefetchHeaders::default(), false, false),
        BotFilterDecision::Allow
    );

    for bot_ua in [
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "GPTBot/1.0 (+https://openai.com/gptbot)",
        "ClaudeBot/1.0",
        "curl/8.7.1",
        "Mozilla/5.0 HeadlessChrome/130.0.0.0",
    ] {
        assert_eq!(
            classify_traffic(Some(bot_ua), PrefetchHeaders::default(), true, false),
            BotFilterDecision::Drop(BotDropReason::KnownBot),
            "expected drop for {bot_ua}"
        );
    }

    assert_eq!(
        classify_traffic(Some("curl/8.7.1"), PrefetchHeaders::default(), true, true),
        BotFilterDecision::Allow
    );
    assert_eq!(
        classify_traffic(
            Some(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            ),
            PrefetchHeaders::default(),
            true,
            false
        ),
        BotFilterDecision::Allow
    );
}

#[test]
fn classifies_coarse_browser_os_and_device_safely() {
    let mac_chrome = classify_user_agent(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    );
    assert_eq!(mac_chrome.browser, "Chrome 131");
    assert_eq!(mac_chrome.os, "macOS 15");
    assert_eq!(mac_chrome.device_type, DeviceType::Desktop);

    let iphone = classify_user_agent(
        "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
    );
    assert_eq!(iphone.browser, "Safari 18");
    assert_eq!(iphone.os, "iOS 18");
    assert_eq!(iphone.device_type, DeviceType::Mobile);

    let ipad = classify_user_agent(
        "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/130.0.6723.90 Mobile/15E148 Safari/604.1",
    );
    assert_eq!(ipad.browser, "Chrome 130");
    assert_eq!(ipad.os, "iOS 17");
    assert_eq!(ipad.device_type, DeviceType::Tablet);

    let android_phone = classify_user_agent(
        "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Firefox/132.0 Mobile Safari/537.36",
    );
    assert_eq!(android_phone.browser, "Firefox 132");
    assert_eq!(android_phone.os, "Android 15");
    assert_eq!(android_phone.device_type, DeviceType::Mobile);

    let win_edge = classify_user_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.51",
    );
    assert_eq!(win_edge.browser, "Edge 131");
    assert_eq!(win_edge.os, "Windows 10/11");
    assert_eq!(win_edge.device_type, DeviceType::Desktop);

    let empty = classify_user_agent("");
    assert_eq!(empty.device_type, DeviceType::Unknown);
}
