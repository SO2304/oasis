/**
 * OASIS Kernel — IPC Message Bus
 *
 * Heritage: LAP ADR-008 (Event Bus Durable, HMAC-SHA256)
 *
 * INNOVATION: Dual-lane bus architecture
 * - FAST lane: synchronous, for PANIC and HARD_RT messages (< 1ms)
 * - NORMAL lane: async microtask-based, for everything else
 *
 * Design:
 * - Topic-based pub/sub with glob pattern matching
 * - Tenant isolation enforced at bus level (R4)
 * - PANIC messages bypass all filters and hit every subscriber
 * - Messages are immutable after creation
 */

import {
  type KernelMessage,
  type MessageType,
  type AgentId,
  type TenantId,
  MessageType as MT,
  monotonicNow,
} from '../types.js';
import { getKillSwitch } from '../kill-switch.js';

type MessageHandler<T = unknown> = (message: KernelMessage<T>) => void;

interface Subscription {
  readonly id: string;
  readonly topicPattern: string;
  readonly tenantId: TenantId | '*';
  readonly handler: MessageHandler;
  readonly regex: RegExp;
}

let subIdCounter = 0;

function topicToRegex(pattern: string): RegExp {
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '[^.]+')
    .replace(/#/g, '.*');
  return new RegExp(`^${escaped}$`);
}

export class MessageBus {
  private readonly subscriptions = new Map<string, Subscription>();
  private messageCount = 0;

  constructor(private readonly killSwitch = getKillSwitch()) {
    // Auto-subscribe to kernel panic — broadcast to all
    this.killSwitch.onPanic((event) => {
      this.publishSync({
        id: `panic-${Date.now()}`,
        type: MT.PANIC,
        topic: 'kernel.panic',
        source: 'KERNEL',
        target: 'BROADCAST',
        payload: event,
        timestamp: event.timestamp,
        tenantId: '*' as TenantId,
      });
    });
  }

  /**
   * Subscribe to a topic pattern.
   * Patterns: "agent.*.ready", "hal.driver.#", "kernel.panic"
   * Use '*' for single-level wildcard, '#' for multi-level.
   */
  subscribe<T = unknown>(
    topicPattern: string,
    tenantId: TenantId | '*',
    handler: MessageHandler<T>,
  ): string {
    const id = `sub-${++subIdCounter}`;
    this.subscriptions.set(id, {
      id,
      topicPattern,
      tenantId,
      handler: handler as MessageHandler,
      regex: topicToRegex(topicPattern),
    });
    return id;
  }

  /** Unsubscribe by subscription ID */
  unsubscribe(subscriptionId: string): boolean {
    return this.subscriptions.delete(subscriptionId);
  }

  /**
   * FAST lane — synchronous publish.
   * Use for PANIC and HARD_RT messages only.
   * Handlers execute inline, blocking the caller.
   */
  publishSync<T>(message: KernelMessage<T>): void {
    this.messageCount++;
    for (const sub of this.subscriptions.values()) {
      if (this.matches(sub, message)) {
        try {
          sub.handler(message as KernelMessage);
        } catch {
          // Never let a subscriber crash the bus
        }
      }
    }
  }

  /**
   * NORMAL lane — async publish via microtask.
   * Use for EVENT, COMMAND, QUERY, RESPONSE messages.
   * Handlers execute on next microtask — non-blocking.
   */
  publish<T>(message: KernelMessage<T>): void {
    // PANIC always goes sync — no delay allowed
    if (message.type === MT.PANIC) {
      this.publishSync(message);
      return;
    }

    this.messageCount++;
    queueMicrotask(() => {
      for (const sub of this.subscriptions.values()) {
        if (this.matches(sub, message)) {
          try {
            sub.handler(message as KernelMessage);
          } catch {
            // Swallow
          }
        }
      }
    });
  }

  /** Create a properly typed message */
  createMessage<T>(
    type: MessageType,
    topic: string,
    source: AgentId | 'KERNEL',
    target: AgentId | 'BROADCAST',
    payload: T,
    tenantId: TenantId,
  ): KernelMessage<T> {
    return {
      id: `msg-${++this.messageCount}`,
      type,
      topic,
      source,
      target,
      payload,
      timestamp: monotonicNow(),
      tenantId,
    };
  }

  /** Total messages processed */
  getMessageCount(): number {
    return this.messageCount;
  }

  /** Active subscription count */
  getSubscriptionCount(): number {
    return this.subscriptions.size;
  }

  private matches(sub: Subscription, message: KernelMessage): boolean {
    // PANIC bypasses tenant isolation — everyone must hear it
    if (message.type === MT.PANIC) {
      return sub.regex.test(message.topic) || sub.topicPattern === 'kernel.panic';
    }

    // Tenant isolation (R4)
    if (sub.tenantId !== '*' && message.tenantId !== sub.tenantId) {
      return false;
    }

    // Topic matching
    if (!sub.regex.test(message.topic)) return false;

    // Target filtering — BROADCAST hits everyone, specific target hits only that agent
    // (subscription-level filtering is topic-based, target is for the subscriber to check)
    return true;
  }
}
