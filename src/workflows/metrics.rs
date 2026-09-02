use crate::jobs::JobKind;
use crate::models::{METRICS_STALE, MetricsPeriod, MetricsPoint, MetricsSeries};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use crate::state::{AppState, AuthState, TreeSel};
use crate::workflows::start_job;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(150);

pub fn plan_metrics(
    terminus: PathBuf,
    site: &str,
    env: &str,
    period: MetricsPeriod,
) -> CommandPlan {
    let site_env = format!("{site}.{env}");
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: terminus,
        argv: vec![
            "env:metrics".into(),
            site_env.clone(),
            format!("--period={}", period.label()),
            "--datapoints=auto".into(),
            "--format=json".into(),
            "--fields=datetime,visits,pages_served,cache_hits,cache_misses,cache_hit_ratio".into(),
        ],
        cwd: None,
        why: format!(
            "platform visits/pages/cache for {site_env} ({}, lagged, not APM)",
            period.label()
        ),
        safety: SafetyTier::ReadOnly,
        target: PlanTarget::Env {
            site: site.to_string(),
            env: env.to_string(),
        },
        dry_run: false,
        timeout: Some(Duration::from_secs(60)),
        expects_json: true,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: false,
    }
}

pub fn parse_metrics(
    json: &str,
    target: PlanTarget,
    period: MetricsPeriod,
) -> anyhow::Result<MetricsSeries> {
    if json.trim().is_empty() {
        return Ok(MetricsSeries {
            target,
            period,
            datapoints: "auto".into(),
            points: vec![],
            fetched_at: Instant::now(),
        });
    }
    let value: Value = serde_json::from_str(json)?;
    let mut points = points_from_value(&value);
    points.sort_by(|a, b| a.datetime.cmp(&b.datetime));
    Ok(MetricsSeries {
        target,
        period,
        datapoints: format!("{}", points.len()),
        points,
        fetched_at: Instant::now(),
    })
}

fn points_from_value(value: &Value) -> Vec<MetricsPoint> {
    match value {
        Value::Array(arr) => arr.iter().filter_map(point_from_value).collect(),
        Value::Object(map) => {
            if let Some(ts) = map.get("timeseries").or_else(|| map.get("data")) {
                return points_from_value(ts);
            }
            if map.get("visits").is_some() || map.get("pages_served").is_some() {
                return point_from_value(value).into_iter().collect();
            }
            let mut pts = Vec::new();
            for (k, v) in map {
                if let Some(mut p) = point_from_value(v) {
                    if p.datetime.is_empty() {
                        p.datetime = k.clone();
                    }
                    pts.push(p);
                }
            }
            pts
        }
        _ => vec![],
    }
}

fn point_from_value(v: &Value) -> Option<MetricsPoint> {
    let obj = v.as_object()?;
    let visits = json_u64(obj.get("visits"));
    let pages = json_u64(obj.get("pages_served").or_else(|| obj.get("pages")));
    if visits == 0
        && pages == 0
        && obj.get("cache_hit_ratio").is_none()
        && obj.get("datetime").is_none()
    {
        return None;
    }
    let hits = json_u64(obj.get("cache_hits"));
    let misses = json_u64(obj.get("cache_misses"));
    Some(MetricsPoint {
        datetime: json_str(obj.get("datetime"))
            .or_else(|| json_str(obj.get("period")))
            .or_else(|| json_str(obj.get("date")))
            .unwrap_or_default(),
        visits,
        pages_served: pages,
        cache_hits: hits,
        cache_misses: misses,
        cache_hit_ratio: parse_ratio(obj.get("cache_hit_ratio"), hits, misses, pages),
    })
}

fn parse_ratio(v: Option<&Value>, hits: u64, misses: u64, pages: u64) -> f64 {
    if let Some(v) = v {
        match v {
            Value::Number(n) => {
                let x = n.as_f64().unwrap_or(0.0);
                if x > 1.0 {
                    (x / 100.0).clamp(0.0, 1.0)
                } else {
                    x.clamp(0.0, 1.0)
                }
            }
            Value::String(s) => {
                let t = s.trim().trim_end_matches('%');
                if let Ok(x) = t.parse::<f64>() {
                    if s.contains('%') || x > 1.0 {
                        (x / 100.0).clamp(0.0, 1.0)
                    } else {
                        x.clamp(0.0, 1.0)
                    }
                } else {
                    0.0
                }
            }
            _ => derived_ratio(hits, misses, pages),
        }
    } else {
        derived_ratio(hits, misses, pages)
    }
}

fn derived_ratio(hits: u64, misses: u64, pages: u64) -> f64 {
    let denom = hits.saturating_add(misses);
    if denom > 0 {
        hits as f64 / denom as f64
    } else if pages > 0 && hits > 0 {
        (hits as f64 / pages as f64).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn json_u64(v: Option<&Value>) -> u64 {
    match v {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| f as u64))
            .unwrap_or(0),
        Some(Value::String(s)) => s.replace(',', "").parse().unwrap_or(0),
        _ => 0,
    }
}

fn json_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn cache_key(site: &str, env: &str, period: MetricsPeriod) -> (String, MetricsPeriod) {
    (format!("{site}.{env}"), period)
}

fn inflight_key(site: &str, env: &str, period: MetricsPeriod) -> String {
    format!("metrics:{site}.{env}:{}", period.label())
}

pub fn series_stale(series: &MetricsSeries) -> bool {
    series.fetched_at.elapsed() >= METRICS_STALE
}

pub fn on_selection_changed(state: &mut AppState) {
    if state.demo {
        return;
    }
    match &state.selected {
        TreeSel::Env { site, env } => {
            let frozen = state.site(site).is_some_and(|s| s.frozen);
            if frozen {
                state.pending_metrics = None;
                return;
            }
            state.pending_metrics = Some((site.clone(), env.clone(), Instant::now()));
        }
        _ => {
            state.pending_metrics = None;
        }
    }
}

pub fn flush_debounce(state: &mut AppState) {
    if state.demo {
        return;
    }
    if let Some((site, env, at)) = state.pending_metrics.clone() {
        if at.elapsed() >= DEBOUNCE {
            state.pending_metrics = None;
            request(state, &site, &env, false);
        }
    }
}

pub fn request(state: &mut AppState, site: &str, env: &str, force: bool) {
    if state.demo || !state.tools.terminus_ok() {
        return;
    }
    if !matches!(state.auth, AuthState::LoggedIn { .. }) {
        return;
    }
    if state.site(site).is_some_and(|s| s.frozen) {
        return;
    }
    let period = state.metrics_period;
    let key = cache_key(site, env, period);
    if !force {
        if let Some(series) = state.metrics.get(&key) {
            if !series_stale(series) {
                return;
            }
        }
    }
    let ikey = inflight_key(site, env, period);
    if !force && state.inflight_readonly.contains_key(&ikey) {
        return;
    }
    let plan = plan_metrics(state.tools.terminus_path(), site, env, period);
    state.current = Some(StagedPlan::One(plan.clone()));
    state.metrics_error = None;
    if let Some(id) = start_job(
        state,
        plan,
        JobKind::Metrics {
            site: site.into(),
            env: env.into(),
            period: period.label().into(),
        },
    ) {
        state.inflight_readonly.insert(ikey, id);
    }
}

pub fn apply_metrics(state: &mut AppState, site: &str, env: &str, period_label: &str, json: &str) {
    let period = match period_label {
        "week" => MetricsPeriod::Week,
        "month" => MetricsPeriod::Month,
        _ => MetricsPeriod::Day,
    };
    let target = PlanTarget::Env {
        site: site.to_string(),
        env: env.to_string(),
    };
    match parse_metrics(json, target, period) {
        Ok(series) => {
            state.metrics.insert(cache_key(site, env, period), series);
            state.metrics_error = None;
        }
        Err(err) => {
            state.metrics_error = Some(format!("env:metrics parse failed: {err:#}"));
        }
    }
}

pub fn clear_inflight(state: &mut AppState, kind: &JobKind) {
    if let JobKind::Metrics { site, env, period } = kind {
        state
            .inflight_readonly
            .remove(&format!("metrics:{site}.{env}:{period}"));
    }
}

pub fn set_period(state: &mut AppState, period: MetricsPeriod) {
    if state.metrics_period == period {
        return;
    }
    state.metrics_period = period;
    state.config.config.metrics_period = period;
    state.config.mark_dirty();
    if state.demo {
        seed_demo_period(state, period);
        return;
    }
    if let TreeSel::Env { site, env } = state.selected.clone() {
        request(state, &site, &env, false);
    }
}

fn seed_demo_period(state: &mut AppState, period: MetricsPeriod) {
    let Some((site, env_id)) = state.selected_env().map(|e| (e.site.clone(), e.id.clone())) else {
        return;
    };
    let key = cache_key(&site, &env_id, period);
    if state.metrics.contains_key(&key) {
        return;
    }
    let target = PlanTarget::Env { site, env: env_id };
    state
        .metrics
        .insert(key, crate::fixtures::walking_metrics(target, period));
}

pub fn refresh_selected(state: &mut AppState) {
    if let TreeSel::Env { site, env } = state.selected.clone() {
        request(state, &site, &env, true);
    }
}

pub fn metrics_inflight(state: &AppState) -> bool {
    let Some(env) = state.selected_env() else {
        return false;
    };
    let key = inflight_key(&env.site, &env.id, state.metrics_period);
    state.inflight_readonly.contains_key(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timeseries_array() {
        let json = r#"{
          "timeseries": [
            {"datetime":"2026-08-01","visits":10,"pages_served":40,"cache_hits":32,"cache_misses":8,"cache_hit_ratio":0.8},
            {"datetime":"2026-08-02","visits":20,"pages_served":50,"cache_hit_ratio":"88%"}
          ]
        }"#;
        let series = parse_metrics(
            json,
            PlanTarget::Env {
                site: "acme".into(),
                env: "live".into(),
            },
            MetricsPeriod::Day,
        )
        .unwrap();
        assert_eq!(series.points.len(), 2);
        assert!((series.points[0].cache_hit_ratio - 0.8).abs() < 1e-6);
        assert!((series.points[1].cache_hit_ratio - 0.88).abs() < 1e-6);
        assert_eq!(series.points[1].visits, 20);
    }

    #[test]
    fn parses_object_map_and_percent_over_100() {
        let json = r#"{
          "2026-08-01": {"visits":5,"pages_served":10,"cache_hit_ratio":90},
          "2026-08-02": {"visits":6,"pages_served":12,"cache_hits":9,"cache_misses":3}
        }"#;
        let series = parse_metrics(json, PlanTarget::None, MetricsPeriod::Week).unwrap();
        assert_eq!(series.points.len(), 2);
        assert!((series.points[0].cache_hit_ratio - 0.9).abs() < 1e-6);
        assert!((series.points[1].cache_hit_ratio - 0.75).abs() < 1e-6);
    }

    #[test]
    fn empty_json_is_empty_series() {
        let series = parse_metrics("", PlanTarget::None, MetricsPeriod::Day).unwrap();
        assert!(series.points.is_empty());
    }

    #[test]
    fn parses_root_array() {
        let json = r#"[
          {"datetime":"2026-08-01","visits":1,"pages_served":2,"cache_hit_ratio":0.5}
        ]"#;
        let series = parse_metrics(json, PlanTarget::None, MetricsPeriod::Day).unwrap();
        assert_eq!(series.points.len(), 1);
        assert!((series.points[0].cache_hit_ratio - 0.5).abs() < 1e-6);
    }

    #[test]
    fn plan_is_env_row_only() {
        let plan = plan_metrics(
            PathBuf::from("terminus"),
            "acme-wp",
            "test",
            MetricsPeriod::Day,
        );
        assert_eq!(plan.argv[0], "env:metrics");
        assert_eq!(plan.argv[1], "acme-wp.test");
        assert!(plan.argv.iter().any(|a| a == "--period=day"));
        assert!(plan.argv.iter().any(|a| a == "--datapoints=auto"));
        assert!(plan.argv.iter().any(|a| a == "--format=json"));
        assert!(!plan.argv[1].eq("acme-wp"));
        assert!(plan.expects_json);
        assert_eq!(plan.safety, SafetyTier::ReadOnly);
    }
}
