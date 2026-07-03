// The tasks surface over the node's `tasks` module: a committed-state list,
// a small composer, and explicit one-way status advancement.

import { useEffect, useRef, useState } from "react";
import type { CSSProperties, FormEvent } from "react";

import type { Task, TaskStatus } from "../../../domain/tasks-client";
import { Icon } from "../../components/Icon";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius, shadow } from "../../theme/tokens";

const STATUS_ORDER: TaskStatus[] = ["Open", "InProgress", "Done"];

const STATUS_PILLS: Record<
  TaskStatus,
  { label: string; countLabel: string; text: string; bg: string; border: string }
> = {
  Open: {
    label: "Open",
    countLabel: "open",
    text: color.green,
    bg: "#eef5f0",
    border: "#cfe3d7",
  },
  InProgress: {
    label: "In progress",
    countLabel: "in progress",
    text: color.amber,
    bg: "#fbf4e6",
    border: "#ecdcae",
  },
  Done: {
    label: "Done",
    countLabel: "done",
    text: color.purple,
    bg: "#f1edf5",
    border: "#ddd2e6",
  },
};

const inputBase: CSSProperties = {
  width: "100%",
  minWidth: 0,
  height: 36,
  padding: "0 12px",
  borderRadius: radius.sm,
  border: `1px solid ${color.borderStrong}`,
  background: color.paper,
  font: `400 13px ${font.sans}`,
  color: color.ink,
  outline: "none",
};

const shortId = (id: string): string =>
  id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id || "—";

const taskDate = (unixSeconds: number): string =>
  Number.isFinite(unixSeconds) && unixSeconds > 0
    ? new Date(unixSeconds * 1000).toLocaleDateString([], {
        month: "short",
        day: "numeric",
      })
    : "unknown";

const countFor = (tasks: Task[], status: TaskStatus): number =>
  tasks.filter((task) => task.status === status).length;

const advanceCopy = (task: Task): { label: string; ariaLabel: string } => {
  switch (task.status) {
    case "Open":
      return { label: "Start", ariaLabel: `Start ${task.title}` };
    case "InProgress":
      return { label: "Mark done", ariaLabel: `Mark ${task.title} done` };
    case "Done":
      return { label: "Complete", ariaLabel: `${task.title} complete` };
  }
};

function StatusPill({ status }: { status: TaskStatus }) {
  const pill = STATUS_PILLS[status];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        borderRadius: 999,
        border: `1px solid ${pill.border}`,
        background: pill.bg,
        color: pill.text,
        padding: "3px 8px",
        font: `700 9.5px ${font.mono}`,
        whiteSpace: "nowrap",
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          background: pill.text,
          flexShrink: 0,
        }}
      />
      {pill.label}
    </span>
  );
}

function CountPill({ status, count }: { status: TaskStatus; count: number }) {
  const pill = STATUS_PILLS[status];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        borderRadius: 999,
        border: `1px solid ${pill.border}`,
        background: pill.bg,
        color: pill.text,
        padding: "4px 9px",
        font: `600 10.5px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: pill.text }} />
      {count} {pill.countLabel}
    </span>
  );
}

function AdvanceButton({
  task,
  disabled,
  pending,
  onAdvance,
}: {
  task: Task;
  disabled: boolean;
  pending: boolean;
  onAdvance: (taskId: string) => void;
}) {
  const [hover, setHover] = useState(false);
  const copy = advanceCopy(task);
  const active = !disabled && !pending;

  return (
    <button
      type="button"
      aria-label={copy.ariaLabel}
      aria-busy={pending || undefined}
      disabled={disabled}
      onClick={() => onAdvance(task.id)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        minWidth: 92,
        height: 30,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 4,
        borderRadius: radius.sm,
        border: `1px solid ${active ? color.borderStrong : color.borderSoft}`,
        background: active && hover ? color.hover : color.paper,
        color: active ? color.inkSoft : color.muted2,
        cursor: active ? "pointer" : "default",
        font: `600 11px ${font.sans}`,
        whiteSpace: "nowrap",
      }}
    >
      {pending ? "Queued" : copy.label}
      {active ? <Icon name="chevronRight" size={13} strokeWidth={1.9} /> : null}
    </button>
  );
}

function TaskRow({
  task,
  canAdvance,
  pending,
  onAdvance,
}: {
  task: Task;
  canAdvance: boolean;
  pending: boolean;
  onAdvance: (taskId: string) => void;
}) {
  const [hover, setHover] = useState(false);
  const done = task.status === "Done";
  const pill = STATUS_PILLS[task.status];

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 13,
        padding: "13px 16px",
        borderBottom: `1px solid ${color.borderSoft}`,
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
        <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
          <span
            style={{
              font: `600 14px ${font.sans}`,
              color: done ? color.muted3 : color.ink,
              textDecoration: done ? "line-through" : "none",
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
            title={task.title}
          >
            {task.title}
          </span>
          <StatusPill status={task.status} />
        </div>
        <div
          style={{
            marginTop: 5,
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

      {done ? (
        <span
          style={{
            width: 92,
            height: 30,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: radius.sm,
            border: `1px solid ${color.borderSoft}`,
            background: color.sunken,
            color: color.muted2,
            font: `600 11px ${font.sans}`,
            flexShrink: 0,
            whiteSpace: "nowrap",
          }}
        >
          Complete
        </span>
      ) : (
        <AdvanceButton
          task={task}
          disabled={!canAdvance}
          pending={pending}
          onAdvance={onAdvance}
        />
      )}
    </div>
  );
}

function CenterState({
  title,
  detail,
  muted,
}: {
  title: string;
  detail: string;
  muted?: boolean;
}) {
  return (
    <div
      style={{
        minHeight: 280,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 9,
        padding: 24,
        textAlign: "center",
      }}
    >
      <span
        style={{
          width: 36,
          height: 36,
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: muted ? color.sunken : "#eef5f0",
          color: muted ? color.muted : color.green,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="tasks" size={17} strokeWidth={1.7} />
      </span>
      <div style={{ font: `600 14px ${font.sans}`, color: color.muted3 }}>{title}</div>
      <div
        style={{
          maxWidth: 360,
          font: `400 11.5px ${font.sans}`,
          color: color.muted2,
          lineHeight: 1.55,
        }}
      >
        {detail}
      </div>
    </div>
  );
}

export function TasksView() {
  const { state, actions } = useDucktape();
  const [draft, setDraft] = useState("");
  const [inputFocus, setInputFocus] = useState(false);
  const [buttonHover, setButtonHover] = useState(false);
  const [pendingTaskId, setPendingTaskId] = useState<string | null>(null);
  const pendingTimer = useRef<number | null>(null);

  const statusLoaded = state.status !== null;
  const tasksBacked = Boolean(state.status?.modules.some((mod) => mod.id === "tasks"));
  const loading = !statusLoaded;
  const writable = tasksBacked;
  const taskCount = state.tasks.length;
  const canSubmit = writable && draft.trim().length > 0;

  useEffect(
    () => () => {
      if (pendingTimer.current !== null) window.clearTimeout(pendingTimer.current);
    },
    [],
  );

  useEffect(() => {
    if (!pendingTaskId) return;
    const task = state.tasks.find((item) => item.id === pendingTaskId);
    if (!task || task.status === "Done") setPendingTaskId(null);
  }, [pendingTaskId, state.tasks]);

  const add = (event: FormEvent) => {
    event.preventDefault();
    const title = draft.trim();
    if (!writable || !title) return;
    actions.addTask(title);
    setDraft("");
  };

  const advance = (taskId: string) => {
    if (!writable) return;
    const task = state.tasks.find((item) => item.id === taskId);
    if (!task || task.status === "Done") return;
    setPendingTaskId(taskId);
    if (pendingTimer.current !== null) window.clearTimeout(pendingTimer.current);
    pendingTimer.current = window.setTimeout(() => {
      setPendingTaskId((current) => (current === taskId ? null : current));
      pendingTimer.current = null;
    }, 1200);
    actions.advanceTask(taskId);
  };

  return (
    <div
      data-screen-label="Tasks"
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        background: color.paper,
      }}
    >
      <div
        style={{
          minHeight: 56,
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 10,
          padding: "0 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.paper,
        }}
      >
        <span style={{ font: `600 16px ${font.sans}`, color: color.dark }}>Tasks</span>
        <span style={{ font: `400 13px ${font.mono}`, color: color.muted2 }}>
          {taskCount}
        </span>
        <div
          style={{
            marginLeft: "auto",
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            gap: 7,
            flexWrap: "wrap",
          }}
        >
          {STATUS_ORDER.map((status) => (
            <CountPill key={status} status={status} count={countFor(state.tasks, status)} />
          ))}
        </div>
      </div>

      <form
        onSubmit={add}
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "flex-end",
          gap: 10,
          padding: "13px 22px",
          borderBottom: `1px solid ${color.borderSoft}`,
          background: color.sidebar,
        }}
      >
        <label
          htmlFor="task-title"
          style={{
            flex: 1,
            minWidth: 0,
            display: "grid",
            gap: 6,
            font: `700 9px ${font.mono}`,
            letterSpacing: ".08em",
            color: writable ? color.muted2 : color.muted,
          }}
        >
          TASK TITLE
          <input
            id="task-title"
            value={draft}
            disabled={!writable}
            onChange={(event) => setDraft(event.target.value)}
            onFocus={() => setInputFocus(true)}
            onBlur={() => setInputFocus(false)}
            placeholder={loading ? "Loading tasks…" : "Describe a task"}
            style={{
              ...inputBase,
              borderColor: inputFocus ? accentVar : color.borderStrong,
              background: writable ? color.paper : color.sunken,
              color: writable ? color.ink : color.muted2,
            }}
          />
        </label>
        <button
          type="submit"
          aria-label="Add task"
          disabled={!canSubmit}
          onMouseEnter={() => setButtonHover(true)}
          onMouseLeave={() => setButtonHover(false)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            width: 36,
            height: 36,
            borderRadius: radius.sm,
            background: canSubmit ? (buttonHover ? color.dark : accentVar) : color.chip,
            color: canSubmit ? color.paper : color.muted2,
            border: `1px solid ${canSubmit ? "transparent" : color.borderStrong}`,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
            cursor: canSubmit ? "pointer" : "default",
          }}
        >
          <Icon name="plus" size={15} strokeWidth={1.9} />
        </button>
      </form>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 18, background: color.sidebar }}>
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
          {loading ? (
            <CenterState
              title="Loading tasks…"
              detail="Waiting for this node's committed task snapshot."
              muted
            />
          ) : !tasksBacked ? (
            <CenterState
              title="Tasks module is not available"
              detail="This node did not report a tasks module, so task reads and writes are disabled."
              muted
            />
          ) : state.tasks.length === 0 ? (
            <CenterState
              title="No tasks yet"
              detail="Add a task above to start tracking work on this node."
            />
          ) : (
            state.tasks.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
                canAdvance={writable}
                pending={pendingTaskId === task.id}
                onAdvance={advance}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
