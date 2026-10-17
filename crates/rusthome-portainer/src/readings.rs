//! Docker API data → readings. Keys match HA's Portainer integration
//! (`unique_id` `{entry}_{container}_{key}`), observed in the real registry.

use serde_json::Value;

use crate::ContainerData;

#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub key: &'static str,
    pub domain: &'static str,
    pub label: &'static str,
    pub state: String,
    pub unit: Option<&'static str>,
    pub device_class: Option<&'static str>,
}

fn r(key: &'static str, domain: &'static str, label: &'static str, state: String) -> Reading {
    Reading {
        key,
        domain,
        label,
        state,
        unit: None,
        device_class: None,
    }
}

fn on_off(b: bool) -> String {
    if b { "on" } else { "off" }.to_owned()
}

#[allow(clippy::cast_precision_loss)] // byte counts far below 2^52
fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000.0)
}

/// Readings of one container. Stats-based ones only when stats exist.
#[must_use]
pub fn readings(c: &ContainerData) -> Vec<Reading> {
    let state = &c.inspect["State"];
    let status = state["Status"]
        .as_str()
        .or_else(|| c.summary["State"].as_str())
        .unwrap_or("unknown");
    let mut out = vec![
        Reading {
            device_class: Some("running"),
            ..r(
                "status",
                "binary_sensor",
                "Statut",
                on_off(status == "running"),
            )
        },
        Reading {
            device_class: Some("enum"),
            ..r("container_state", "sensor", "État", status.to_owned())
        },
        Reading {
            device_class: Some("enum"),
            ..r(
                "container_health_state",
                "sensor",
                "Santé",
                state["Health"]["Status"]
                    .as_str()
                    .unwrap_or("none")
                    .to_owned(),
            )
        },
        r(
            "image",
            "sensor",
            "Image",
            c.inspect["Config"]["Image"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ),
        r(
            "container_restart_count",
            "sensor",
            "Restart count",
            c.inspect["RestartCount"].as_u64().unwrap_or(0).to_string(),
        ),
        Reading {
            device_class: Some("problem"),
            ..r(
                "container_out_of_memory",
                "binary_sensor",
                "Out of memory",
                on_off(state["OOMKilled"].as_bool().unwrap_or(false)),
            )
        },
    ];
    if let Some(started) = state["StartedAt"]
        .as_str()
        .filter(|s| !s.starts_with("0001"))
    {
        out.push(Reading {
            device_class: Some("timestamp"),
            ..r(
                "container_started_at",
                "sensor",
                "Started",
                started.to_owned(),
            )
        });
    }
    if let Some(stats) = &c.stats {
        out.extend(stat_readings(stats));
    }
    out
}

fn stat_readings(s: &Value) -> Vec<Reading> {
    let mut out = Vec::new();
    let u = |v: &Value| v.as_u64();
    let cpu = (|| {
        let total = u(&s["cpu_stats"]["cpu_usage"]["total_usage"])?;
        let pre = u(&s["precpu_stats"]["cpu_usage"]["total_usage"])?;
        let sys = u(&s["cpu_stats"]["system_cpu_usage"])?;
        let pre_sys = u(&s["precpu_stats"]["system_cpu_usage"])?;
        let cpus = u(&s["cpu_stats"]["online_cpus"]).unwrap_or(1);
        let (d, ds) = (total.checked_sub(pre)?, sys.checked_sub(pre_sys)?);
        #[allow(clippy::cast_precision_loss)]
        (ds > 0).then(|| d as f64 / ds as f64 * cpus as f64 * 100.0)
    })();
    if let Some(pct) = cpu {
        out.push(Reading {
            unit: Some("%"),
            ..r(
                "cpu_usage_total",
                "sensor",
                "Utilisation totale du processeur",
                format!("{pct:.1}"),
            )
        });
    }
    let mem = &s["memory_stats"];
    if let (Some(usage), Some(limit)) = (u(&mem["usage"]), u(&mem["limit"])) {
        // cgroup v2: inactive_file; cgroup v1: cache.
        let cache = u(&mem["stats"]["inactive_file"])
            .or_else(|| u(&mem["stats"]["cache"]))
            .unwrap_or(0);
        let used = usage.saturating_sub(cache);
        out.push(Reading {
            unit: Some("MB"),
            device_class: Some("data_size"),
            ..r(
                "memory_usage",
                "sensor",
                "Utilisation de la mémoire",
                mb(used),
            )
        });
        out.push(Reading {
            unit: Some("MB"),
            device_class: Some("data_size"),
            ..r("memory_limit", "sensor", "Limite de mémoire", mb(limit))
        });
        if limit > 0 {
            #[allow(clippy::cast_precision_loss)]
            let pct = used as f64 / limit as f64 * 100.0;
            out.push(Reading {
                unit: Some("%"),
                ..r(
                    "memory_usage_percentage",
                    "sensor",
                    "Pourcentage d'utilisation de la mémoire",
                    format!("{pct:.2}"),
                )
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::frigate;

    fn get<'a>(rs: &'a [Reading], key: &str) -> &'a Reading {
        rs.iter().find(|r| r.key == key).unwrap()
    }

    #[test]
    fn computes_docker_like_metrics() {
        let rs = readings(&frigate());
        assert_eq!(get(&rs, "status").state, "on");
        assert_eq!(get(&rs, "container_health_state").state, "healthy");
        assert_eq!(get(&rs, "container_restart_count").state, "3");
        // 1e9 / 1e10 * 4 cpus * 100
        assert_eq!(get(&rs, "cpu_usage_total").state, "40.0");
        // 600 MB - 100 MB inactive_file
        assert_eq!(get(&rs, "memory_usage").state, "500.0");
        assert_eq!(get(&rs, "memory_usage_percentage").state, "12.50");
        assert_eq!(get(&rs, "container_out_of_memory").state, "off");
    }

    #[test]
    fn stopped_container_has_no_stats_readings() {
        let mut c = frigate();
        c.stats = None;
        c.inspect["State"]["Status"] = "exited".into();
        let rs = readings(&c);
        assert_eq!(get(&rs, "status").state, "off");
        assert!(rs.iter().all(|r| r.key != "cpu_usage_total"));
    }
}
