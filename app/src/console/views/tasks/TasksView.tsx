// The tasks surface over the node's `tasks` module: three status lanes and an
// add-task composer. A task advances one way (Open → InProgress → Done) via
// UpdateStatus — click a card to move it along.

import { useState } from "react";
import type { FormEvent } from "react";

import type { Task, TaskStatus } from "../../../domain/tasks-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const LANES: { status: TaskStatus; label: string; tint: string }[] = [
  { status: "Open", label: "Open", tint: color.blue },
  { status: "InProgress", label: "In progress", tint: color.amber },
  { status: "Done", label: "Done", tint: color.green },
];

const dayOf = (millis: number): string =>
  new Date(millis).toLocaleDateString([], { month: "short", day: "numeric" });

function TaskCard({ task, onAdvance }: { task: Task; onAdvance: (id: string) => void }) {
  const done = task.status === "Done";
  return (
    <button
      onClick={() => onAdvance(task.id)}
      title={done ? undefined : "Advance status"}
      style={{
        all: "unset",
        cursor: done ? "default" : "pointer",
        display: "flex",
        flexDirection: "column",
        gap: 4,
        padding: "9px 11px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        background: color.paper,
        boxShadow: shadow.card,
        animation: "ik-fade .16s ease-out",
      }}
    >
      <span
        style={{
          font: `500 12.5px ${font.sans}`,
          color: done ? color.muted2 : color.ink,
          textDecoration: done ? "line-through" : "none",
          wordBreak: "break-word",
        }}
      >
        {task.title}
      </span>
      <span style={{ font: `400 10px ${font.mono}`, color: color.muted2 }}>
        {dayOf(task.created_at)}
      </span>
    </button>
  );
}

export function TasksView() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");

  const add = (event: FormEvent) => {
    event.preventDefault();
    actions.addTask(draft);
    setDraft("");
  };

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Tasks</span>
        <form onSubmit={add} style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <input
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="New task"
            style={{
              width: 220,
              padding: "6px 10px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderStrong}`,
              background: color.paper,
              font: `400 12px ${font.sans}`,
              color: color.ink,
            }}
          />
          <button
            type="submit"
            title="Add task"
            style={{
              all: "unset",
              cursor: "pointer",
              width: 26,
              height: 26,
              borderRadius: 7,
              background: draft.trim() ? accentVar : color.chip,
              color: "#fff",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Icon name="plus" size={14} />
          </button>
        </form>
      </div>

      <div style={{ flex: 1, display: "flex", gap: 13, padding: 17, overflowX: "auto" }}>
        {LANES.map((lane) => {
          const cards = state.tasks.filter((task) => task.status === lane.status);
          return (
            <div
              key={lane.status}
              style={{
                width: 260,
                flexShrink: 0,
                display: "flex",
                flexDirection: "column",
                gap: 8,
                padding: 11,
                borderRadius: radius.md,
                background: color.sunken,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span
                  style={{ width: 7, height: 7, borderRadius: "50%", background: lane.tint }}
                />
                <span
                  style={{
                    font: `600 11px ${font.sans}`,
                    color: color.muted3,
                    letterSpacing: ".03em",
                  }}
                >
                  {lane.label}
                </span>
                <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted2 }}>
                  {cards.length}
                </span>
              </div>
              {cards.map((task) => (
                <TaskCard key={task.id} task={task} onAdvance={actions.advanceTask} />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
