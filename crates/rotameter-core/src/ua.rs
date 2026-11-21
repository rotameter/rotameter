#![forbid(unsafe_code)]
//! Coarse User-Agent browser, OS, and device-type classifier (`SPEC-PRIVACY.md` §5).

use crate::normalize::truncate_utf8;
use crate::types::{DeviceType, MAX_USER_AGENT_BYTES};

/// Coarse non-identifying client attributes extracted from a `User-Agent` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedUa {
    /// Browser family and major version (e.g., `"Firefox 132"` or `"Other"`).
    pub browser: String,
    /// Operating system and major version (e.g., `"macOS 15"` or `"Other"`).
    pub os: String,
    /// Coarse device form factor.
    pub device_type: DeviceType,
}

/// Classifies a raw `User-Agent` string into coarse browser, OS, and device categories.
///
/// Inspects at most `512` bytes of `raw_ua`; the raw string should be discarded immediately by
/// callers after hashing and classification.
pub fn classify_user_agent(raw_ua: &str) -> ClassifiedUa {
    let ua = truncate_utf8(raw_ua.trim(), MAX_USER_AGENT_BYTES);
    if ua.is_empty() {
        return ClassifiedUa {
            browser: "Other".to_string(),
            os: "Other".to_string(),
            device_type: DeviceType::Unknown,
        };
    }

    let browser = detect_browser(ua);
    let os = detect_os(ua);
    let device_type = detect_device(ua, &os);

    ClassifiedUa {
        browser,
        os,
        device_type,
    }
}

fn detect_browser(ua: &str) -> String {
    if let Some(major) = token_major(ua, "Edg/")
        .or_else(|| token_major(ua, "EdgA/"))
        .or_else(|| token_major(ua, "EdgiOS/"))
    {
        return format!("Edge {major}");
    }
    if let Some(major) = token_major(ua, "OPR/").or_else(|| token_major(ua, "Opera/")) {
        return format!("Opera {major}");
    }
    if let Some(major) = token_major(ua, "SamsungBrowser/") {
        return format!("Samsung Internet {major}");
    }
    if let Some(major) = token_major(ua, "Vivaldi/") {
        return format!("Vivaldi {major}");
    }
    if let Some(major) = token_major(ua, "Firefox/").or_else(|| token_major(ua, "FxiOS/")) {
        return format!("Firefox {major}");
    }
    if let Some(major) = token_major(ua, "CriOS/").or_else(|| token_major(ua, "Chrome/")) {
        return format!("Chrome {major}");
    }
    if ua.contains("Safari/") {
        return match token_major(ua, "Version/") {
            Some(major) => format!("Safari {major}"),
            None => "Safari".to_string(),
        };
    }
    "Other".to_string()
}

fn detect_os(ua: &str) -> String {
    if ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod") {
        return match token_major(ua, "CPU iPhone OS ")
            .or_else(|| token_major(ua, "CPU OS "))
            .or_else(|| token_major(ua, "OS "))
        {
            Some(major) => format!("iOS {major}"),
            None => "iOS".to_string(),
        };
    }
    if ua.contains("Android") {
        return match token_major(ua, "Android ") {
            Some(major) => format!("Android {major}"),
            None => "Android".to_string(),
        };
    }
    if ua.contains("CrOS") {
        return "ChromeOS".to_string();
    }
    if ua.contains("Windows") {
        if ua.contains("Windows NT 10.0") {
            return "Windows 10/11".to_string();
        }
        return "Windows".to_string();
    }
    if ua.contains("Mac OS X") {
        return match token_major(ua, "Mac OS X ") {
            Some(major) => format!("macOS {major}"),
            None => "macOS".to_string(),
        };
    }
    if ua.contains("Linux") {
        return "Linux".to_string();
    }
    "Other".to_string()
}

fn detect_device(ua: &str, os: &str) -> DeviceType {
    if ua.contains("iPad")
        || ua.contains("Tablet")
        || (os.starts_with("Android") && !ua.contains("Mobile"))
    {
        return DeviceType::Tablet;
    }
    if ua.contains("iPhone")
        || ua.contains("iPod")
        || ua.contains("Mobile")
        || os.starts_with("Android")
    {
        return DeviceType::Mobile;
    }
    if os.starts_with("Windows") || os.starts_with("macOS") || os == "Linux" || os == "ChromeOS" {
        return DeviceType::Desktop;
    }
    DeviceType::Unknown
}

fn token_major<'a>(ua: &'a str, marker: &str) -> Option<&'a str> {
    let idx = ua.find(marker)?;
    let rest = &ua[idx + marker.len()..];
    let digits_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    (digits_len > 0).then(|| &rest[..digits_len])
}
