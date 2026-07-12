// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 <author org>
//
//! Heartbeat record + query filter. One file, no dependencies on backends.

use serde::{Deserialize, Serialize};

/// One agent's "I'm alive, doing X" record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub agent_id: String,            // unique per worktree / session
    pub session_id: Option<String>,  // CLAUDE_SESSION_ID if known
    pub phase: String,               // free-form: "wave-7-auth-providers", "merge-ceremony", etc.
    pub dna: Option<String>,         // active DNA serial (RULE 0.12)
    pub branch: Option<String>,      // git branch the agent is on
    pub cwd: Option<String>,         // working directory
    pub last_seen_epoch: u64,        // seconds since UNIX epoch
    pub note: Option<String>,        // optional human-readable status
}

#[derive(Debug, Clone, Default)]
pub struct PingFilter {
    /// Only return heartbeats newer than this many seconds (TTL filter).
    /// Default 90s.
    pub max_age_s: Option<u64>,
    /// Only return heartbeats matching this phase prefix.
    pub phase_prefix: Option<String>,
    /// Only return heartbeats with branch matching exactly.
    pub branch: Option<String>,
}

impl PingFilter {
    pub fn alive(&self, h: &Heartbeat, now: u64) -> bool {
        let max = self.max_age_s.unwrap_or(90);
        if now.saturating_sub(h.last_seen_epoch) > max {
            return false;
        }
        if let Some(p) = &self.phase_prefix {
            if !h.phase.starts_with(p.as_str()) {
                return false;
            }
        }
        if let Some(b) = &self.branch {
            if h.branch.as_deref() != Some(b.as_str()) {
                return false;
            }
        }
        true
    }
}

pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hb(phase: &str, branch: Option<&str>, last_seen_epoch: u64) -> Heartbeat {
        Heartbeat {
            agent_id: "a1".into(),
            session_id: None,
            phase: phase.into(),
            dna: None,
            branch: branch.map(String::from),
            cwd: None,
            last_seen_epoch,
            note: None,
        }
    }

    #[test]
    fn alive_default_ttl_is_90s() {
        let f = PingFilter::default();
        assert!(f.alive(&hb("p", None, 100), 189)); // 89s old, under 90
        assert!(!f.alive(&hb("p", None, 100), 191)); // 91s old, over 90
    }

    #[test]
    fn alive_respects_custom_max_age() {
        let f = PingFilter { max_age_s: Some(10), ..Default::default() };
        assert!(f.alive(&hb("p", None, 100), 109));
        assert!(!f.alive(&hb("p", None, 100), 111));
    }

    #[test]
    fn alive_phase_prefix_matches_start_only() {
        let f = PingFilter { phase_prefix: Some("wave-7".into()), ..Default::default() };
        assert!(f.alive(&hb("wave-7-auth", None, 100), 100));
        assert!(!f.alive(&hb("wave-8-auth", None, 100), 100));
        assert!(!f.alive(&hb("auth-wave-7", None, 100), 100));
    }

    #[test]
    fn alive_branch_must_match_exactly() {
        let f = PingFilter { branch: Some("main".into()), ..Default::default() };
        assert!(f.alive(&hb("p", Some("main"), 100), 100));
        assert!(!f.alive(&hb("p", Some("main-2"), 100), 100));
        assert!(!f.alive(&hb("p", None, 100), 100));
    }

    #[test]
    fn alive_combines_all_filters_with_and() {
        let f = PingFilter {
            max_age_s: Some(50),
            phase_prefix: Some("wave-7".into()),
            branch: Some("main".into()),
        };
        // Passes phase+branch but fails TTL.
        assert!(!f.alive(&hb("wave-7-auth", Some("main"), 100), 200));
        // Passes TTL+branch but fails phase.
        assert!(!f.alive(&hb("wave-8-auth", Some("main"), 100), 120));
        // Passes all three.
        assert!(f.alive(&hb("wave-7-auth", Some("main"), 100), 120));
    }
}
