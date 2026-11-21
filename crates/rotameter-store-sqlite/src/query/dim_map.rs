#![forbid(unsafe_code)]
//! Mapping from [`Dimension`] variants to `SQLite` column/join bindings and JSON keys.

use rotameter_core::query::Dimension;

pub(crate) struct DimBinding {
    pub(crate) key: &'static str,
    pub(crate) expr: &'static str,
    pub(crate) join: Option<(&'static str, &'static str)>,
    pub(crate) needs_events: bool,
}

pub(crate) fn dimension_key_name(dim: Dimension) -> &'static str {
    dim_binding(dim).key
}

pub(crate) const fn dim_binding(dim: Dimension) -> DimBinding {
    match dim {
        Dimension::Path => DimBinding {
            key: "path",
            expr: "d_path.value",
            join: Some(("d_path", "e.path_id")),
            needs_events: true,
        },
        Dimension::EntryPage => DimBinding {
            key: "entry_page",
            expr: "d_entry.value",
            join: Some(("d_entry", "s.entry_path_id")),
            needs_events: false,
        },
        Dimension::ExitPage => DimBinding {
            key: "exit_page",
            expr: "d_exit.value",
            join: Some(("d_exit", "s.exit_path_id")),
            needs_events: false,
        },
        Dimension::Source => DimBinding {
            key: "source",
            expr: "d_ref.value",
            join: Some(("d_ref", "s.ref_host_id")),
            needs_events: false,
        },
        Dimension::Referrer => DimBinding {
            key: "referrer",
            expr: "d_ref.value",
            join: Some(("d_ref", "s.ref_host_id")),
            needs_events: false,
        },
        Dimension::Channel => DimBinding {
            key: "channel",
            expr: "d_chan.value",
            join: Some(("d_chan", "s.channel_id")),
            needs_events: false,
        },
        Dimension::Country => DimBinding {
            key: "country",
            expr: "d_cty.value",
            join: Some(("d_cty", "s.country_id")),
            needs_events: false,
        },
        Dimension::Region => DimBinding {
            key: "region",
            expr: "d_reg.value",
            join: Some(("d_reg", "s.region_id")),
            needs_events: false,
        },
        Dimension::Browser => DimBinding {
            key: "browser",
            expr: "d_br.value",
            join: Some(("d_br", "s.browser_id")),
            needs_events: false,
        },
        Dimension::Os => DimBinding {
            key: "os",
            expr: "d_os.value",
            join: Some(("d_os", "s.os_id")),
            needs_events: false,
        },
        Dimension::Device => DimBinding {
            key: "device",
            expr: "CASE s.device_type WHEN 1 THEN 'desktop' WHEN 2 THEN 'mobile' WHEN 3 THEN 'tablet' ELSE 'unknown' END",
            join: None,
            needs_events: false,
        },
        Dimension::Viewport => DimBinding {
            key: "viewport",
            expr: "CASE s.viewport_class WHEN 1 THEN 'xs' WHEN 2 THEN 'sm' WHEN 3 THEN 'md' WHEN 4 THEN 'lg' WHEN 5 THEN 'xl' ELSE 'unknown' END",
            join: None,
            needs_events: false,
        },
        Dimension::EventName => DimBinding {
            key: "event_name",
            expr: "d_ev.value",
            join: Some(("d_ev", "e.event_name_id")),
            needs_events: true,
        },
        Dimension::UtmSource => DimBinding {
            key: "utm_source",
            expr: "d_us.value",
            join: Some(("d_us", "s.utm_source_id")),
            needs_events: false,
        },
        Dimension::UtmMedium => DimBinding {
            key: "utm_medium",
            expr: "d_um.value",
            join: Some(("d_um", "s.utm_medium_id")),
            needs_events: false,
        },
        Dimension::UtmCampaign => DimBinding {
            key: "utm_campaign",
            expr: "d_uc.value",
            join: Some(("d_uc", "s.utm_campaign_id")),
            needs_events: false,
        },
    }
}
