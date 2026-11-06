// App-wide transient UI: toasts, and an in-app confirm/prompt that replaces
// window.confirm/prompt (which some webviews block or style badly).

export type Tone = 'info' | 'warning' | 'critical' | 'ok';

interface Toast {
  id: number;
  text: string;
  tone: Tone;
}

interface DialogRequest {
  title: string;
  body?: string;
  confirm: string;
  danger: boolean;
  input?: { value: string; label: string };
  resolve: (v: string | boolean | null) => void;
}

let next = 0;

class Ui {
  toasts = $state<Toast[]>([]);
  dialog = $state<DialogRequest | null>(null);

  toast(text: string, tone: Tone = 'info', ms = 4000) {
    const id = ++next;
    this.toasts.push({ id, text, tone });
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), ms);
  }

  confirm(title: string, opts: { body?: string; confirm?: string; danger?: boolean } = {}): Promise<boolean> {
    return new Promise((resolve) => {
      this.dialog = {
        title,
        body: opts.body,
        confirm: opts.confirm ?? 'OK',
        danger: opts.danger ?? false,
        resolve: (v) => resolve(v === true),
      };
    });
  }

  prompt(title: string, label: string, value = '', confirm = 'OK'): Promise<string | null> {
    return new Promise((resolve) => {
      this.dialog = {
        title,
        confirm,
        danger: false,
        input: { value, label },
        resolve: (v) => resolve(typeof v === 'string' ? v : null),
      };
    });
  }

  settle(v: string | boolean | null) {
    const d = this.dialog;
    this.dialog = null;
    d?.resolve(v);
  }
}

export const ui = new Ui();
