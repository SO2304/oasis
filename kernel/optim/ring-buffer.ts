/**
 * OASIS Kernel — Ring Buffer
 *
 * Fixed-capacity circular buffer for O(1) push/evict.
 *
 * PROBLEM: TensionField uses array.splice() which is O(n).
 * With 10,000 tensions at 1kHz, that's 10M shift operations/sec.
 *
 * SOLUTION: Ring buffer. Push = O(1), evict = O(1), iterate = O(n).
 * When the buffer is full, the oldest entry is overwritten.
 * No memory allocation after initial setup.
 */

export class RingBuffer<T> {
  private readonly items: (T | null)[];
  private head = 0; // Next write position
  private count = 0;
  private readonly cap: number;

  constructor(capacity: number) {
    this.cap = capacity;
    this.items = new Array<T | null>(capacity).fill(null);
  }

  /** Push an item. Returns the evicted item if buffer was full, null otherwise. */
  push(item: T): T | null {
    const evicted = this.count >= this.cap ? this.items[this.head] : null;
    this.items[this.head] = item;
    this.head = (this.head + 1) % this.cap;
    if (this.count < this.cap) this.count++;
    return evicted;
  }

  /** Iterate over all live items (oldest to newest). Callback returns false to stop. */
  forEach(callback: (item: T, index: number) => void | false): void {
    if (this.count === 0) return;
    const start = this.count < this.cap ? 0 : this.head;
    for (let i = 0; i < this.count; i++) {
      const idx = (start + i) % this.cap;
      const item = this.items[idx];
      if (item !== null) {
        if (callback(item, i) === false) return;
      }
    }
  }

  /**
   * Remove items matching a predicate. Returns count removed.
   * Sets matching slots to null (lazy deletion).
   */
  removeWhere(predicate: (item: T) => boolean): number {
    let removed = 0;
    for (let i = 0; i < this.cap; i++) {
      const item = this.items[i];
      if (item !== null && predicate(item)) {
        this.items[i] = null;
        this.count--;
        removed++;
      }
    }
    return removed;
  }

  /** Count of live items */
  get size(): number {
    return this.count;
  }

  /** Buffer capacity */
  get capacity(): number {
    return this.cap;
  }

  /** Is the buffer full? */
  get full(): boolean {
    return this.count >= this.cap;
  }

  /** Clear all items */
  clear(): void {
    this.items.fill(null);
    this.head = 0;
    this.count = 0;
  }
}
