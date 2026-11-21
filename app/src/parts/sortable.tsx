// Ordering by drag and drop, wherever Rosson made the office type order numbers: a list whose
// rows move by their grip (⠿) — with a mouse, a finger (the grip alone takes the touch, the page
// still scrolls), or the keyboard (Tab to the grip, Space, ↑↓, Space; Esc cancels) — and say so to
// screen readers in the office's language.

import { createContext, useContext, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { closestCenter, DndContext, DragOverlay, KeyboardSensor, PointerSensor, useSensor, useSensors, type Announcements, type DragEndEvent, type DragMoveEvent } from '@dnd-kit/core';
import { SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy, type SortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { useT } from '../i18n';

function useListSensors() {
  return useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));
}

/** The rows `ids`, in their order; `onMove`: a row dropped at another place (`to` from 0). */
export function SortableList({ ids, onMove, children }: { ids: string[]; onMove: (id: string, to: number) => void; children: ReactNode }) {
  const t = useT();
  const place = (id: string | number | undefined) => (id === undefined ? '' : String(ids.indexOf(String(id)) + 1));
  const announcements: Announcements = {
    onDragStart: ({ active }) => `${t('dnd_picked')} ${place(active.id)}.`,
    onDragOver: ({ over }) => (over ? `${t('dnd_over')} ${place(over.id)}.` : ''),
    onDragEnd: ({ over }) => (over ? `${t('dnd_dropped')} ${place(over.id)}.` : t('dnd_cancelled')),
    onDragCancel: () => t('dnd_cancelled'),
  };
  const end = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const to = ids.indexOf(String(over.id));
    if (to >= 0) onMove(String(active.id), to);
  };
  return (
    <DndContext sensors={useListSensors()} collisionDetection={closestCenter} onDragEnd={end} accessibility={{ announcements, screenReaderInstructions: { draggable: t('dnd_instructions') } }}>
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        {children}
      </SortableContext>
    </DndContext>
  );
}

/** Where a row dragged in `SortableGroups` will land: before or after the row `id`. */
type Drop = { id: string; after: boolean };
const DropLine = createContext<Drop | null>(null);

/** The rows stay still while one is dragged: a line shows where it will land. */
const still: SortingStrategy = () => null;

/** Rows in groups (a show's days), dragged within their group or to another. The rows stay
 *  still and one line shows where the row will land; the row itself is drawn above the page
 *  (`overlay`), never clipped by a box it sits in (a class's séries), its place left faded.
 *  `onMove`: a row dropped in `group` at `to` (from 0, its place there). Each group's rows in a
 *  `SortableGroup`. */
export function SortableGroups({
  groups,
  onMove,
  overlay,
  children,
}: {
  groups: { id: string; ids: string[] }[];
  onMove: (id: string, group: string, to: number) => void;
  overlay: (id: string) => ReactNode;
  children: ReactNode;
}) {
  const t = useT();
  const [active, setActive] = useState<string | null>(null);
  const [drop, setDropState] = useState<Drop | null>(null);
  // The latest, for the drop itself (the last move may not have been drawn yet).
  const latest = useRef<Drop | null>(null);
  const setDrop = (d: Drop | null) => {
    if (d?.id === latest.current?.id && d?.after === latest.current?.after) return;
    latest.current = d;
    setDropState(d);
  };
  const all = groups.flatMap(g => g.ids);
  const groupOf = (id: string) => groups.find(g => g.ids.includes(id));
  const place = (id: string | number | undefined) => {
    const g = id === undefined ? undefined : groupOf(String(id));
    return g ? String(g.ids.indexOf(String(id)) + 1) : '';
  };
  const announcements: Announcements = {
    onDragStart: ({ active }) => `${t('dnd_picked')} ${place(active.id)}.`,
    onDragOver: ({ over }) => (over ? `${t('dnd_over')} ${place(over.id)}.` : ''),
    onDragEnd: ({ over }) => (over ? `${t('dnd_dropped')} ${place(over.id)}.` : t('dnd_cancelled')),
    onDragCancel: () => t('dnd_cancelled'),
  };
  // Before or after the row under it: by the dragged row's middle (the keyboard puts it on the
  // row exactly: then by the direction it went).
  const track = ({ active, over }: DragMoveEvent) => {
    if (!over || over.id === active.id) return setDrop(null);
    const dragged = active.rect.current.translated;
    const middle = dragged ? dragged.top + dragged.height / 2 : 0;
    const overMiddle = over.rect.top + over.rect.height / 2;
    const after = middle === overMiddle ? all.indexOf(String(over.id)) > all.indexOf(String(active.id)) : middle > overMiddle;
    setDrop({ id: String(over.id), after });
  };
  const reset = () => {
    setActive(null);
    setDrop(null);
  };
  const end = ({ active }: DragEndEvent) => {
    const d = latest.current;
    reset();
    const id = String(active.id);
    const group = d && groupOf(d.id);
    if (!d || !group) return;
    const others = group.ids.filter(x => x !== id);
    const to = others.indexOf(d.id) + (d.after ? 1 : 0);
    if (groupOf(id) === group && to === group.ids.indexOf(id)) return;
    onMove(id, group.id, to);
  };
  return (
    <DndContext
      sensors={useListSensors()}
      collisionDetection={closestCenter}
      onDragStart={e => setActive(String(e.active.id))}
      onDragMove={track}
      onDragOver={track}
      onDragEnd={end}
      onDragCancel={reset}
      accessibility={{ announcements, screenReaderInstructions: { draggable: t('dnd_instructions') } }}
    >
      <DropLine.Provider value={drop}>{children}</DropLine.Provider>
      {createPortal(<DragOverlay dropAnimation={null}>{active ? overlay(active) : null}</DragOverlay>, document.body)}
    </DndContext>
  );
}

/** One group's rows of a `SortableGroups`, in their order. */
export function SortableGroup({ ids, children }: { ids: string[]; children: ReactNode }) {
  return (
    <SortableContext items={ids} strategy={still}>
      {children}
    </SortableContext>
  );
}

/** A row of a `SortableList` or `SortableGroups`: put `ref` and `style` on the row, `grip` on its
 *  handle; `line`, where a dragged row will land (`SortableGroups`), to draw on it. */
export function useSortableRow(id: string, disabled = false) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id, disabled });
  const drop = useContext(DropLine);
  const style: CSSProperties = {
    transform: CSS.Translate.toString(transform),
    transition,
    position: 'relative',
    zIndex: isDragging ? 20 : undefined,
  };
  const line: 'before' | 'after' | null = drop?.id === id ? (drop.after ? 'after' : 'before') : null;
  return { ref: setNodeRef, style, dragging: isDragging, line, grip: { ref: setActivatorNodeRef, attributes, listeners } };
}

/** The line drawn on a row where a dragged row will land (`useSortableRow`'s `line`). */
export function DropMark({ line }: { line: 'before' | 'after' | null }) {
  if (!line) return null;
  // Above the dragged copy too (the overlay's 999): it often floats right over the line.
  return <span aria-hidden className={`pointer-events-none absolute inset-x-2 z-[1000] h-0.5 rounded-full bg-accent ${line === 'before' ? 'top-0' : 'bottom-0'}`} />;
}

/** The handle a row is dragged by. */
export function Grip({ grip, label, disabled = false }: { grip: ReturnType<typeof useSortableRow>['grip']; label: string; disabled?: boolean }) {
  if (disabled) return null;
  return (
    <button
      type="button"
      ref={grip.ref}
      {...grip.attributes}
      {...grip.listeners}
      aria-label={label}
      title={label}
      onClick={e => e.stopPropagation()}
      className="inline-flex min-h-7 min-w-6 cursor-grab touch-none items-center justify-center rounded text-ink-3 hover:bg-hover hover:text-ink active:cursor-grabbing pointer-coarse:min-h-10 pointer-coarse:min-w-9"
    >
      <svg width="10" height="16" viewBox="0 0 10 16" aria-hidden="true" className="fill-current">
        {[2, 8, 14].flatMap(y => [2, 8].map(x => <circle key={`${x}-${y}`} cx={x} cy={y} r="1.6" />))}
      </svg>
    </button>
  );
}

/** A sortable row written inline (in a list's `map`): `children` gets what `useSortableRow` gives. */
export function SortRow({ id, disabled = false, children }: { id: string; disabled?: boolean; children: (row: ReturnType<typeof useSortableRow>) => ReactNode }) {
  const row = useSortableRow(id, disabled);
  return <>{children(row)}</>;
}
