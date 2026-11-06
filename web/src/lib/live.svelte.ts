// A live, reactive copy of one exercise, kept in sync over the websocket.
//
// On reconnect it asks for the events after the last `seq` it applied, so a
// dropped connection costs a few events, not a full reload. Out-of-order or
// repeated events (which the server should never send) force a resync rather
// than corrupting local state.

import { api, socketUrl } from './api';
import { apply } from './reducer';
import type { Command } from './types/Command';
import type { Exercise } from './types/Exercise';
import type { ServerMessage } from './types/ServerMessage';

export type Connection = 'connecting' | 'live' | 'offline';

export class LiveExercise {
  exercise = $state<Exercise | null>(null);
  seq = $state(0);
  connection = $state<Connection>('connecting');

  #ws: WebSocket | null = null;
  #retry = 1000;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #closed = false;

  constructor(readonly id: string) {
    this.#connect();
  }

  send(command: Command) {
    return api.send(this.id, command);
  }

  close() {
    this.#closed = true;
    clearTimeout(this.#timer);
    this.#ws?.close();
  }

  #connect() {
    this.connection = 'connecting';
    const ws = new WebSocket(socketUrl(this.id, this.exercise ? this.seq : undefined));
    this.#ws = ws;
    ws.onopen = () => {
      this.connection = 'live';
      this.#retry = 1000;
    };
    ws.onmessage = (e) => this.#receive(JSON.parse(e.data as string) as ServerMessage);
    ws.onclose = () => {
      if (this.#closed) return;
      this.connection = 'offline';
      this.#timer = setTimeout(() => this.#connect(), this.#retry);
      this.#retry = Math.min(this.#retry * 2, 15000);
    };
    ws.onerror = () => ws.close();
  }

  #receive(msg: ServerMessage) {
    if (msg.kind === 'snapshot') {
      this.exercise = msg.exercise;
      this.seq = msg.seq;
      return;
    }
    if (!this.exercise || msg.seq !== this.seq + 1) {
      // A gap or a repeat: start over from a snapshot.
      this.exercise = null;
      this.#ws?.close();
      return;
    }
    apply(this.exercise, msg);
    this.seq = msg.seq;
  }
}
