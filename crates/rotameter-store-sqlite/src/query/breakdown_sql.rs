#![forbid(unsafe_code)]
//! Parameterized `SQL` builders for session-level and event-level breakdown queries.

pub(crate) fn build_session_breakdown_sql(
    dim_cols: &[(&'static str, &'static str, String)],
    joins: &str,
    where_tail: &str,
) -> String {
    let select_dims: Vec<String> = dim_cols
        .iter()
        .map(|(_, expr, alias)| format!("{expr} AS {alias}"))
        .collect();
    let group_aliases: Vec<&str> = dim_cols.iter().map(|(_, _, a)| a.as_str()).collect();
    format!(
        "SELECT {dims},
                COUNT(DISTINCT s.visitor_id),
                COALESCE(SUM(s.pageview_count), 0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN s.event_count <= 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(MAX(0, s.last_event_ms - s.started_at_ms)), 0)
         FROM sessions s {joins}
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}
         GROUP BY {groups}",
        dims = select_dims.join(", "),
        groups = group_aliases.join(", "),
    )
}

pub(crate) fn build_event_breakdown_sql(
    dim_cols: &[(&'static str, &'static str, String)],
    joins: &str,
    where_tail: &str,
) -> String {
    let select_dims: Vec<String> = dim_cols
        .iter()
        .map(|(_, expr, alias)| format!("{expr} AS {alias}"))
        .collect();
    let group_aliases: Vec<&str> = dim_cols.iter().map(|(_, _, a)| a.as_str()).collect();
    let alias_csv = group_aliases.join(", ");
    format!(
        "WITH matched AS (
             SELECT {dims},
                    e.visitor_id AS visitor_id,
                    e.session_id AS session_id,
                    CASE WHEN en_pv.value = 'pageview' THEN 1 ELSE 0 END AS is_pv,
                    s.event_count AS event_count,
                    MAX(0, s.last_event_ms - s.started_at_ms) AS dur_ms
             FROM events e
             JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
             JOIN dim_strings en_pv ON en_pv.id = e.event_name_id
             {joins}
             WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}
         ),
         sess_agg AS (
             SELECT {alias_csv},
                    session_id,
                    visitor_id,
                    SUM(is_pv) AS pv_cnt,
                    MAX(CASE WHEN event_count <= 1 THEN 1 ELSE 0 END) AS is_bounce,
                    MAX(dur_ms) AS dur_ms
             FROM matched
             GROUP BY {alias_csv}, session_id
         )
         SELECT {alias_csv},
                COUNT(DISTINCT visitor_id),
                COALESCE(SUM(pv_cnt), 0),
                COUNT(*),
                COALESCE(SUM(is_bounce), 0),
                COALESCE(SUM(dur_ms), 0)
         FROM sess_agg
         GROUP BY {alias_csv}",
        dims = select_dims.join(", "),
    )
}
