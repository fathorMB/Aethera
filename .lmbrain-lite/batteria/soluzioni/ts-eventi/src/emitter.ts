// A small typed event emitter.
//
// Rules every method must keep:
// - listeners run in the order they were registered;
// - `emit` works on a snapshot: listeners added or removed while an emit is running
//   do not change which listeners that emit calls;
// - the same function may be registered more than once, and then runs once per registration.

export type Listener<Args extends unknown[]> = (...args: Args) => void;

type EventMap = Record<string, unknown[]>;

interface Entry {
  fn: Listener<any>;
  once: boolean;
}

export class Emitter<Events extends EventMap> {
  private listeners = new Map<keyof Events, Entry[]>();

  /** Registers `fn`; the returned function removes this registration only. */
  on<K extends keyof Events>(event: K, fn: Listener<Events[K]>): () => void {
    const entry: Entry = { fn, once: false };
    this.entries(event).push(entry);
    return () => this.removeEntry(event, entry);
  }

  /** Like `on`, but the registration is removed right before its first call. */
  once<K extends keyof Events>(event: K, fn: Listener<Events[K]>): () => void {
    const entry: Entry = { fn, once: true };
    this.entries(event).push(entry);
    return () => this.removeEntry(event, entry);
  }

  /** Removes every registration of `fn` for `event`; returns how many were removed. */
  off<K extends keyof Events>(event: K, fn: Listener<Events[K]>): number {
    const list = this.listeners.get(event);
    if (!list) return 0;
    const kept = list.filter((entry) => entry.fn !== fn);
    const removed = list.length - kept.length;
    list.splice(0, list.length, ...kept);
    return removed;
  }

  listenerCount<K extends keyof Events>(event: K): number {
    return this.listeners.get(event)?.length ?? 0;
  }

  /** Calls every listener of `event` with `args`; returns how many were called. */
  emit<K extends keyof Events>(event: K, ...args: Events[K]): number {
    const snapshot = [...(this.listeners.get(event) ?? [])];
    let called = 0;
    for (const entry of snapshot) {
      if (entry.once) {
        this.removeEntry(event, entry);
      }
      entry.fn(...args);
      called++;
    }
    return called;
  }

  private entries<K extends keyof Events>(event: K): Entry[] {
    let list = this.listeners.get(event);
    if (!list) {
      list = [];
      this.listeners.set(event, list);
    }
    return list;
  }

  private removeEntry<K extends keyof Events>(event: K, entry: Entry): void {
    const list = this.listeners.get(event);
    if (!list) return;
    const i = list.indexOf(entry);
    if (i >= 0) list.splice(i, 1);
  }
}
