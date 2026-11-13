// Everything the console's panels and map share about one open exercise:
// the live data, what is selected, which tool is active, which layers and
// filters are on, and a handle on the map. Created by Console and handed to
// its children through context.

import { getContext, setContext } from 'svelte';
import { LiveExercise } from './live.svelte';
import { load, save } from './prefs';
import { ui } from './ui.svelte';
import type { Applied } from './types/Applied';
import type { Asset } from './types/Asset';
import type { Command } from './types/Command';
import type { GeoPoint } from './types/GeoPoint';
import type { Shape } from './types/Shape';

export type Mode =
  | { kind: 'select' }
  | { kind: 'add' }
  | { kind: 'measure' }
  | { kind: 'draw'; shape: Shape }
  | { kind: 'link'; from: string }
  | { kind: 'pick'; prompt: string; resolve: (p: GeoPoint | null) => void };

export type Tab = 'assets' | 'injects' | 'log' | 'setup';

export interface Layers {
  clusters: boolean;
  heat: 'off' | 'fill' | 'ring';
  zones: boolean;
  labels: boolean;
  links: boolean;
  annotations: boolean;
  legend: boolean;
}

export interface MapHandle {
  flyTo(p: GeoPoint, zoom?: number): void;
  fit(points: GeoPoint[]): void;
  center(): GeoPoint;
}

const KEY = Symbol('workspace');

export class Workspace {
  readonly live: LiveExercise;
  selected = $state<string | null>(null);
  mode = $state<Mode>({ kind: 'select' });
  tab = $state<Tab>(load<Tab>('tab', 'assets'));
  layers = $state<Layers>({
    clusters: false,
    heat: 'off',
    zones: false,
    labels: true,
    links: true,
    annotations: true,
    legend: true,
    ...load<Partial<Layers>>('layers', {}),
  });
  locked = $state(false);
  drawColor = $state(load('drawColor', '#00ff41'));
  /** Where a new asset will go, once the add form is filled in. */
  placing = $state<GeoPoint | null>(null);
  /** Asset-list filter, also applied to the map. */
  query = $state('');
  /** Where an OpenStreetMap search should look; set from the map's context menu. */
  osmAt = $state<GeoPoint | null>(null);
  hiddenTypes = $state<string[]>([]);
  map: MapHandle | null = null;
  showTaxonomy = $state(false);
  showHelp = $state(false);
  showRoom = $state(false);
  showTimeline = $state(false);
  /** What the Script tab is editing: an inject (by id, or a new one) or a new poll. */
  editing = $state<{ kind: 'inject'; id?: string } | { kind: 'poll' } | null>(null);

  /**
   * `readonly` is the participant's console: they look, select and read, but
   * every editing tool is off. The server enforces this too.
   */
  constructor(
    id: string,
    readonly readonly = false,
  ) {
    this.live = new LiveExercise(id);
    $effect(() => save('layers', this.layers));
    $effect(() => save('tab', this.tab));
    $effect(() => save('drawColor', this.drawColor));
  }

  get exercise() {
    return this.live.exercise;
  }

  /** Assets that pass the search box and type toggles. */
  visible = $derived.by((): Asset[] => {
    const ex = this.live.exercise;
    if (!ex) return [];
    const q = this.query.trim().toLowerCase();
    return Object.values(ex.assets).filter(
      (a) =>
        !this.hiddenTypes.includes(a.type_id) &&
        (!q || a.name.toLowerCase().includes(q) || a.tags.some((t) => t.toLowerCase().includes(q))),
    );
  });

  /** Send a command; a refusal becomes a toast rather than an exception. */
  async send(cmd: Command): Promise<Applied | null> {
    try {
      return await this.live.send(cmd);
    } catch (e) {
      ui.toast((e as Error).message, 'critical', 6000);
      return null;
    }
  }

  async undo() {
    const r = await this.send({ type: 'undo' });
    if (r) ui.toast('Undone', 'ok', 2000);
  }

  select(id: string | null) {
    this.selected = id;
    this.placing = null;
    if (id) this.tab = 'assets';
  }

  setMode(mode: Mode) {
    if (this.readonly && mode.kind !== 'select' && mode.kind !== 'measure') return;
    if (this.mode.kind === 'pick') this.mode.resolve(null);
    this.mode = mode;
    if (mode.kind !== 'add') this.placing = null;
  }

  /** Ask the facilitator to click a spot on the map. */
  pickLocation(prompt: string): Promise<GeoPoint | null> {
    return new Promise((resolve) => this.setMode({ kind: 'pick', prompt, resolve }));
  }

  close() {
    this.live.close();
  }
}

export function provideWorkspace(ws: Workspace) {
  setContext(KEY, ws);
}

export function useWorkspace(): Workspace {
  return getContext<Workspace>(KEY);
}
