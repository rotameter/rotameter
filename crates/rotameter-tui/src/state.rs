#![forbid(unsafe_code)]
//! Dashboard state, breakdown tabs, and typed query builder.

use rotameter_client::types::{
    Dimension, Granularity, Metric, QueryFilter, QueryRequest, QueryResponse, RangePreset,
    RealtimeResponse, SiteRef, TimeseriesRow, WireTimeRange,
};

/// Selectable breakdown panel tabs in the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakdownTab {
    /// Path (`Dimension::Path`).
    Pages,
    /// Referrer source host (`Dimension::Source`).
    Sources,
    /// Country (`Dimension::Country`).
    Geo,
    /// Browser (`Dimension::Browser`).
    Tech,
    /// Event name (`Dimension::EventName`).
    Events,
}

impl BreakdownTab {
    /// Ordered list of all tabs in display sequence.
    pub const ALL: [Self; 5] = [
        Self::Pages,
        Self::Sources,
        Self::Geo,
        Self::Tech,
        Self::Events,
    ];

    /// Short uppercase tab label shown in the tab strip.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pages => "PAGES",
            Self::Sources => "SOURCES",
            Self::Geo => "GEO",
            Self::Tech => "TECH",
            Self::Events => "EVENTS",
        }
    }

    /// Underlying analytics breakdown dimension for this tab.
    pub const fn dimension(self) -> Dimension {
        match self {
            Self::Pages => Dimension::Path,
            Self::Sources => Dimension::Source,
            Self::Geo => Dimension::Country,
            Self::Tech => Dimension::Browser,
            Self::Events => Dimension::EventName,
        }
    }

    /// JSON row key name returned by `POST /v1/query` for this tab.
    pub const fn row_key(self) -> &'static str {
        match self {
            Self::Pages => "path",
            Self::Sources => "source",
            Self::Geo => "country",
            Self::Tech => "browser",
            Self::Events => "event_name",
        }
    }

    /// Advances to the next tab cyclically.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Pages => Self::Sources,
            Self::Sources => Self::Geo,
            Self::Geo => Self::Tech,
            Self::Tech => Self::Events,
            Self::Events => Self::Pages,
        }
    }

    /// Moves to the previous tab cyclically.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Pages => Self::Events,
            Self::Sources => Self::Pages,
            Self::Geo => Self::Sources,
            Self::Tech => Self::Geo,
            Self::Events => Self::Tech,
        }
    }
}

/// Complete interactive TUI state.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    /// Accessible site slugs for the site switcher overlay.
    pub sites: Vec<String>,
    /// Currently selected site reference.
    pub active_site: SiteRef,
    /// Selected trailing time window preset.
    pub range: RangePreset,
    /// Active breakdown panel tab.
    pub tab: BreakdownTab,
    /// Stack of active `Eq` drill-down filters.
    pub filters: Vec<QueryFilter>,
    /// Currently scrubbed bucket index in `timeseries`.
    pub scrub_idx: Option<usize>,
    /// Highlighted row index in the breakdown table.
    pub selected_row: usize,
    /// Whether the site switcher overlay is visible.
    pub site_switcher_open: bool,
    /// Cursor index inside the site switcher overlay.
    pub site_picker_idx: usize,
    /// Whether the keybinding help overlay is visible.
    pub help_open: bool,
    /// Whether a background HTTP fetch is currently in flight.
    pub loading: bool,
    /// Last non-fatal background status or error message.
    pub status_msg: Option<String>,
    /// Set to exit the event loop cleanly.
    pub should_quit: bool,
    /// Latest analytics query response.
    pub query: Option<QueryResponse>,
    /// Latest realtime response.
    pub realtime: Option<RealtimeResponse>,
}

impl AppState {
    /// Initializes dashboard state targeting `active_site` with `sites` choices.
    pub fn new(active_site: SiteRef, sites: Vec<String>) -> Self {
        Self {
            sites,
            active_site,
            range: RangePreset::Days30,
            tab: BreakdownTab::Pages,
            filters: Vec::new(),
            scrub_idx: None,
            selected_row: 0,
            site_switcher_open: false,
            site_picker_idx: 0,
            help_open: false,
            loading: true,
            status_msg: None,
            should_quit: false,
            query: None,
            realtime: None,
        }
    }

    /// Builds a typed `POST /v1/query` payload matching current window, tab, and filters.
    pub fn build_query_request(&self) -> QueryRequest {
        let granularity = match self.range {
            RangePreset::Hours24 => Granularity::Hour,
            RangePreset::Months12 | RangePreset::All => Granularity::Month,
            RangePreset::Days7 | RangePreset::Days30 => Granularity::Day,
        };
        let metrics = vec![
            Metric::Visitors,
            Metric::Pageviews,
            Metric::Visits,
            Metric::ViewsPerVisit,
            Metric::BounceRate,
            Metric::AvgVisitDurationMs,
        ];
        let range = WireTimeRange::Preset { preset: self.range };
        let mut req = QueryRequest::new(self.active_site.clone(), range, metrics);
        req.granularity = Some(granularity);
        req.group_by = vec![self.tab.dimension()];
        req.filters.clone_from(&self.filters);
        req.compare_previous = true;
        req.limit = 20;
        req
    }

    /// Applies fresh query and realtime responses while clamping cursor indices safely.
    pub fn apply_snapshot(&mut self, query: QueryResponse, realtime: RealtimeResponse) {
        let ts_len = query.timeseries.len();
        self.scrub_idx = self
            .scrub_idx
            .and_then(|i| (ts_len > 0).then(|| i.min(ts_len - 1)));
        self.selected_row = self.selected_row.min(query.rows.len().saturating_sub(1));
        self.query = Some(query);
        self.realtime = Some(realtime);
        self.loading = false;
        self.status_msg = None;
    }

    /// Returns the bucket highlighted by the scrubber (or latest bucket when idle).
    pub fn active_bucket(&self) -> Option<&TimeseriesRow> {
        let ts = &self.query.as_ref()?.timeseries;
        self.scrub_idx.and_then(|i| ts.get(i)).or_else(|| ts.last())
    }
}
