//! A small reader for OpenSSH client configs (`~/.ssh/config`).
//!
//! It resolves each concrete `Host` alias the way `ssh` does: blocks are walked
//! top to bottom, every block whose patterns match contributes, and the first
//! value seen for a keyword wins. Only the keywords OwlSSH uses are kept.
//! `Match` blocks are skipped, `Include` is followed (with globs).

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigEntry {
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub user: Option<String>,
    pub identity_file: Option<String>,
    /// First hop of `ProxyJump`, as written (an alias or `user@host:port`).
    pub proxy_jump: Option<String>,
}

#[derive(Debug, Default)]
struct Block {
    patterns: Vec<String>,
    /// `(keyword lowercased, value)` in file order.
    options: Vec<(String, String)>,
}

const MAX_INCLUDE_DEPTH: usize = 8;

pub fn default_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

/// Reads and resolves a config file. A missing file gives an empty list.
pub fn load(path: &Path) -> Vec<SshConfigEntry> {
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut blocks = vec![Block {
        patterns: vec!["*".into()],
        ..Block::default()
    }];
    read_file(path, &base, &mut blocks, 0);
    resolve(&blocks)
}

/// Parses config text, `Include` paths are resolved against `base`.
#[cfg(test)]
fn parse(text: &str, base: &Path) -> Vec<SshConfigEntry> {
    let mut blocks = vec![Block {
        patterns: vec!["*".into()],
        ..Block::default()
    }];
    parse_into(text, base, &mut blocks, 0);
    resolve(&blocks)
}

fn read_file(path: &Path, base: &Path, blocks: &mut Vec<Block>, depth: usize) {
    if let Ok(text) = std::fs::read_to_string(path) {
        parse_into(&text, base, blocks, depth);
    }
}

fn parse_into(text: &str, base: &Path, blocks: &mut Vec<Block>, depth: usize) {
    let mut in_match = false;
    for raw in text.lines() {
        let Some((key, value)) = split_line(raw) else {
            continue;
        };
        match key.as_str() {
            "host" => {
                in_match = false;
                blocks.push(Block {
                    patterns: split_args(&value),
                    options: Vec::new(),
                });
            }
            "match" => in_match = true,
            _ if in_match => {}
            "include" => {
                if depth < MAX_INCLUDE_DEPTH {
                    for arg in split_args(&value) {
                        for file in expand_include(&arg, base) {
                            read_file(&file, base, blocks, depth + 1);
                        }
                    }
                }
            }
            _ => {
                if let Some(block) = blocks.last_mut() {
                    block.options.push((key, value));
                }
            }
        }
    }
}

/// Splits `Keyword value` or `Keyword=value`, drops comments and blank lines.
fn split_line(raw: &str) -> Option<(String, String)> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let idx = line.find(|c: char| c.is_whitespace() || c == '=')?;
    let key = line[..idx].to_ascii_lowercase();
    let value = line[idx..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
        .trim()
        .to_string();
    Some((key, value))
}

/// Splits arguments on whitespace, honouring double quotes.
fn split_args(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in value.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn expand_include(arg: &str, base: &Path) -> Vec<PathBuf> {
    let expanded = expand_tilde(arg);
    let path = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        base.join(&expanded)
    };
    let pattern = path.to_string_lossy().to_string();
    match glob::glob(&pattern) {
        Ok(paths) => paths.filter_map(Result::ok).collect(),
        Err(_) => vec![path],
    }
}

pub fn expand_tilde(p: &str) -> String {
    match (
        p.strip_prefix("~/").or(p.strip_prefix("~\\")),
        dirs::home_dir(),
    ) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().to_string(),
        _ => p.to_string(),
    }
}

fn is_concrete(pattern: &str) -> bool {
    !pattern.contains(['*', '?', '!'])
}

fn block_matches(block: &Block, alias: &str) -> bool {
    let mut matched = false;
    for p in &block.patterns {
        if let Some(neg) = p.strip_prefix('!') {
            if wildcard_match(neg, alias) {
                return false;
            }
        } else if wildcard_match(p, alias) {
            matched = true;
        }
    }
    matched
}

/// `*` and `?` glob matching, case-insensitive like OpenSSH host patterns.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

fn resolve(blocks: &[Block]) -> Vec<SshConfigEntry> {
    let mut aliases: Vec<String> = Vec::new();
    for block in blocks {
        for p in &block.patterns {
            if is_concrete(p) && !aliases.iter().any(|a| a == p) {
                aliases.push(p.clone());
            }
        }
    }

    aliases
        .into_iter()
        .map(|alias| {
            let first = |key: &str| -> Option<String> {
                blocks
                    .iter()
                    .filter(|b| block_matches(b, &alias))
                    .flat_map(|b| b.options.iter())
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
            };
            let hostname = first("hostname").unwrap_or_else(|| alias.clone());
            let port = first("port").and_then(|p| p.parse().ok()).unwrap_or(22);
            let identity_file = first("identityfile")
                .and_then(|v| split_args(&v).into_iter().next())
                .map(|v| expand_tilde(&v));
            let proxy_jump = first("proxyjump")
                .and_then(|v| v.split(',').next().map(|s| s.trim().to_string()))
                .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("none"));
            let user = first("user");
            SshConfigEntry {
                hostname: hostname.replace("%h", &alias),
                alias,
                port,
                user,
                identity_file,
                proxy_jump,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(text: &str) -> Vec<SshConfigEntry> {
        parse(text, Path::new("."))
    }

    #[test]
    fn reads_basic_hosts() {
        let cfg = "
# comment
Host web
    HostName 10.0.0.5
    User deploy
    Port 2222
    IdentityFile ~/.ssh/web_ed25519

Host db
  hostname=db.internal
";
        let e = entries(cfg);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].alias, "web");
        assert_eq!(e[0].hostname, "10.0.0.5");
        assert_eq!(e[0].user.as_deref(), Some("deploy"));
        assert_eq!(e[0].port, 2222);
        assert!(e[0]
            .identity_file
            .as_deref()
            .unwrap()
            .ends_with("web_ed25519"));
        assert_eq!(e[1].hostname, "db.internal");
        assert_eq!(e[1].port, 22);
    }

    #[test]
    fn first_value_wins_and_wildcards_apply() {
        let cfg = "
Host prod-*
    User admin
Host prod-api
    HostName api.example.com
    User ignored
Host *
    User fallback
    Port 2200
";
        let e = entries(cfg);
        assert_eq!(e.len(), 1, "wildcard patterns are not hosts");
        assert_eq!(e[0].user.as_deref(), Some("admin"));
        assert_eq!(e[0].port, 2200);
    }

    #[test]
    fn negated_patterns_exclude() {
        let cfg = "
Host * !bastion
    ProxyJump bastion
Host bastion app
";
        let e = entries(cfg);
        let bastion = e.iter().find(|x| x.alias == "bastion").unwrap();
        let app = e.iter().find(|x| x.alias == "app").unwrap();
        assert_eq!(bastion.proxy_jump, None);
        assert_eq!(app.proxy_jump.as_deref(), Some("bastion"));
    }

    #[test]
    fn match_blocks_are_skipped() {
        let cfg = "
Host a
Match user root
    User nobody
Host b
";
        let e = entries(cfg);
        assert_eq!(e.len(), 2);
        assert!(e.iter().all(|x| x.user.is_none()));
    }

    #[test]
    fn proxyjump_keeps_first_hop() {
        let e = entries("Host x\n ProxyJump j1, j2\n");
        assert_eq!(e[0].proxy_jump.as_deref(), Some("j1"));
        let e = entries("Host x\n ProxyJump none\n");
        assert_eq!(e[0].proxy_jump, None);
    }

    #[test]
    fn follows_includes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("conf.d")).unwrap();
        std::fs::write(
            dir.path().join("conf.d/one.conf"),
            "Host inc\n HostName 1.2.3.4\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("config"),
            "Include conf.d/*.conf\nHost main\n",
        )
        .unwrap();
        let e = load(&dir.path().join("config"));
        let names: Vec<_> = e.iter().map(|x| x.alias.as_str()).collect();
        assert_eq!(names, ["inc", "main"]);
        assert_eq!(e[0].hostname, "1.2.3.4");
    }

    #[test]
    fn wildcard_matching() {
        assert!(wildcard_match("*.example.com", "a.example.com"));
        assert!(wildcard_match("web?", "web1"));
        assert!(!wildcard_match("web?", "web12"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("PROD", "prod"));
    }
}
