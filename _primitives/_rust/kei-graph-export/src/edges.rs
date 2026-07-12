use crate::nodes::{AgentInfo, ManifestInfo};
use crate::types::{sanitize_id, Edge};
use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;

// Edge rule 1: manifest → block
pub fn edges_manifest_block(
    manifests: &[ManifestInfo],
    lookup: &HashMap<String, String>,
    links: &mut Vec<Edge>,
) {
    for m in manifests {
        for name in &m.blocks {
            if let Some(id) = lookup.get(name.as_str()) {
                links.push(edge(&m.node_id, id, "block_dep", 1.0));
            }
        }
    }
}

// Edge rule 2: manifest → path-atom
pub fn edges_manifest_path_ref(
    manifests: &[ManifestInfo],
    lookup: &HashMap<String, String>,
    links: &mut Vec<Edge>,
) {
    for m in manifests {
        for name in &m.path_refs {
            if let Some(id) = lookup.get(name.as_str()) {
                links.push(edge(&m.node_id, id, "path_ref", 0.5));
            }
        }
    }
}

// Edge rule 3: manifest → rule block
pub fn edges_manifest_rule(
    manifests: &[ManifestInfo],
    lookup: &HashMap<String, String>,
    links: &mut Vec<Edge>,
) {
    for m in manifests {
        for name in &m.rule_blocks {
            if let Some(id) = lookup.get(name.as_str()) {
                links.push(edge(&m.node_id, id, "rule_dep", 0.7));
            }
        }
    }
}

// Edge rule 4: branch lineage
pub fn edges_branch_lineage(led: &Connection, links: &mut Vec<Edge>) -> Result<()> {
    let mut stmt = led.prepare(
        "SELECT branch, parent_branch FROM agents \
         WHERE branch IS NOT NULL AND parent_branch IS NOT NULL AND parent_branch != ''",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (branch, parent) = row?;
        links.push(edge(
            &format!("branch::{}", sanitize_id(&parent)),
            &format!("branch::{}", sanitize_id(&branch)),
            "branch_lineage", 1.0,
        ));
    }
    Ok(())
}

// Edge rule 5: agent fork
pub fn edges_agent_fork(agents: &[AgentInfo], links: &mut Vec<Edge>) {
    for a in agents {
        if let Some(pid) = &a.fork_parent_id {
            links.push(edge(&sanitize_id(pid), &a.node_id, "agent_fork", 1.0));
        }
    }
}

// Edge rule 6: skill invocation
pub fn edges_skill_invocation(led: &Connection, links: &mut Vec<Edge>) -> Result<()> {
    let mut stmt = led.prepare(
        "SELECT agent_id, skill_name FROM skill_invocations \
         WHERE agent_id IS NOT NULL AND skill_name IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (agent_id, skill_name) = row?;
        links.push(edge(
            &sanitize_id(&agent_id),
            &format!("skill::{}", sanitize_id(&skill_name)),
            "skill_run", 0.5,
        ));
    }
    Ok(())
}

// Edge rule 7: agent → manifest (heuristic)
pub fn edges_agent_manifest(
    agents: &[AgentInfo],
    manifests: &[ManifestInfo],
    links: &mut Vec<Edge>,
) {
    for a in agents {
        let slug = subagent_slug(&a.branch);
        if let Some(m) = manifests.iter().find(|m| m.name == slug) {
            links.push(edge(&a.node_id, &m.node_id, "agent_uses_manifest", 1.0));
        } else if !slug.is_empty() {
            eprintln!("warn: no manifest for slug '{}' (branch: {})", slug, a.branch);
        }
    }
}

fn subagent_slug(branch: &str) -> String {
    let part = branch.split('/').next_back().unwrap_or(branch);
    let stripped = strip_trailing_digits_and_dashes(part);
    let stripped = stripped.strip_prefix("inline-").unwrap_or(stripped);
    stripped.to_string()
}

fn strip_trailing_digits_and_dashes(s: &str) -> &str {
    let mut end = s.len();
    let bytes = s.as_bytes();
    while end > 0 && (bytes[end - 1].is_ascii_digit() || bytes[end - 1] == b'-') {
        end -= 1;
    }
    s[..end].trim_end_matches('-')
}

fn edge(src: &str, tgt: &str, kind: &str, weight: f32) -> Edge {
    Edge {
        source: src.to_string(),
        target: tgt.to_string(),
        kind: kind.to_string(),
        weight,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_trailing_digits_and_dashes_strips_numeric_suffix() {
        assert_eq!(strip_trailing_digits_and_dashes("code-implementer-42"), "code-implementer");
        assert_eq!(strip_trailing_digits_and_dashes("researcher-7"), "researcher");
    }

    #[test]
    fn strip_trailing_digits_and_dashes_no_suffix_is_unchanged() {
        assert_eq!(strip_trailing_digits_and_dashes("plain-name"), "plain-name");
    }

    #[test]
    fn strip_trailing_digits_and_dashes_all_digits_is_empty() {
        assert_eq!(strip_trailing_digits_and_dashes("12345"), "");
    }

    #[test]
    fn subagent_slug_takes_last_path_segment() {
        assert_eq!(subagent_slug("agents/code-implementer-42"), "code-implementer");
    }

    #[test]
    fn subagent_slug_strips_inline_prefix_after_digits() {
        assert_eq!(subagent_slug("agents/inline-researcher-7"), "researcher");
    }

    #[test]
    fn subagent_slug_no_path_separator_still_works() {
        assert_eq!(subagent_slug("critic-3"), "critic");
    }

    #[test]
    fn edges_manifest_block_links_known_names_skips_unknown() {
        let manifests = vec![ManifestInfo {
            node_id: "m1".into(),
            name: "code-implementer".into(),
            blocks: vec!["baseline".into(), "missing-block".into()],
            path_refs: vec![],
            rule_blocks: vec![],
        }];
        let mut lookup = HashMap::new();
        lookup.insert("baseline".to_string(), "block::baseline".to_string());

        let mut links = Vec::new();
        edges_manifest_block(&manifests, &lookup, &mut links);

        assert_eq!(links.len(), 1, "unknown block name should be silently skipped");
        assert_eq!(links[0].source, "m1");
        assert_eq!(links[0].target, "block::baseline");
        assert_eq!(links[0].kind, "block_dep");
    }

    #[test]
    fn edges_agent_fork_only_emits_edge_when_parent_present() {
        let agents = vec![
            AgentInfo { node_id: "a1".into(), branch: "b1".into(), fork_parent_id: Some("p1".into()) },
            AgentInfo { node_id: "a2".into(), branch: "b2".into(), fork_parent_id: None },
        ];
        let mut links = Vec::new();
        edges_agent_fork(&agents, &mut links);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source, "p1");
        assert_eq!(links[0].target, "a1");
        assert_eq!(links[0].kind, "agent_fork");
    }

    #[test]
    fn edges_agent_manifest_matches_by_derived_slug() {
        let agents = vec![AgentInfo {
            node_id: "a1".into(),
            branch: "agents/code-implementer-42".into(),
            fork_parent_id: None,
        }];
        let manifests = vec![ManifestInfo {
            node_id: "m1".into(),
            name: "code-implementer".into(),
            blocks: vec![],
            path_refs: vec![],
            rule_blocks: vec![],
        }];
        let mut links = Vec::new();
        edges_agent_manifest(&agents, &manifests, &mut links);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source, "a1");
        assert_eq!(links[0].target, "m1");
        assert_eq!(links[0].kind, "agent_uses_manifest");
    }

    #[test]
    fn edges_agent_manifest_no_match_emits_nothing() {
        let agents = vec![AgentInfo {
            node_id: "a1".into(),
            branch: "agents/unknown-role-1".into(),
            fork_parent_id: None,
        }];
        let manifests = vec![ManifestInfo {
            node_id: "m1".into(),
            name: "code-implementer".into(),
            blocks: vec![],
            path_refs: vec![],
            rule_blocks: vec![],
        }];
        let mut links = Vec::new();
        edges_agent_manifest(&agents, &manifests, &mut links);
        assert!(links.is_empty());
    }
}
