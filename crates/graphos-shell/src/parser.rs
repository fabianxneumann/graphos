use alloc::vec::Vec;
use alloc::string::String;
use graphos_core::fnv1a_64;

/// Parsed shell command
#[derive(Debug)]
pub enum Command {
    /// .look [deep]
    Look { deep: bool },
    /// .go <path>
    Go { path: Path },
    /// .spawn <NodeType> [name]
    Spawn { type_name: String, name: Option<String> },
    /// .link <src> <target> <kind>
    Link { src: Path, target: Path, kind: String },
    /// .cut <src> <target>
    Cut { src: Path, target: Path },
    /// .find <type> [depth N]
    Find { type_filter: Option<String>, max_depth: u8 },
    /// .who
    Who,
    /// .bind <name> <target>
    Bind { name: String, target: Path },
    /// .touch <edge_name>
    Touch { edge_name: String },
    /// .ask <prompt> (AI inference)
    Ask { prompt: String },
    /// .history
    History,
    /// .help
    Help,
    /// .snapshot — create graph snapshot
    Snapshot,
    /// .rollback <epoch> — rollback to snapshot epoch
    Rollback { epoch: u64 },
    /// .store <data> — store content-addressed data
    Store { data: String },
    /// .similar [k] — find k similar nodes
    Similar { k: usize },
    /// .physics [n] — run n force-directed steps
    Physics { steps: usize },
    /// .clusters — show detected clusters
    Clusters,
    /// Natural language (fallback to AI)
    NaturalLanguage { text: String },
    /// Raw traversal: #<node_id>
    RawTraversal { node_id_hex: String },
    /// Empty input
    Empty,
}

/// Path representation
#[derive(Debug, Clone)]
pub enum Path {
    Absolute(Vec<u64>),
    Relative(Vec<u64>),
    Parent,
    Alias(String),
    Direct(String),
}

impl Path {
    pub fn parse(input: &str) -> Self {
        if input == ".." {
            return Path::Parent;
        }
        if let Some(alias) = input.strip_prefix('@') {
            return Path::Alias(String::from(alias));
        }
        if let Some(hex) = input.strip_prefix('#') {
            return Path::Direct(String::from(hex));
        }
        if let Some(abs) = input.strip_prefix('/') {
            let hashes: Vec<u64> = abs
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| fnv1a_64(s.as_bytes()))
                .collect();
            return Path::Absolute(hashes);
        }
        if let Some(rel) = input.strip_prefix("./") {
            let hashes: Vec<u64> = rel
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| fnv1a_64(s.as_bytes()))
                .collect();
            return Path::Relative(hashes);
        }
        // Default: treat as relative single segment
        Path::Relative(alloc::vec![fnv1a_64(input.as_bytes())])
    }
}

/// Parse a raw input line into a Command
pub fn parse(input: &str) -> Command {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }

    // Raw traversal
    if trimmed.starts_with('#') {
        return Command::RawTraversal {
            node_id_hex: String::from(&trimmed[1..]),
        };
    }

    // Direct ops (.-prefix)
    if let Some(cmd) = trimmed.strip_prefix('.') {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let verb = parts[0];
        let args = parts.get(1).copied().unwrap_or("");

        return match verb {
            "look" => Command::Look {
                deep: args == "deep",
            },
            "go" => Command::Go {
                path: Path::parse(args),
            },
            "spawn" => {
                let spawn_parts: Vec<&str> = args.splitn(2, ' ').collect();
                Command::Spawn {
                    type_name: String::from(spawn_parts[0]),
                    name: spawn_parts.get(1).map(|s| String::from(*s)),
                }
            }
            "link" => {
                let link_parts: Vec<&str> = args.splitn(3, ' ').collect();
                if link_parts.len() >= 2 {
                    Command::Link {
                        src: Path::parse(link_parts[0]),
                        target: Path::parse(link_parts[1]),
                        kind: link_parts
                            .get(2)
                            .map(|s| String::from(*s))
                            .unwrap_or_else(|| String::from("reference")),
                    }
                } else {
                    Command::Help
                }
            }
            "cut" => {
                let cut_parts: Vec<&str> = args.splitn(2, ' ').collect();
                if cut_parts.len() == 2 {
                    Command::Cut {
                        src: Path::parse(cut_parts[0]),
                        target: Path::parse(cut_parts[1]),
                    }
                } else {
                    Command::Help
                }
            }
            "find" => {
                let find_parts: Vec<&str> = args.split(' ').collect();
                let type_filter = if find_parts.first().map(|s| !s.is_empty()).unwrap_or(false) {
                    Some(String::from(*find_parts.first().unwrap()))
                } else {
                    None
                };
                let max_depth = find_parts
                    .iter()
                    .position(|&s| s == "depth")
                    .and_then(|i| find_parts.get(i + 1))
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(2);
                Command::Find {
                    type_filter,
                    max_depth,
                }
            }
            "who" => Command::Who,
            "bind" => {
                let bind_parts: Vec<&str> = args.splitn(2, ' ').collect();
                if bind_parts.len() == 2 {
                    Command::Bind {
                        name: String::from(bind_parts[0]),
                        target: Path::parse(bind_parts[1]),
                    }
                } else {
                    Command::Help
                }
            }
            "touch" => Command::Touch {
                edge_name: String::from(args),
            },
            "ask" => Command::Ask {
                prompt: String::from(args),
            },
            "history" => Command::History,
            "help" | "?" => Command::Help,
            "snapshot" => Command::Snapshot,
            "rollback" => {
                let epoch = args.trim().parse::<u64>().unwrap_or(0);
                Command::Rollback { epoch }
            }
            "store" => Command::Store {
                data: String::from(args),
            },
            "similar" => {
                let k = args.trim().parse::<usize>().unwrap_or(5);
                Command::Similar { k }
            }
            "physics" => {
                let steps = args.trim().parse::<usize>().unwrap_or(100);
                Command::Physics { steps }
            }
            "clusters" => Command::Clusters,
            _ => Command::Help,
        };
    }

    // Natural language fallback
    Command::NaturalLanguage {
        text: String::from(trimmed),
    }
}
