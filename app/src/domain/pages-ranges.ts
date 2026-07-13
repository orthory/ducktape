import type { InlineMark, RelativeAnchor, SpanMark } from "./pages-client";

interface TextEdit {
  start: number;
  oldEnd: number;
  newEnd: number;
}

const editBetween = (oldText: string, newText: string): TextEdit | null => {
  if (oldText === newText) return null;
  const oldChars = Array.from(oldText);
  const newChars = Array.from(newText);
  let prefix = 0;
  while (
    prefix < oldChars.length &&
    prefix < newChars.length &&
    oldChars[prefix] === newChars[prefix]
  ) prefix += 1;
  let suffix = 0;
  while (
    suffix < oldChars.length - prefix &&
    suffix < newChars.length - prefix &&
    oldChars[oldChars.length - 1 - suffix] === newChars[newChars.length - 1 - suffix]
  ) suffix += 1;
  const start = oldChars.slice(0, prefix).join("").length;
  return {
    start,
    oldEnd: start + oldChars.slice(prefix, oldChars.length - suffix).join("").length,
    newEnd: start + newChars.slice(prefix, newChars.length - suffix).join("").length,
  };
};

const mapStart = (pos: number, edit: TextEdit): number =>
  pos < edit.start
    ? pos
    : pos >= edit.oldEnd
      ? pos + edit.newEnd - edit.oldEnd
      : edit.start;

const mapEnd = (pos: number, edit: TextEdit): number =>
  pos <= edit.start
    ? pos
    : pos >= edit.oldEnd
      ? pos + edit.newEnd - edit.oldEnd
      : edit.newEnd;

const rebase = <T extends RelativeAnchor>(range: T, edit: TextEdit, length: number): T => {
  if (range.start === range.end) {
    const pos = Math.min(
      length,
      range.start <= edit.start
        ? range.start
        : range.start >= edit.oldEnd
          ? range.start + edit.newEnd - edit.oldEnd
          : edit.newEnd,
    );
    return { ...range, start: pos, end: pos };
  }
  const start = Math.min(length, mapStart(range.start, edit));
  return { ...range, start, end: Math.max(start, Math.min(length, mapEnd(range.end, edit))) };
};

export const rebaseRange = (
  oldText: string,
  newText: string,
  range: RelativeAnchor,
): RelativeAnchor => {
  const edit = editBetween(oldText, newText);
  return edit ? rebase(range, edit, newText.length) : range;
};

const normalize = (marks: SpanMark[]): SpanMark[] => {
  const sorted = marks
    .filter((mark) => mark.start < mark.end)
    .sort((a, b) => a.kind.localeCompare(b.kind) || a.start - b.start || a.end - b.end);
  const out: SpanMark[] = [];
  for (const mark of sorted) {
    const last = out[out.length - 1];
    if (last && last.kind === mark.kind && mark.start <= last.end) {
      last.end = Math.max(last.end, mark.end);
    } else {
      out.push({ ...mark });
    }
  }
  return out;
};

export const rebaseMarks = (
  oldText: string,
  newText: string,
  marks: SpanMark[] = [],
): SpanMark[] => {
  const edit = editBetween(oldText, newText);
  return edit ? normalize(marks.map((mark) => rebase(mark, edit, newText.length))) : marks;
};

export const applySpanMark = (
  marks: SpanMark[] = [],
  range: RelativeAnchor,
  kind: InlineMark,
  active: boolean,
): SpanMark[] =>
  normalize(
    active
      ? [...marks, { ...range, kind }]
      : marks.flatMap((mark): SpanMark[] => {
          if (mark.kind !== kind || mark.end <= range.start || mark.start >= range.end) {
            return [mark];
          }
          return [
            ...(mark.start < range.start ? [{ ...mark, end: range.start }] : []),
            ...(mark.end > range.end ? [{ ...mark, start: range.end }] : []),
          ];
        }),
  );

export const rangeHasMark = (
  marks: SpanMark[] = [],
  range: RelativeAnchor,
  kind: InlineMark,
): boolean => {
  let covered = range.start;
  for (const mark of normalize(marks.filter((mark) => mark.kind === kind))) {
    if (mark.end <= covered) continue;
    if (mark.start > covered) return false;
    covered = mark.end;
    if (covered >= range.end) return true;
  }
  return false;
};

export const splitMarks = (
  marks: SpanMark[] = [],
  at: number,
): { left: SpanMark[]; right: SpanMark[] } => ({
  left: normalize(
    marks.filter((mark) => mark.start < at).map((mark) => ({ ...mark, end: Math.min(mark.end, at) })),
  ),
  right: normalize(
    marks.filter((mark) => mark.end > at).map((mark) => ({
      ...mark,
      start: Math.max(mark.start, at) - at,
      end: mark.end - at,
    })),
  ),
});

export const mergeMarks = (
  left: SpanMark[] = [],
  right: SpanMark[] = [],
  offset: number,
): SpanMark[] => normalize([
  ...left,
  ...right.map((mark) => ({ ...mark, start: mark.start + offset, end: mark.end + offset })),
]);
