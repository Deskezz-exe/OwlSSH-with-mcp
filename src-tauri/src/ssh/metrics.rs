//! Server health for the dashboard, gathered with one exec per refresh.
//!
//! The probe is plain POSIX sh over /proc, so it needs nothing installed on
//! the server. Sections are separated by `@@name` markers and parsed here;
//! anything a server cannot answer simply stays empty.

use std::time::Duration;

use serde::Serialize;

use super::exec::exec;
use super::pool::SshConn;
use crate::error::AppResult;

/// CPU usage is the difference between two /proc/stat samples taken 400 ms apart,
/// so a single call returns a real number without keeping state between polls.
const PROBE: &str = r#"LC_ALL=C; export LC_ALL
echo @@stat; head -n1 /proc/stat; sleep 0.4; head -n1 /proc/stat
echo @@mem; grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree):' /proc/meminfo
echo @@disk; df -Pk / | tail -n1
echo @@uptime; cat /proc/uptime
echo @@load; cat /proc/loadavg
echo @@cpus; nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo
echo @@os; (. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME") || uname -sr
echo @@ps; ps -eo pid=,pcpu=,pmem=,comm= --sort=-pcpu 2>/dev/null | head -n5
echo @@docker; if command -v docker >/dev/null 2>&1; then docker ps -q 2>/dev/null | wc -l; docker ps -aq 2>/dev/null | wc -l; fi
echo @@end"#;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub cpu_percent: Option<f32>,
    pub cpus: Option<u32>,
    pub load: Option<[f32; 3]>,
    pub mem_total_kb: Option<u64>,
    pub mem_used_kb: Option<u64>,
    pub swap_total_kb: Option<u64>,
    pub swap_used_kb: Option<u64>,
    pub disk_total_kb: Option<u64>,
    pub disk_used_kb: Option<u64>,
    pub uptime_secs: Option<u64>,
    pub os: Option<String>,
    pub processes: Vec<Process>,
    pub docker: Option<Docker>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub pid: u32,
    pub cpu: f32,
    pub mem: f32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Docker {
    pub running: u32,
    pub total: u32,
}

pub async fn collect(conn: &SshConn) -> AppResult<Metrics> {
    let out = exec(conn, PROBE, Duration::from_secs(15)).await?;
    Ok(parse(&out.stdout))
}

pub fn parse(text: &str) -> Metrics {
    let mut m = Metrics::default();
    for (name, body) in sections(text) {
        let lines: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        match name {
            "stat" => m.cpu_percent = parse_cpu(&lines),
            "mem" => parse_mem(&lines, &mut m),
            "disk" => {
                // Filesystem 1024-blocks Used Available Capacity Mounted-on
                let cols: Vec<&str> = lines
                    .first()
                    .map(|l| l.split_whitespace().collect())
                    .unwrap_or_default();
                if cols.len() >= 4 {
                    m.disk_total_kb = cols[1].parse().ok();
                    m.disk_used_kb = cols[2].parse().ok();
                }
            }
            "uptime" => {
                m.uptime_secs = lines
                    .first()
                    .and_then(|l| l.split_whitespace().next())
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v as u64);
            }
            "load" => {
                let v: Vec<f32> = lines
                    .first()
                    .map(|l| {
                        l.split_whitespace()
                            .take(3)
                            .filter_map(|x| x.parse().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                if v.len() == 3 {
                    m.load = Some([v[0], v[1], v[2]]);
                }
            }
            "cpus" => m.cpus = lines.first().and_then(|l| l.parse().ok()),
            "os" => m.os = lines.first().map(|l| l.trim_matches('"').to_string()),
            "ps" => m.processes = lines.iter().filter_map(|l| parse_process(l)).collect(),
            "docker" => {
                if let [running, total, ..] = lines.as_slice() {
                    if let (Ok(running), Ok(total)) = (running.parse(), total.parse()) {
                        m.docker = Some(Docker { running, total });
                    }
                }
            }
            _ => {}
        }
    }
    m
}

fn sections(text: &str) -> Vec<(&str, String)> {
    let mut out: Vec<(&str, String)> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix("@@") {
            out.push((name, String::new()));
        } else if let Some((_, body)) = out.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out
}

/// Busy share between two `cpu  user nice system idle iowait irq softirq steal` lines.
fn parse_cpu(lines: &[&str]) -> Option<f32> {
    let sample = |line: &str| -> Option<(u64, u64)> {
        let v: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|x| x.parse().ok())
            .collect();
        if v.len() < 4 {
            return None;
        }
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        let total: u64 = v.iter().take(8).sum();
        Some((total, idle))
    };
    let (t1, i1) = sample(lines.first()?)?;
    let (t2, i2) = sample(lines.get(1)?)?;
    let total = t2.checked_sub(t1)?;
    if total == 0 {
        return Some(0.0);
    }
    let idle = i2.saturating_sub(i1);
    Some(((total - idle.min(total)) as f32 / total as f32 * 100.0).clamp(0.0, 100.0))
}

fn parse_mem(lines: &[&str], m: &mut Metrics) {
    let value = |key: &str| -> Option<u64> {
        lines
            .iter()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
    };
    m.mem_total_kb = value("MemTotal:");
    m.mem_used_kb = match (m.mem_total_kb, value("MemAvailable:")) {
        (Some(total), Some(avail)) => Some(total.saturating_sub(avail)),
        _ => None,
    };
    m.swap_total_kb = value("SwapTotal:");
    m.swap_used_kb = match (m.swap_total_kb, value("SwapFree:")) {
        (Some(total), Some(free)) => Some(total.saturating_sub(free)),
        _ => None,
    };
}

fn parse_process(line: &str) -> Option<Process> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let cpu = parts.next()?.parse().ok()?;
    let mem = parts.next()?.parse().ok()?;
    let name = parts.collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(Process {
        pid,
        cpu,
        mem,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@@stat
cpu  1000 0 500 8000 500 0 0 0 0 0
cpu  1100 0 550 8300 550 0 0 0 0 0
@@mem
MemTotal:        4028076 kB
MemAvailable:    1007019 kB
SwapTotal:       2097148 kB
SwapFree:        2097148 kB
@@disk
/dev/vda1         41152736 12345678  26690000      32% /
@@uptime
355612.33 1400000.00
@@load
0.52 0.40 0.35 1/245 12345
@@cpus
2
@@os
Ubuntu 24.04.1 LTS
@@ps
 1234 35.5  4.1 node
  987 12.0 10.2 postgres
@@docker
3
5
@@end
";

    #[test]
    fn parses_a_full_probe() {
        let m = parse(SAMPLE);
        // 500 ticks passed, 350 of them idle or iowait.
        assert!((m.cpu_percent.unwrap() - 30.0).abs() < 0.01);
        assert_eq!(m.cpus, Some(2));
        assert_eq!(m.load, Some([0.52, 0.40, 0.35]));
        assert_eq!(m.mem_total_kb, Some(4028076));
        assert_eq!(m.mem_used_kb, Some(4028076 - 1007019));
        assert_eq!(m.swap_used_kb, Some(0));
        assert_eq!(m.disk_total_kb, Some(41152736));
        assert_eq!(m.disk_used_kb, Some(12345678));
        assert_eq!(m.uptime_secs, Some(355612));
        assert_eq!(m.os.as_deref(), Some("Ubuntu 24.04.1 LTS"));
        assert_eq!(m.processes.len(), 2);
        assert_eq!(m.processes[0].name, "node");
        assert_eq!(m.processes[0].cpu, 35.5);
        assert_eq!(
            m.docker,
            Some(Docker {
                running: 3,
                total: 5
            })
        );
    }

    #[test]
    fn missing_sections_stay_empty() {
        let m = parse("@@stat\n@@mem\n@@docker\n@@end\n");
        assert_eq!(m, Metrics::default());
    }

    #[test]
    fn cpu_counter_reset_does_not_panic() {
        let lines = ["cpu 100 0 0 900", "cpu 10 0 0 90"];
        assert_eq!(parse_cpu(&lines), None);
    }
}
