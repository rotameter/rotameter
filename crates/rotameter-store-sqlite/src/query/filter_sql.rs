#![forbid(unsafe_code)]
//! Parameterized `SQL` join planner and predicate builder for `QueryFilter`.

use rotameter_core::error::CoreError;
use rotameter_core::query::{Dimension, FilterOp, FilterValue, QueryFilter};
use rusqlite::types::Value;

use crate::query::dim_map::dim_binding;

pub(crate) struct SqlFilterPlan {
    joins: Vec<(&'static str, &'static str)>,
    clauses: Vec<String>,
    pub(crate) needs_events: bool,
}

impl SqlFilterPlan {
    pub(crate) fn ensure_dimension(&mut self, dim: Dimension) -> &'static str {
        let binding = dim_binding(dim);
        if binding.needs_events {
            self.needs_events = true;
        }
        if let Some(join) = binding.join
            && !self.joins.iter().any(|existing| existing.0 == join.0)
        {
            self.joins.push(join);
        }
        binding.expr
    }

    pub(crate) fn join_sql(&self) -> String {
        let mut out = String::new();
        for (alias, col) in &self.joins {
            out.push_str(" JOIN dim_strings ");
            out.push_str(alias);
            out.push_str(" ON ");
            out.push_str(alias);
            out.push_str(".id = ");
            out.push_str(col);
        }
        out
    }

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
) -> Result<SqlFilterPlan, CoreError> {
    let mut plan = SqlFilterPlan {
        joins: Vec::new(),
        clauses: Vec::new(),
        needs_events: false,
    };
    for &dim in group_by {
        plan.ensure_dimension(dim);
    }
    for filter in filters {
        let expr = plan.ensure_dimension(filter.dimension);
        let clause = build_predicate(expr, filter.op, &filter.value, params)?;
        plan.clauses.push(clause);
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
