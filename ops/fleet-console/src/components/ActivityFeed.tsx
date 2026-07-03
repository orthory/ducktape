import type { FleetActivity } from "../types";

// Agent activity for a worktree. Compact = one line inside a tile; full = the
// commit trail in the drawer. Pluggable: swap the data source (git churn today)
// without touching callers.
export function ActivityFeed({
  activity,
  compact,
}: {
  activity?: FleetActivity;
  compact?: boolean;
}) {
  if (!activity) return null;

  if (compact) {
    const latest = activity.commits[0];
    return (
      <div className="activity-compact">
        {activity.dirty > 0 ? (
          <span className="dirty">✎ {activity.dirty} changed</span>
        ) : (
          <span className="clean">clean</span>
        )}
        {latest && <span className="lastcommit">· {latest.age}</span>}
      </div>
    );
  }

  return (
    <div className="activity-full">
      <div className="activity-title">Activity</div>
      {activity.dirty > 0 && (
        <div className="dirty">✎ {activity.dirty} uncommitted file(s)</div>
      )}
      <ul className="commits">
        {activity.commits.map((c) => (
          <li key={c.sha}>
            <code className="sha">{c.sha}</code>
            <span className="subj">{c.subject}</span>
            <span className="age">{c.age}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
