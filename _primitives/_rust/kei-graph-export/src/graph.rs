use crate::{edges, nodes};
use crate::types::{Edge, Node, Space, SpaceData};
use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct GraphConfig {
    pub registry_db: PathBuf,
    pub ledger_db: PathBuf,
    pub manifests_dir: PathBuf,
}

pub fn build_graph(cfg: &GraphConfig) -> Result<Space> {
    let reg = Connection::open(&cfg.registry_db)?;
    let led = Connection::open(&cfg.ledger_db)?;

    let (mut nodes, block_lookup) = nodes::collect_blocks(&reg)?;
    let agents = nodes::collect_agents(&led, &mut nodes)?;
    let manifests = nodes::collect_manifests(&cfg.manifests_dir, &mut nodes)?;
    nodes::collect_branches(&led, &mut nodes)?;
    nodes::collect_skills(&led, &mut nodes)?;

    let mut links = Vec::new();
    edges::edges_manifest_block(&manifests, &block_lookup, &mut links);
    edges::edges_manifest_path_ref(&manifests, &block_lookup, &mut links);
    edges::edges_manifest_rule(&manifests, &block_lookup, &mut links);
    edges::edges_branch_lineage(&led, &mut links)?;
    edges::edges_agent_fork(&agents, &mut links);
    edges::edges_skill_invocation(&led, &mut links)?;
    edges::edges_agent_manifest(&agents, &manifests, &mut links);

    compute_connections(&mut nodes, &links);

    Ok(Space {
        name: "Runtime",
        icon: "activity",
        description: "Live KeiSei agent + atom DNA graph",
        colors: default_colors(),
        data: SpaceData { nodes, links },
    })
}

fn compute_connections(nodes: &mut [Node], links: &[Edge]) {
    let mut degree: HashMap<String, usize> = HashMap::new();
    for l in links {
        *degree.entry(l.source.clone()).or_default() += 1;
        *degree.entry(l.target.clone()).or_default() += 1;
    }
    for n in nodes.iter_mut() {
        n.connections = *degree.get(&n.id).unwrap_or(&0);
    }
}

fn default_colors() -> HashMap<String, String> {
    [
        ("atom", "#6ee7b7"),
        ("hook", "#f59e0b"),
        ("primitive", "#60a5fa"),
        ("rule", "#c084fc"),
        ("skill", "#34d399"),
        ("agent", "#f87171"),
        ("branch", "#94a3b8"),
        ("manifest", "#fb923c"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            title: id.to_string(),
            kind: "atom".to_string(),
            category: "atom".to_string(),
            tags: Vec::new(),
            connections: 0,
            extra: HashMap::new(),
        }
    }

    fn edge(src: &str, tgt: &str) -> Edge {
        Edge {
            source: src.to_string(),
            target: tgt.to_string(),
            kind: "block_dep".to_string(),
            weight: 1.0,
        }
    }

    #[test]
    fn compute_connections_counts_both_endpoints() {
        let mut nodes = vec![node("a"), node("b"), node("c")];
        let links = vec![edge("a", "b"), edge("a", "c")];
        compute_connections(&mut nodes, &links);
        assert_eq!(nodes[0].connections, 2); // "a": source of both edges
        assert_eq!(nodes[1].connections, 1); // "b": target of one
        assert_eq!(nodes[2].connections, 1); // "c": target of one
    }

    #[test]
    fn compute_connections_node_with_no_edges_stays_zero() {
        let mut nodes = vec![node("isolated")];
        let links: Vec<Edge> = Vec::new();
        compute_connections(&mut nodes, &links);
        assert_eq!(nodes[0].connections, 0);
    }

    #[test]
    fn compute_connections_self_loop_counts_twice() {
        let mut nodes = vec![node("a")];
        let links = vec![edge("a", "a")];
        compute_connections(&mut nodes, &links);
        assert_eq!(nodes[0].connections, 2);
    }

    #[test]
    fn default_colors_has_an_entry_for_every_node_kind() {
        let colors = default_colors();
        for kind in ["atom", "hook", "primitive", "rule", "skill", "agent", "branch", "manifest"] {
            assert!(colors.contains_key(kind), "missing color for kind '{kind}'");
        }
        assert_eq!(colors.len(), 8);
    }
}
