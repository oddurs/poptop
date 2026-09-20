//! Process tree construction.
//!
//! Purely a view over `Sample::procs`: `ppid` is already recorded in every
//! retained sample, so the tree for a sample from forty seconds ago is the real
//! hierarchy from that moment, not a guess reconstructed from the present.
//!
//! The one invariant worth holding onto is that every process appears exactly
//! once. A corrupt or racing `ppid` can produce a cycle, and a process whose
//! parent exited between samples has a `ppid` pointing at nothing; neither may
//! cause a process to vanish from the table or the tree to recurse forever.

use crate::app::Sort;
use crate::sample::ProcSample;
use std::collections::{HashMap, HashSet};

/// One rendered line of the tree.
pub struct TreeRow<'a> {
    /// Borrowed for an ordinary row; owned for a group, whose figures are a sum
    /// that exists in no sample.
    pub proc: std::borrow::Cow<'a, ProcSample>,
    /// Box-drawing prefix, e.g. `"│  ├─ "`. Empty for roots.
    pub prefix: String,
    /// True when this row survives only because it is an ancestor of a filter
    /// match, not because it matched itself.
    pub context_only: bool,
    /// How many processes this row stands for, when it stands for a *name*
    /// rather than a process. `None` for an ordinary row.
    ///
    /// `Some(1)` is a real state and not the same as `None`: while grouping,
    /// every row is keyed on the name it folds, even a name with one process
    /// under it. Deriving group-ness from a count instead made the selection
    /// vanish the moment a pool shrank to one — the row stopped being a group,
    /// and a group selection stopped matching it.
    pub members: Option<usize>,
    /// Set when this row is a *thread* of the process on the row above it.
    ///
    /// `proc` still points at that process, so the row files and sorts under
    /// it; what the renderer draws instead is this. A thread has no memory,
    /// user or command line of its own — it shares its process's — so a row
    /// that borrowed those fields from `proc` would be repeating the line above
    /// it rather than adding to it.
    pub thread: Option<crate::sample::ThreadSample>,
}

impl<'a> TreeRow<'a> {
    /// An ordinary row: one process, no indent, matched on its own account.
    pub fn of(p: &'a ProcSample) -> Self {
        Self {
            proc: std::borrow::Cow::Borrowed(p),
            prefix: String::new(),
            context_only: false,
            members: None,
            thread: None,
        }
    }

    /// Whether this row is a thread rather than a process.
    pub fn is_thread(&self) -> bool {
        self.thread.is_some()
    }

    /// Whether this row stands for a name rather than for one process.
    pub fn is_group(&self) -> bool {
        self.members.is_some()
    }

    /// How many processes it folds. One for an ordinary row.
    pub fn count(&self) -> usize {
        self.members.unwrap_or(1)
    }
}

/// Build the tree for one sample.
///
/// `matched` is `None` when no filter is active. When it is `Some`, the tree
/// keeps every match plus its ancestors — a filtered tree flattened to bare
/// matches loses the parentage that makes it a tree at all.
///
/// Takes references rather than the sample's slice so the caller can withhold
/// processes entirely. A process absent from `procs` is absent from `by_pid`
/// too, so it cannot reappear as somebody's ancestor and its children become
/// roots — which is what hiding kernel threads has to mean, `kthreadd` being
/// the ancestor of every one of them.
pub fn build<'a>(
    procs: &[&'a ProcSample],
    sort: Sort,
    matched: Option<&HashSet<i32>>,
) -> Vec<TreeRow<'a>> {
    let by_pid: HashMap<i32, &'a ProcSample> = procs.iter().map(|p| (p.pid, *p)).collect();

    let visible = matched.map(|m| with_ancestors(m, &by_pid));
    let keep = |pid: i32| visible.as_ref().is_none_or(|v| v.contains(&pid));

    let mut children: HashMap<i32, Vec<&ProcSample>> = HashMap::new();
    let mut roots: Vec<&ProcSample> = Vec::new();

    for p in procs.iter().copied().filter(|p| keep(p.pid)) {
        // A process is a root when its parent is gone from this sample, or when
        // it claims itself as its own parent. Orphans become roots rather than
        // disappearing with the parent that exited.
        let parent_visible = p.ppid != p.pid && by_pid.contains_key(&p.ppid) && keep(p.ppid);
        if parent_visible {
            children.entry(p.ppid).or_default().push(p);
        } else {
            roots.push(p);
        }
    }

    for kids in children.values_mut() {
        kids.sort_by(|a, b| sort.compare(a, b));
    }
    // Roots by their whole branch, not by themselves. On macOS, launchd is a
    // root the kernel will not describe — zero CPU, zero memory — and it is the
    // parent of every process the user owns, while 189 unreadable daemons are
    // roots beside it with the same zeros. Ordered by their own figures they
    // tied, and the tree opened on a page of `?` (0107). A branch's figure is
    // its busiest member's, so the root that holds the work comes first.
    let peak = branch_peaks(&roots, &children, sort);
    roots.sort_by(|a, b| {
        let (pa, pb) = (&peak[&a.pid], &peak[&b.pid]);
        sort.compare(pa, pb).then_with(|| sort.compare(a, b))
    });

    let mut out = Vec::with_capacity(procs.len());
    let mut visited = HashSet::new();
    for (i, root) in roots.iter().enumerate() {
        walk(
            root,
            &children,
            &mut visited,
            &mut out,
            &mut Vec::new(),
            i + 1 == roots.len(),
        );
    }

    // Anything still unvisited is caught in a ppid cycle: every member has a
    // parent that is present, so none of them qualified as a root. Emit them at
    // the top level so a cycle costs correct nesting, never a missing process.
    let stranded: Vec<&'a ProcSample> = procs
        .iter()
        .copied()
        .filter(|p| keep(p.pid) && !visited.contains(&p.pid))
        .collect();
    for p in stranded {
        out.push(TreeRow::of(p));
    }

    if let Some(m) = matched {
        for row in &mut out {
            row.context_only = !m.contains(&row.proc.pid);
        }
    }
    out
}

/// For each root, the member of its branch that sorts first — the busiest
/// process under it, for a resource sort.
///
/// Iterative, and each process visited once, so a deep chain cannot overflow
/// the stack and a cycle cannot loop: anything already seen is not descended
/// into again.
fn branch_peaks<'a>(
    roots: &[&'a ProcSample],
    children: &HashMap<i32, Vec<&'a ProcSample>>,
    sort: Sort,
) -> HashMap<i32, &'a ProcSample> {
    let mut peaks = HashMap::with_capacity(roots.len());
    let mut seen = HashSet::new();
    for &root in roots {
        let mut best = root;
        let mut stack = vec![root];
        while let Some(p) = stack.pop() {
            if !seen.insert(p.pid) {
                continue;
            }
            if sort.compare(p, best).is_lt() {
                best = p;
            }
            if let Some(kids) = children.get(&p.pid) {
                stack.extend(kids.iter().copied());
            }
        }
        peaks.insert(root.pid, best);
    }
    peaks
}

/// Expand a match set to include every ancestor of every match.
fn with_ancestors(matched: &HashSet<i32>, by_pid: &HashMap<i32, &ProcSample>) -> HashSet<i32> {
    let mut keep = matched.clone();
    for &pid in matched {
        let mut cur = pid;
        // Bounded by the number of processes: a ppid cycle would otherwise
        // walk upward forever.
        let mut guard = HashSet::new();
        while guard.insert(cur) {
            let Some(p) = by_pid.get(&cur) else { break };
            if p.ppid == cur || !by_pid.contains_key(&p.ppid) {
                break;
            }
            cur = p.ppid;
            keep.insert(cur);
        }
    }
    keep
}

#[cfg(test)]
fn refs(procs: &[ProcSample]) -> Vec<&ProcSample> {
    procs.iter().collect()
}

fn walk<'a>(
    node: &'a ProcSample,
    children: &HashMap<i32, Vec<&'a ProcSample>>,
    visited: &mut HashSet<i32>,
    out: &mut Vec<TreeRow<'a>>,
    ancestors_last: &mut Vec<bool>,
    is_last: bool,
) {
    if !visited.insert(node.pid) {
        return; // already placed; a cycle led back here
    }

    let prefix = if ancestors_last.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        // Every ancestor above the immediate parent contributes either a
        // continuing spine or blank space — except a root's, whose siblings
        // are other roots and are drawn with no connector at all. A spine
        // there hung in the first column under every root but the last,
        // joining nothing to nothing.
        for (depth, &last) in ancestors_last[..ancestors_last.len() - 1]
            .iter()
            .enumerate()
        {
            s.push_str(if last || depth == 0 { "   " } else { "│  " });
        }
        s.push_str(if is_last { "└─ " } else { "├─ " });
        s
    };

    out.push(TreeRow {
        prefix,
        ..TreeRow::of(node)
    });

    if let Some(kids) = children.get(&node.pid) {
        ancestors_last.push(is_last);
        for (i, kid) in kids.iter().enumerate() {
            walk(
                kid,
                children,
                visited,
                out,
                ancestors_last,
                i + 1 == kids.len(),
            );
        }
        ancestors_last.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn p(pid: i32, ppid: i32, name: &str, cpu: f32) -> ProcSample {
        ProcSample {
            pid,
            ppid,
            name: std::sync::Arc::from(name),
            user: Arc::from("root"),
            cpu,
            rss: 1024,
            threads: Some(1),
            state: 'S',
            started: Some(0),
            cmd: None,
            io: None,
            container: None,
            minflt: None,
            majflt: None,
            vsize: None,
            nice: None,
            pss: None,
        }
    }

    fn names(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|r| format!("{}{}", r.prefix, r.proc.name))
            .collect()
    }

    #[test]
    fn nests_children_under_parents() {
        let procs = vec![
            p(1, 0, "init", 0.0),
            p(2, 1, "sshd", 1.0),
            p(3, 2, "bash", 2.0),
        ];
        let rows = build(&refs(&procs), Sort::Pid, None);
        assert_eq!(names(&rows), vec!["init", "└─ sshd", "   └─ bash"]);
    }

    #[test]
    fn siblings_use_spine_glyphs_and_respect_sort() {
        let procs = vec![
            p(1, 0, "init", 0.0),
            p(2, 1, "low", 1.0),
            p(3, 1, "high", 90.0),
        ];
        // Sorted by CPU descending, so "high" comes first and "low" is last.
        let rows = build(&refs(&procs), Sort::Cpu, None);
        assert_eq!(names(&rows), vec!["init", "├─ high", "└─ low"]);
    }

    #[test]
    fn an_orphan_becomes_a_root() {
        // Parent 999 exited between samples; the child must not vanish with it.
        let procs = vec![p(1, 0, "init", 0.0), p(5, 999, "orphan", 0.0)];
        let rows = build(&refs(&procs), Sort::Pid, None);
        assert_eq!(rows.len(), 2);
        assert!(names(&rows).contains(&"orphan".to_string()));
    }

    #[test]
    fn a_ppid_cycle_neither_hangs_nor_drops_a_process() {
        // 2 and 3 claim each other as parent: neither is a root.
        let procs = vec![p(1, 0, "init", 0.0), p(2, 3, "a", 0.0), p(3, 2, "b", 0.0)];
        let rows = build(&refs(&procs), Sort::Pid, None);
        assert_eq!(rows.len(), 3, "every process must appear exactly once");
    }

    #[test]
    fn a_self_parented_process_is_a_root() {
        let procs = vec![p(1, 1, "weird", 0.0)];
        let rows = build(&refs(&procs), Sort::Pid, None);
        assert_eq!(names(&rows), vec!["weird"]);
    }

    #[test]
    fn every_process_appears_exactly_once() {
        let procs: Vec<ProcSample> = (1..=50)
            .map(|i| {
                p(
                    i,
                    if i == 1 { 0 } else { i / 2 },
                    &format!("p{i}"),
                    i as f32,
                )
            })
            .collect();
        let rows = build(&refs(&procs), Sort::Cpu, None);
        assert_eq!(rows.len(), 50);
        let seen: HashSet<i32> = rows.iter().map(|r| r.proc.pid).collect();
        assert_eq!(seen.len(), 50);
    }

    #[test]
    fn filtering_keeps_ancestors_as_context() {
        let procs = vec![
            p(1, 0, "init", 0.0),
            p(2, 1, "sshd", 0.0),
            p(3, 2, "target", 0.0),
            p(4, 1, "unrelated", 0.0),
        ];
        let matched = HashSet::from([3]);
        let rows = build(&refs(&procs), Sort::Pid, Some(&matched));

        assert_eq!(names(&rows), vec!["init", "└─ sshd", "   └─ target"]);
        // Ancestors are context, the match is not.
        assert!(rows[0].context_only);
        assert!(rows[1].context_only);
        assert!(!rows[2].context_only);
    }

    #[test]
    fn filtering_with_a_cycle_above_the_match_terminates() {
        let procs = vec![p(1, 2, "a", 0.0), p(2, 1, "b", 0.0), p(3, 1, "target", 0.0)];
        let matched = HashSet::from([3]);
        let rows = build(&refs(&procs), Sort::Pid, Some(&matched));
        assert!(rows.iter().any(|r| r.proc.name.as_ref() == "target"));
    }

    #[test]
    fn deep_nesting_indents_cumulatively() {
        let procs: Vec<ProcSample> = (1..=4)
            .map(|i| p(i, i - 1, &format!("d{i}"), 0.0))
            .collect();
        let rows = build(&refs(&procs), Sort::Pid, None);
        assert_eq!(names(&rows), vec!["d1", "└─ d2", "   └─ d3", "      └─ d4"]);
    }
    /// A process the kernel would not describe to this user, as macOS reports
    /// one: no parent it will name, no memory, no thread count, no owner.
    fn unreadable(pid: i32, name: &str) -> ProcSample {
        ProcSample {
            rss: 0,
            threads: None,
            user: Arc::from("?"),
            ..p(pid, 0, name, 0.0)
        }
    }

    #[test]
    fn the_busy_branch_comes_first_whatever_its_root_shows() {
        // 0107. On macOS, sysinfo cannot read another user's processes and
        // gives each a parent of 0, so every root-owned daemon becomes a root
        // — 189 of them — and so does launchd, which is unreadable too and
        // holds every process the user owns. Ordered by their own figures, the
        // 189 zeros and launchd tied, and the tree opened on a page of `?`
        // with the user's processes somewhere below. A root sorts by its whole
        // branch; one the kernel told nothing about sorts after one it did.
        let mut launchd = unreadable(1, "launchd");
        launchd.ppid = 0;
        let procs = vec![
            unreadable(400, "cfprefsd"),
            unreadable(401, "trustd"),
            launchd,
            p(500, 1, "ghostty", 2.0),
            p(501, 500, "cargo", 90.0),
            p(600, 0, "idle-root", 0.0),
            unreadable(402, "backupd"),
        ];
        let rows = build(&refs(&procs), Sort::Cpu, None);
        assert_eq!(
            names(&rows),
            vec![
                "launchd",
                "└─ ghostty",
                "   └─ cargo",
                "idle-root",
                "cfprefsd",
                "trustd",
                "backupd"
            ]
        );
    }
}
