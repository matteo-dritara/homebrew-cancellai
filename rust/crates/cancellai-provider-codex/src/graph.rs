//! Codex subagent/rollout graph (ported from `cancellai.py`'s `choose_codex_old_sessions`'
//! `root_id_for`/grouping logic, E05-S04 AC1: "Root/subagent trees are preserved as graph
//! relationships.").
//!
//! Scope note: only the graph-*building* half is ported here. `choose_codex_old_sessions`
//! itself also selects which trees are old enough to act on (`cutoff`/`keep_latest`,
//! deduplicating by resolved filesystem path, choosing between "one action per root" and
//! "one action per rollout file" depending on native-delete support) - that is PLAN-stage
//! action-selection scope a CLASSIFY-stage adapter story does not implement (see this crate's
//! module doc and `docs/architecture/TARGET.md`'s OBSERVE→CLASSIFY→RESOLVE→PLAN pipeline).

use std::collections::{HashMap, HashSet};

use crate::session::CodexSession;

/// One root-rooted subagent tree: `root_id` (the thread id at the top of the chain) plus every
/// discovered session that resolves to it, including the root's own rollout when discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentTree {
    pub root_id: String,
    pub members: Vec<CodexSession>,
}

/// Walks `session_id`'s `parent_session_id` chain to find its tree's root, exactly matching
/// `cancellai.py`'s `root_id_for`: a session with no parent, or whose parent was not itself
/// discovered (an independent safety unit - its true root is unknown, not assumed), is its own
/// root; a cycle (malformed/cyclic metadata) isolates the *original* session rather than
/// over-grouping it into a false tree.
fn root_id_for(session_id: &str, by_id: &HashMap<&str, &CodexSession>) -> String {
    let mut current = session_id.to_string();
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        if !seen.insert(current.clone()) {
            // Cycle: isolate the id this call actually started from, not wherever the cycle
            // was detected.
            return session_id.to_string();
        }
        let Some(session) = by_id.get(current.as_str()) else {
            return current;
        };
        let Some(parent) = &session.parent_session_id else {
            return current;
        };
        if !by_id.contains_key(parent.as_str()) {
            return current;
        }
        current = parent.clone();
    }
}

/// Groups `sessions` into root-rooted [`SubagentTree`]s. Every discovered session appears in
/// exactly one tree (grouped by its own `root_id_for` result), even when duplicate copies of
/// the same `session_id` exist across `sessions/`/`archived_sessions/` - both copies resolve to
/// the same root and land in the same tree, matching `cancellai.py`'s own conservative
/// duplicate handling (never dropping a real file from consideration).
pub fn group_into_subagent_trees(sessions: &[CodexSession]) -> Vec<SubagentTree> {
    let by_id: HashMap<&str, &CodexSession> = sessions
        .iter()
        .map(|session| (session.session_id.as_str(), session))
        .collect();

    let mut order: Vec<String> = Vec::new();
    let mut trees: HashMap<String, Vec<CodexSession>> = HashMap::new();
    for session in sessions {
        let root_id = root_id_for(&session.session_id, &by_id);
        if !trees.contains_key(&root_id) {
            order.push(root_id.clone());
        }
        trees.entry(root_id).or_default().push(session.clone());
    }

    order
        .into_iter()
        .map(|root_id| {
            let members = trees.remove(&root_id).unwrap_or_default();
            SubagentTree { root_id, members }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::RolloutCategory;
    use std::path::PathBuf;

    fn session(id: &str, parent: Option<&str>) -> CodexSession {
        CodexSession {
            category: RolloutCategory::Session,
            path: PathBuf::from(format!("/root/sessions/rollout-{id}.jsonl")),
            session_id: id.to_string(),
            parent_session_id: parent.map(str::to_string),
            size_bytes: 10,
        }
    }

    #[test]
    fn ac1_a_root_with_two_children_is_one_tree_of_three_members() {
        let sessions = vec![
            session("33333333-3333-4333-8333-333333333333", None),
            session(
                "33333333-3333-4333-8333-333333333334",
                Some("33333333-3333-4333-8333-333333333333"),
            ),
            session(
                "33333333-3333-4333-8333-333333333335",
                Some("33333333-3333-4333-8333-333333333333"),
            ),
        ];
        let trees = group_into_subagent_trees(&sessions);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].root_id, "33333333-3333-4333-8333-333333333333");
        assert_eq!(trees[0].members.len(), 3);
    }

    #[test]
    fn a_session_with_no_parent_is_its_own_single_member_tree() {
        let sessions = vec![session("22222222-2222-4222-8222-222222222222", None)];
        let trees = group_into_subagent_trees(&sessions);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].members.len(), 1);
    }

    #[test]
    fn a_parent_that_was_never_discovered_makes_the_child_its_own_root() {
        // The parent id is not among `sessions` at all - an independent safety unit, per the
        // Python reference's own comment: "Parent is not locally discoverable."
        let sessions = vec![session(
            "44444444-4444-4444-8444-444444444444",
            Some("00000000-0000-4000-8000-000000000000"),
        )];
        let trees = group_into_subagent_trees(&sessions);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].root_id, "44444444-4444-4444-8444-444444444444");
    }

    #[test]
    fn a_cycle_isolates_the_original_session_rather_than_looping_forever() {
        let sessions = vec![
            session(
                "11111111-1111-4111-8111-111111111111",
                Some("22222222-2222-4222-8222-222222222222"),
            ),
            session(
                "22222222-2222-4222-8222-222222222222",
                Some("11111111-1111-4111-8111-111111111111"),
            ),
        ];
        let trees = group_into_subagent_trees(&sessions);
        // Both sessions are cyclic; each is isolated as its own root rather than the function
        // looping forever or silently merging them into one arbitrary tree.
        assert_eq!(trees.len(), 2);
        let root_ids: Vec<&str> = trees.iter().map(|t| t.root_id.as_str()).collect();
        assert!(root_ids.contains(&"11111111-1111-4111-8111-111111111111"));
        assert!(root_ids.contains(&"22222222-2222-4222-8222-222222222222"));
    }

    #[test]
    fn duplicate_copies_of_the_same_session_id_land_in_the_same_tree() {
        let sessions = vec![
            session("55555555-5555-4555-8555-555555555555", None),
            session("55555555-5555-4555-8555-555555555555", None),
        ];
        let trees = group_into_subagent_trees(&sessions);
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].members.len(), 2);
    }
}
