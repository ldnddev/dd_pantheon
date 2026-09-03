use crate::models::{
    Backup, ConnectionMode, Env, Framework, LocalApp, MetricsPeriod, MetricsPoint, MetricsSeries,
    OrgRef, Site, SiteOverlay, Tag,
};
use crate::plan::{CommandPlan, PlanTarget, SafetyTier, StagedPlan, ToolKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

pub struct DemoData {
    pub sites: Vec<Site>,
    pub envs: HashMap<String, Vec<Env>>,
    pub metrics: HashMap<(String, MetricsPeriod), MetricsSeries>,
    pub backups: HashMap<String, Vec<Backup>>,
    pub plan: StagedPlan,
    pub log_lines: Vec<String>,
}

pub fn demo_data() -> DemoData {
    let org = OrgRef {
        org_id: "11111111-2222-3333-4444-555555555555".into(),
        org_name: "ldnddev".into(),
    };

    let acme_wp = Site {
        name: "acme-wp".into(),
        id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
        label: Some("Acme WP".into()),
        framework: Framework::WordPress,
        region: Some("us-central".into()),
        frozen: false,
        plan_name: Some("Gold".into()),
        owner: Some("jared".into()),
        upstream: Some("wordpress".into()),
        upstream_label: Some("WordPress".into()),
        memberships: Some("ldnddev".into()),
        tags: vec![
            Tag {
                name: "prod".into(),
                org: org.org_id.clone(),
            },
            Tag {
                name: "client-acme".into(),
                org: org.org_id.clone(),
            },
        ],
        orgs: vec![org.clone()],
        local: Some(LocalApp {
            path: PathBuf::from("/home/jlyvers/sites/acme-wp"),
            lando_name: Some("acmewp".into()),
            recipe: Some("pantheon".into()),
            framework: Some(Framework::WordPress),
            terminus_site: Some("acme-wp".into()),
            running: Some(true),
            url: Some("https://acme-wp.lndo.site".into()),
        }),
        overlay: Some(SiteOverlay {
            cms: Some(Framework::WordPress),
            multidev_ok: true,
            composer_managed: false,
            git_branch: Some("master".into()),
            local_path: Some(PathBuf::from("/home/jlyvers/sites/acme-wp")),
        }),
    };

    let acme_d10 = Site {
        name: "acme-d10".into(),
        id: "bbbbbbbb-cccc-dddd-eeee-ffffffffffff".into(),
        label: Some("Acme Drupal".into()),
        framework: Framework::Drupal,
        region: Some("us-central".into()),
        frozen: false,
        plan_name: Some("Gold".into()),
        owner: Some("jared".into()),
        upstream: Some("drupal".into()),
        upstream_label: Some("Drupal".into()),
        memberships: Some("ldnddev".into()),
        tags: vec![Tag {
            name: "prod".into(),
            org: org.org_id.clone(),
        }],
        orgs: vec![org.clone()],
        local: None,
        overlay: Some(SiteOverlay {
            cms: Some(Framework::Drupal),
            multidev_ok: true,
            composer_managed: true,
            git_branch: Some("master".into()),
            local_path: None,
        }),
    };

    let frozen_lab = Site {
        name: "frozen-lab".into(),
        id: "cccccccc-dddd-eeee-ffff-000000000000".into(),
        label: Some("Frozen Lab".into()),
        framework: Framework::WordPress,
        region: Some("us-west".into()),
        frozen: true,
        plan_name: Some("Sandbox".into()),
        owner: Some("jared".into()),
        upstream: None,
        upstream_label: None,
        memberships: None,
        tags: vec![],
        orgs: vec![],
        local: None,
        overlay: None,
    };

    let mut envs = HashMap::new();
    envs.insert(
        "acme-wp".into(),
        vec![
            env("acme-wp", "dev", ConnectionMode::Git, false),
            env("acme-wp", "test", ConnectionMode::Git, false),
            env("acme-wp", "live", ConnectionMode::Git, true),
            env("acme-wp", "feat-x", ConnectionMode::Sftp, false),
        ],
    );
    envs.insert(
        "acme-d10".into(),
        vec![
            env("acme-d10", "dev", ConnectionMode::Git, false),
            env("acme-d10", "test", ConnectionMode::Git, false),
            env("acme-d10", "live", ConnectionMode::Sftp, false),
        ],
    );
    envs.insert(
        "frozen-lab".into(),
        vec![
            env("frozen-lab", "dev", ConnectionMode::Git, false),
            env("frozen-lab", "test", ConnectionMode::Git, false),
            env("frozen-lab", "live", ConnectionMode::Git, false),
        ],
    );

    let mut metrics = HashMap::new();
    for site in ["acme-wp", "acme-d10", "frozen-lab"] {
        for env_id in env_ids_for(site) {
            let target = PlanTarget::Env {
                site: site.to_string(),
                env: env_id.to_string(),
            };
            metrics.insert(
                (format!("{site}.{env_id}"), MetricsPeriod::Day),
                walking_metrics(target, MetricsPeriod::Day),
            );
        }
    }

    let mut backups = HashMap::new();
    backups.insert("acme-wp.test".into(), dummy_backups("acme-wp", "test"));
    backups.insert("acme-wp.live".into(), dummy_backups("acme-wp", "live"));

    DemoData {
        sites: vec![acme_wp, acme_d10, frozen_lab],
        envs,
        metrics,
        backups,
        plan: StagedPlan::One(dummy_backup_plan("acme-wp", "test")),
        log_lines: vec![
            "[12:01:03] Created backup_20260831_acme-wp_test.tgz".into(),
            "[12:01:04] backup:create finished (exit 0)".into(),
            "[12:01:05] (demo log — no live job)".into(),
        ],
    }
}

fn env_ids_for(site: &str) -> &'static [&'static str] {
    match site {
        "acme-wp" => &["dev", "test", "live", "feat-x"],
        _ => &["dev", "test", "live"],
    }
}

fn env(site: &str, id: &str, mode: ConnectionMode, locked: bool) -> Env {
    Env {
        id: id.into(),
        site: site.into(),
        domain: Some(format!("{id}-{site}.pantheonsite.io")),
        connection_mode: mode,
        locked,
        initialized: true,
        php_version: Some("8.3".into()),
        php_runtime_generation: Some("8.3".into()),
        drush_version: None,
        created: None,
    }
}

pub fn dummy_backups(site: &str, env: &str) -> Vec<Backup> {
    vec![
        Backup {
            file: format!("backup_20260831_{site}_{env}_all.tgz"),
            size: "48.2M".into(),
            date: "2026-08-31 12:01:03".into(),
            expiry: "2027-08-31".into(),
            initiator: "manual".into(),
        },
        Backup {
            file: format!("backup_20260824_{site}_{env}_database.tgz"),
            size: "12.0M".into(),
            date: "2026-08-24 09:00:00".into(),
            expiry: "2027-08-24".into(),
            initiator: "automated".into(),
        },
    ]
}

pub fn dummy_backup_plan(site: &str, env: &str) -> CommandPlan {
    CommandPlan {
        tool: ToolKind::Terminus,
        binary: PathBuf::from("terminus"),
        argv: vec![
            "backup:create".into(),
            format!("{site}.{env}"),
            "--element=all".into(),
        ],
        cwd: None,
        why: format!("backup {site}.{env} before deploy (demo)"),
        safety: SafetyTier::Mutating,
        target: PlanTarget::Env {
            site: site.into(),
            env: env.into(),
        },
        dry_run: true,
        timeout: None,
        expects_json: false,
        extra_env: vec![],
        redact: vec![],
        confirm_with_yes: true,
    }
}

pub fn walking_metrics(target: PlanTarget, period: MetricsPeriod) -> MetricsSeries {
    let mut points = Vec::with_capacity(14);
    for i in 0..14 {
        let t = 0.42 + (0.93 - 0.42) * (i as f64) / 13.0;
        let visits = 800 + i as u64 * 40;
        let pages = visits * 3 + 200;
        let hits = (pages as f64 * t) as u64;
        points.push(MetricsPoint {
            datetime: format!("2026-08-{:02}", i + 17),
            visits,
            pages_served: pages,
            cache_hits: hits,
            cache_misses: pages.saturating_sub(hits),
            cache_hit_ratio: t,
        });
    }
    MetricsSeries {
        target,
        period,
        datapoints: "14".into(),
        points,
        fetched_at: Instant::now(),
    }
}
