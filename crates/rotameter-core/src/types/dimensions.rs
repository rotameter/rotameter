#![forbid(unsafe_code)]
//! Device, viewport, channel, and dictionary dimension enumerations.

use serde::{Deserialize, Serialize};

/// Coarse hardware form factor derived from User-Agent classification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum DeviceType {
    /// Unrecognized device class (`0`).
    #[default]
    Unknown = 0,
    /// Desktop or laptop (`1`).
    Desktop = 1,
    /// Handheld mobile phone (`2`).
    Mobile = 2,
    /// Tablet device (`3`).
    Tablet = 3,
}

impl DeviceType {
    /// Returns the compact integer stored in `sessions.device_type`.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a compact integer stored in `sessions.device_type` or Arrow exchange columns.
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Desktop,
            2 => Self::Mobile,
            3 => Self::Tablet,
            _ => Self::Unknown,
        }
    }
}

/// Coarse Tailwind-aligned viewport bucket computed from reported viewport width.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ViewportClass {
    /// Width omitted or zero (`0`).
    #[default]
    Unknown = 0,
    /// `< 640px` (`1`).
    Xs = 1,
    /// `640..768px` (`2`).
    Sm = 2,
    /// `768..1024px` (`3`).
    Md = 3,
    /// `1024..1280px` (`4`).
    Lg = 4,
    /// `>= 1280px` (`5`).
    Xl = 5,
}

impl ViewportClass {
    /// Maps an optional viewport width in CSS pixels to a compact [`ViewportClass`].
    pub const fn from_width(width: Option<u16>) -> Self {
        match width {
            None | Some(0) => Self::Unknown,
            Some(w) if w < 640 => Self::Xs,
            Some(w) if w < 768 => Self::Sm,
            Some(w) if w < 1024 => Self::Md,
            Some(w) if w < 1280 => Self::Lg,
            Some(_) => Self::Xl,
        }
    }

    /// Returns the compact integer stored in `sessions.viewport_class`.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Decodes a compact integer stored in `sessions.viewport_class` or Arrow exchange columns.
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Xs,
            2 => Self::Sm,
            3 => Self::Md,
            4 => Self::Lg,
            5 => Self::Xl,
            _ => Self::Unknown,
        }
    }
}

/// Marketing / acquisition channel classification (`SPEC-PRIVACY.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Direct or self-referred visit.
    Direct,
    /// Search engine referral without paid medium.
    OrganicSearch,
    /// Social network referral without paid medium.
    OrganicSocial,
    /// General external website referral.
    Referral,
    /// Explicit paid advertising medium (`cpc`, `ppc`, `paid`, `display`, etc.).
    PaidCampaign,
    /// Email or newsletter campaign traffic.
    Email,
}

impl Channel {
    /// Returns the human-readable canonical channel label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "Direct",
            Self::OrganicSearch => "Organic Search",
            Self::OrganicSocial => "Organic Social",
            Self::Referral => "Referral",
            Self::PaidCampaign => "Paid / Campaign",
            Self::Email => "Email",
        }
    }

    /// Parses a canonical display label or snake-case channel identifier.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "direct" => Some(Self::Direct),
            "organic search" | "organic_search" => Some(Self::OrganicSearch),
            "organic social" | "organic_social" => Some(Self::OrganicSocial),
            "referral" => Some(Self::Referral),
            "paid / campaign" | "paid_campaign" => Some(Self::PaidCampaign),
            "email" => Some(Self::Email),
            _ => None,
        }
    }
}

/// Dimension dictionary kind stored in site-scoped `dim_strings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DimensionKind {
    /// Normalized URL path (`1`).
    Path = 1,
    /// Normalized referrer source/host (`2`).
    RefHost = 2,
    /// Capped referrer URL (`3`).
    RefUrl = 3,
    /// Event name (`4`).
    EventName = 4,
    /// UTM attribute value (`5`).
    Utm = 5,
    /// Browser name + major version (`6`).
    UaBrowser = 6,
    /// Operating system + major version (`7`).
    UaOs = 7,
    /// ISO 3166-1 alpha-2 country code (`8`).
    GeoCountry = 8,
    /// Subdivision / region code (`9`).
    GeoRegion = 9,
    /// Acquisition channel label (`10`).
    Channel = 10,
}
