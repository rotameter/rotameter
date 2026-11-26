#![forbid(unsafe_code)]
//! Direct column bindings and parameterized `DuckDB` predicate planning for `QueryFilter`.

use duckdb::types::Value;
use rotameter_core::error::CoreError;
use rotameter_core::query::{Dimension, FilterOp, FilterValue, QueryFilter};

pub(crate) struct DuckDimBinding {
    pub(crate) key: &'static str,
    pub(crate) expr: &'static str,
    pub(crate) needs_events: bool,
}

pub(crate) const fn dim_binding(dim: Dimension) -> DuckDimBinding {
    match dim {
        Dimension::Path => DuckDimBinding {
            key: "path",
            expr: "e.path",
            needs_events: true,
        },
        Dimension::EntryPage => DuckDimBinding {
            key: "entry_page",
            expr: "s.entry_path",
            needs_events: false,
        },
        Dimension::ExitPage => DuckDimBinding {
            key: "exit_page",
            expr: "s.exit_path",
            needs_events: false,
        },
        Dimension::Source => DuckDimBinding {
            key: "source",
            expr: "s.ref_host",
            needs_events: false,
        },
        Dimension::Referrer => DuckDimBinding {
            key: "referrer",
            expr: "s.ref_host",
            needs_events: false,
        },
        Dimension::Channel => DuckDimBinding {
            key: "channel",
            expr: "s.channel",
            needs_events: false,
        },
        Dimension::Country => DuckDimBinding {
            key: "country",
            expr: "s.country",
            needs_events: false,
        },
        Dimension::Region => DuckDimBinding {
            key: "region",
            expr: "s.region",
            needs_events: false,
        },
        Dimension::Browser => DuckDimBinding {
            key: "browser",
            expr: "s.browser",
            needs_events: false,
        },
        Dimension::Os => DuckDimBinding {
            key: "os",
            expr: "s.os",
            needs_events: false,
        },
        Dimension::Device => DuckDimBinding {
            key: "device",
            expr: "CASE s.device_type WHEN 1 THEN 'desktop' WHEN 2 THEN 'mobile' WHEN 3 THEN 'tablet' ELSE 'unknown' END",
            needs_events: false,
        },
        Dimension::Viewport => DuckDimBinding {
            key: "viewport",
            expr: "CASE s.viewport_class WHEN 1 THEN 'xs' WHEN 2 THEN 'sm' WHEN 3 THEN 'md' WHEN 4 THEN 'lg' WHEN 5 THEN 'xl' ELSE 'unknown' END",
            needs_events: false,
        },
        Dimension::EventName => DuckDimBinding {
            key: "event_name",
            expr: "e.event_kind",
            needs_events: true,
        },
        Dimension::UtmSource => DuckDimBinding {
            key: "utm_source",
            expr: "COALESCE(s.utm_source, '')",
            needs_events: false,
        },
        Dimension::UtmMedium => DuckDimBinding {
            key: "utm_medium",
            expr: "COALESCE(s.utm_medium, '')",
            needs_events: false,
        },
        Dimension::UtmCampaign => DuckDimBinding {
            key: "utm_campaign",
            expr: "COALESCE(s.utm_campaign, '')",
            needs_events: false,
        },
    }
}

pub(crate) struct DuckFilterPlan {
    clauses: Vec<String>,
    pub(crate) needs_events: bool,
}

impl DuckFilterPlan {
    pub(crate) fn where_sql(&self) -> String {
        let mut out = String::new();
        for clause in &self.clauses {
            out.push_str(" AND ");
            out.push_str(clause);
        }
        out
    }
}

pub(crate) fn build_filter_plan(
    filters: &[QueryFilter],
    group_by: &[Dimension],
    params: &mut Vec<Value>,
) -> Result<DuckFilterPlan, CoreError> {
    let mut plan = DuckFilterPlan {
        clauses: Vec::new(),
        needs_events: false,
    };
    for &dim in group_by {
        if dim_binding(dim).needs_events {
            plan.needs_events = true;
        }
    }
    for filter in filters {
        let binding = dim_binding(filter.dimension);
        if binding.needs_events {
            plan.needs_events = true;
        }
        plan.clauses.push(build_predicate(
            binding.expr,
            filter.op,
            &filter.value,
            params,
        )?);
    }
    Ok(plan)
}

fn build_predicate(
    expr: &str,
    op: FilterOp,
    value: &FilterValue,
    params: &mut Vec<Value>,
) -> Result<String, CoreError> {
    match (op, value) {
        (FilterOp::Eq, FilterValue::Scalar(s)) => {
            params.push(Value::Text(s.clone()));
            Ok(format!("{expr} = ?{}", params.len()))
        }
        (FilterOp::Neq, FilterValue::Scalar(s)) => {
            params.push(Value::Text(s.clone()));
            Ok(format!("{expr} != ?{}", params.len()))
        }
        (FilterOp::In, FilterValue::List(items)) if !items.is_empty() => {
            let mut placeholders = Vec::with_capacity(items.len());
            for item in items {
                params.push(Value::Text(item.clone()));
                placeholders.push(format!("?{}", params.len()));
            }
            Ok(format!("{expr} IN ({})", placeholders.join(", ")))
        }
        (FilterOp::Contains, FilterValue::Scalar(s)) => {
            params.push(Value::Text(format!("%{}%", escape_like(s))));
            Ok(format!("{expr} LIKE ?{} ESCAPE '\\'", params.len()))
        }
        (FilterOp::Prefix, FilterValue::Scalar(s)) => {
            params.push(Value::Text(format!("{}%", escape_like(s))));
            Ok(format!("{expr} LIKE ?{} ESCAPE '\\'", params.len()))
        }
        _ => Err(CoreError::InvalidQuery(
            "filter operator does not match operand value".to_string(),
        )),
    }
}

fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
