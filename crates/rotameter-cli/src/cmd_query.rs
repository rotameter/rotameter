#![forbid(unsafe_code)]
//! `rotameter query` subcommand handler.

use rotameter_client::types::{
    Dimension, FilterOp, FilterValue, Granularity, Metric, QueryFilter, QueryRequest,
    QueryResponse, RangePreset, SiteRef, WireTimeRange,
};
use serde::de::DeserializeOwned;

use crate::cli::GlobalOpts;
use crate::cli_sub::QueryArgs;
use crate::context::remote_client;
use crate::exit::CliError;
use crate::format::{OutputFormat, render_json, render_table};

pub(crate) fn run_query(global: &GlobalOpts, args: &QueryArgs) -> Result<(), CliError> {
    let client = remote_client(global)?;
    let site_id = SiteRef::parse(&args.site)?;
    let range = match (args.from.as_deref(), args.to.as_deref()) {
        (Some(from), Some(to)) => WireTimeRange::Iso {
            from: from.to_string(),
            to: to.to_string(),
        },
        (None, None) => WireTimeRange::Preset {
            preset: parse_enum::<RangePreset>(&args.range, "range")?,
        },
        _ => {
            return Err(CliError::usage(
                "`--from` and `--to` must be provided together",
            ));
        }
    };
    let metrics = if args.metrics.is_empty() {
        vec![
            Metric::Visitors,
            Metric::Pageviews,
            Metric::Visits,
            Metric::ViewsPerVisit,
            Metric::BounceRate,
            Metric::AvgVisitDurationMs,
        ]
    } else {
        args.metrics
            .iter()
            .map(|m| parse_enum(m, "metric"))
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut req = QueryRequest::new(site_id, range, metrics);
    req.granularity = args
        .granularity
        .as_deref()
        .map(|g| parse_enum::<Granularity>(g, "granularity"))
        .transpose()?;
    req.group_by = args
        .group_by
        .iter()
        .map(|d| parse_enum::<Dimension>(d, "group-by"))
        .collect::<Result<Vec<_>, _>>()?;
    req.filters = args
        .filters
        .iter()
        .map(|f| parse_filter(f))
        .collect::<Result<Vec<_>, _>>()?;
    req.compare_previous = args.compare;
    req.limit = args.limit;
    req.offset = args.offset;

    if global.format == OutputFormat::Csv {
        let csv = client.query_csv(&req)?;
        print!("{csv}");
        return Ok(());
    }

    let res = client.query(&req)?;
    if global.format == OutputFormat::Json {
        print!("{}", render_json(&res)?);
        return Ok(());
    }
    print!("{}", render_query_table(&res));
    Ok(())
}

fn render_query_table(res: &QueryResponse) -> String {
    let summary_rows = vec![vec![
        res.summary.visitors.to_string(),
        res.summary.pageviews.to_string(),
        res.summary.visits.to_string(),
        format!("{:.2}", res.summary.views_per_visit),
        format!("{:.1}%", res.summary.bounce_rate * 100.0),
        format!("{}ms", res.summary.avg_visit_duration_ms),
    ]];
    let mut out = render_table(
        &[
            "VISITORS",
            "PAGEVIEWS",
            "VISITS",
            "VIEWS/VISIT",
            "BOUNCE",
            "AVG_DUR",
        ],
        &summary_rows,
    );
    if !res.rows.is_empty() {
        let breakdown: Vec<Vec<String>> = res
            .rows
            .iter()
            .map(|r| {
                let key = r
                    .key
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                vec![
                    key,
                    r.visitors.to_string(),
                    r.pageviews.to_string(),
                    r.visits.to_string(),
                ]
            })
            .collect();
        out.push('\n');
        out.push_str(&render_table(
            &["KEY", "VISITORS", "PAGEVIEWS", "VISITS"],
            &breakdown,
        ));
    }
    out
}

fn parse_enum<T: DeserializeOwned>(raw: &str, flag: &str) -> Result<T, CliError> {
    let norm = raw.trim().to_ascii_lowercase();
    serde_json::from_value(serde_json::Value::String(norm))
        .map_err(|_| CliError::usage(format!("invalid `--{flag}` value `{raw}`")))
}

fn parse_filter(raw: &str) -> Result<QueryFilter, CliError> {
    for (sep, op) in [
        ("!=", FilterOp::Neq),
        ("^=", FilterOp::Prefix),
        ("~=", FilterOp::Contains),
        ("=", FilterOp::Eq),
    ] {
        if let Some((dim_raw, val)) = raw.split_once(sep) {
            return Ok(QueryFilter {
                dimension: parse_enum(dim_raw, "filter")?,
                op,
                value: FilterValue::Scalar(val.trim().to_string()),
            });
        }
    }
    if let Some((dim_raw, list_raw)) = raw.split_once(" in ") {
        let items: Vec<String> = list_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if items.is_empty() {
            return Err(CliError::usage(format!(
                "empty `in` filter list in `{raw}`"
            )));
        }
        return Ok(QueryFilter {
            dimension: parse_enum(dim_raw, "filter")?,
            op: FilterOp::In,
            value: FilterValue::List(items),
        });
    }
    Err(CliError::usage(format!(
        "invalid `--filter` `{raw}`; expected dim=val, !=, ^=, ~=, or `dim in a,b`"
    )))
}
