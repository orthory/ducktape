//! spec-directive parser.
//!
//! a "spec" is just a [`document::Document`]. its task directives are the
//! `/task` nodes inside it — each one becomes a [`TaskSpec`] the orchestrator
//! can hand to an agent.
//!
//! ## agentic trigger convention
//!
//! a document opts into agentic dev via a frontmatter flag: `agentic: true`.
//! frontmatter promotes `title`/`author`/`created_at`/`updated_at` and keeps
//! everything else in its `misc` map, so we look there for the `agentic` key
//! and treat any truthy spelling (`true`/`yes`/`1`, case-insensitive) as on.
//! a spec with no task nodes yields an empty task list even when triggered.

use nodes::Nodes;

/// a single agent-actionable directive extracted from a spec document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    /// the task node's uid (stable identity within the document).
    pub id: uid::Uid,
    /// the task's one-line title.
    pub title: String,
    /// the task's body text (the prose between the open/close markers).
    pub body: String,
}

/// walk the document's nodes and collect one [`TaskSpec`] per `/task` node.
/// non-task nodes (frontmatter, comments, body prose) are ignored.
pub fn parse_spec(doc: &document::Document) -> Vec<TaskSpec> {
    doc.nodes_iter()
        .filter_map(|node| match node {
            Nodes::Task(task) => Some(TaskSpec {
                id: task.uid,
                title: task.title,
                body: task.body,
            }),
            _ => None,
        })
        .collect()
}

/// detect whether the document requests agentic dev.
///
/// convention: a truthy `agentic` key in the frontmatter's `misc` map (see the
/// module docs). returns false when there's no frontmatter or the key is
/// absent / falsy.
pub fn has_agentic_trigger(doc: &document::Document) -> bool {
    doc.nodes_iter().any(|node| match node {
        Nodes::Frontmatter(fm) => fm
            .misc
            .get("agentic")
            .map(|v| is_truthy(v))
            .unwrap_or(false),
        _ => false,
    })
}

fn is_truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(md: &str) -> document::Document {
        document::Document::from_reader(md.trim_start().as_bytes()).expect("doc parses")
    }

    #[test]
    fn parse_spec_extracts_each_task() {
        let doc = parse(
            r#"
---
title: my spec
author: @orthory
created_at: 1
updated_at: 1
---

some intro prose

/task.v1{@orthory;first task;Backlog}
do the first thing
across two lines
/task.v1

/task.v1{@orthory;second task;InProgress}
do the second thing
/task.v1
"#,
        );

        let specs = parse_spec(&doc);
        assert_eq!(specs.len(), 2);

        assert_eq!(specs[0].title, "first task");
        assert_eq!(specs[0].body, "do the first thing\nacross two lines");
        assert!(!specs[0].id.is_nil());

        assert_eq!(specs[1].title, "second task");
        assert_eq!(specs[1].body, "do the second thing");

        // each task carries a distinct uid.
        assert_ne!(specs[0].id, specs[1].id);
    }

    #[test]
    fn parse_spec_ignores_non_task_nodes() {
        let doc = parse(
            r#"
---
title: t
author: @a
created_at: 1
updated_at: 1
---

just prose, no tasks

/comment.v1{@orthory;1;1}
a comment, not a task
/comment.v1
"#,
        );

        assert!(parse_spec(&doc).is_empty());
    }

    #[test]
    fn agentic_trigger_detected_from_frontmatter() {
        let doc = parse(
            r#"
---
title: t
author: @a
created_at: 1
updated_at: 1
agentic: true
---

/task.v1{@orthory;go;Backlog}
work
/task.v1
"#,
        );

        assert!(has_agentic_trigger(&doc));
    }

    #[test]
    fn agentic_trigger_absent_by_default() {
        let doc = parse(
            r#"
---
title: t
author: @a
created_at: 1
updated_at: 1
---

/task.v1{@orthory;go;Backlog}
work
/task.v1
"#,
        );

        assert!(!has_agentic_trigger(&doc));
    }

    #[test]
    fn agentic_trigger_falsy_value_is_off() {
        let doc = parse(
            r#"
---
title: t
author: @a
created_at: 1
updated_at: 1
agentic: false
---
"#,
        );

        assert!(!has_agentic_trigger(&doc));
    }
}
