// Who this browser is to the server: the facilitator (on the host machine),
// a player or observer who joined with a code, or nobody yet.

import { api, ApiError } from './api';
import type { Me } from './types/Me';

class Session {
  /** `undefined` while loading, `null` when not joined. */
  me = $state<Me | null | undefined>(undefined);
  error = $state('');

  async load() {
    try {
      this.me = await api.me();
      this.error = '';
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) this.me = null;
      else this.error = (e as Error).message;
    }
  }

  async leave() {
    await api.leave().catch(() => {});
    await this.load();
  }
}

export const session = new Session();
