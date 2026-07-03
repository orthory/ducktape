// The tasks surface over the node's `tasks` module: a compact committed-state
// list plus an add-task composer. A task advances one way (Open → InProgress →
// Done) via UpdateStatus — click a row to move it along.

import { useState } from "react";
import type { CSSProperties, FormEvent } from "react";

import type { Task, TaskStatus } from "../../../domain/tasks-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const STATUS_PILLS: Record<
  TaskStatus,
  { label: string; text: string; bg: string; border: string }
> = {
  Open: { label: "Open", text: "#5f9e74", bg: "#eef5f0", border: "#cfe3d7" },
  InProgress: {
    label: "In progress",
    text: "#a07b32",
    bg: "#fbf4e6",
    border: "#ecdcae",
  },
  Done: { label: "Done", text: "#7a6f9e", bg: "#f1edf5", border: "#ddd2e6" },
};

const inputStyle: CSSProperties = {
  flex: 1,
  minWidth: 0,
  padding: "9px 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 13px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const shortId = (id: string): string => (id.length > 10 ? `${id.slice(0, 10)}…` : id);

const taskDate = (unixSeconds: number): string =>
  new Date(unixSeconds * 1000).toLocaleDateString([], {
    month: "short",
    day: "numeric",
  });

const countFor = (tasks: Task[], status: TaskStatus): number =>
  tasks.filter((task) => task.status === status).length;

const advanceLabel = (status: TaskStatus): string => {
  switch (status) {
    case "Open":
      return "Start";
    case "InProgress":
      return "Finish";
    case "Done":
      return "Done";
  }
};

function StatusPill({ status }: { status: TaskStatus }) {
  const pill = STATUS_PILLS[status];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: 999,
        border: `1px solid ${pill.border}`,
        background: pill.bg,
        color: pill.text,
        padding: "3px 8px",
        font: `600 10.5px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {pill.label}
    </span>
  );
}

function TaskRow({ task, onAdvance }: { task: Task; onAdvance: (id: string) => void }) {
  const [hover, setHover] = useState(false);
  const done = task.status === "Done";
  const pill = STATUS_PILLS[task.status];

  return (
    <button
      type="button"
      onClick={() => onAdvance(task.id)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      title={done ? "Task is done" : "Advance task"}
      aria-label={`${advanceLabel(task.status)} task: ${task.title}`}
      style={{
        all: "unset",
        boxSizing: "border-box",
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: 13,
        padding: "13px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
        cursor: "pointer",
        background: hover ? color.sidebar : "transparent",
      }}
    >
      <span
        style={{
          width: 30,
          height: 30,
          borderRadius: radius.sm,
          border: `1px solid ${pill.border}`,
          background: pill.bg,
          color: pill.text,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
        }}
      >
        <Icon name={done ? "check" : "tasks"} size={15} strokeWidth={1.7} />
      </span>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <StatusPill status={task.status} />
          <span
            style={{
              font: `600 14px ${font.sans}`,
              color: done ? color.muted3 : color.ink,
              textDecoration: done ? "line-through" : "none",
              overflowWrap: "anywhere",
            }}
          >
            {task.title}
          </span>
        </div>
        <div
          style={{
            marginTop: 4,
            font: `400 11px ${font.mono}`,
            color: color.muted2,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          #{shortId(task.id)} · created {taskDate(task.created_at)}
        </div>
      </div>

      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 3,
          flexShrink: 0,
          font: `600 11px ${font.sans}`,
          color: done ? color.muted2 : hover ? color.ink : color.muted3,
          whiteSpace: "nowrap",
        }}
      >
        {advanceLabel(task.status)}
        {!done && <Icon name="chevronRight" size={14} strokeWidth={1.9} />}
      </span>
    </button>
  );
}

function EmptyState() {
  return (
    <div
      style={{
        flex: 1,
        minHeight: 220,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 8,
        color: color.muted2,
      }}
    >
      <span
        style={{
          width: 34,
          height: 34,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.sunken,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: color.muted,
        }}
      >
        <Icon name="tasks" size={17} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>No tasks yet</div>
      <div style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>
        Nothing is queued for this node.
      </div>
    </div>
  );
}

export function TasksView() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const taskCount = state.tasks.length;
  const canSubmit = draft.trim().length > 0;

  const add = (event: FormEvent) => {
    event.preventDefault();
    actions.addTask(draft);
    setDraft("");
  };

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          padding: "15px 18px 14px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "baseline", gap: 9 }}>
            <span style={{ font: `700 18px ${font.sans}`, color: color.ink }}>Tasks</span>
            <span style={{ font: `500 11px ${font.mono}`, color: color.muted2 }}>
              {taskCount} {taskCount === 1 ? "TASK" : "TASKS"}
            </span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 7, flexShrink: 0 }}>
            {(Object.keys(STATUS_PILLS) as TaskStatus[]).map((status) => (
              <span
                key={status}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 5,
                  font: `500 10.5px ${font.sans}`,
                  color: STATUS_PILLS[status].text,
                  whiteSpace: "nowrap",
                }}
              >
                <span
                  style={{
                    width: 6,
                    height: 6,
                    borderRadius: "50%",
                    background: STATUS_PILLS[status].text,
                  }}
                />
                <span style={{ font: `500 10.5px ${font.mono}` }}>
                  {countFor(state.tasks, status)}
                </span>
              </span>
            ))}
          </div>
        </div>

        <form onSubmit={add} style={{ display: "flex", gap: 8, marginTop: 13 }}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="New task"
            style={inputStyle}
          />
          <button
            type="submit"
            disabled={!canSubmit}
            title="Add task"
            style={{
              all: "unset",
              cursor: canSubmit ? "pointer" : "default",
              width: 34,
              height: 34,
              borderRadius: radius.sm,
              background: canSubmit ? accentVar : color.chip,
              color: canSubmit ? color.paper : color.muted2,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Icon name="plus" size={15} strokeWidth={1.9} />
          </button>
        </form>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: 18, background: color.sidebar }}>
        <div
          style={{
            minHeight: "100%",
            borderRadius: radius.lg,
            border: `1px solid ${color.border}`,
            background: color.paper,
            boxShadow: shadow.card,
            overflow: "hidden",
          }}
        >
          {state.tasks.length === 0 ? (
            <EmptyState />
          ) : (
            state.tasks.map((task) => (
              <TaskRow key={task.id} task={task} onAdvance={actions.advanceTask} />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
