<script lang="ts">
  // The geographic canvas. Everything on it is a GeoJSON source drawn by the
  // GPU: assets as canvas-rendered marker images (markers.ts), links, sector
  // zones, heatmaps, annotations and the measuring line. Each change to the
  // exercise becomes one setData call per source.
  import {
    LngLatBounds,
    Map as MapLibre,
    NavigationControl,
    ScaleControl,
    setWorkerUrl,
    type GeoJSONSource,
    type MapGeoJSONFeature,
    type MapLayerMouseEvent,
    type MapMouseEvent,
  } from 'maplibre-gl';
  // MapLibre 6 parses tiles in a separate worker file; serve Vite's copy.
  import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?url';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { centroid, circle, formatMi, hull, pathMi, type Position } from '../../lib/geo';
  import { drawMarker, markerId, parseMarkerId } from '../../lib/markers';
  import { scoreColor, sectorHealth, statusValue } from '../../lib/sitrep';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { GeoPoint } from '../../lib/types/GeoPoint';
  import Drawing from './Drawing.svelte';
  import ContextMenu from './ContextMenu.svelte';
  import Legend from './Legend.svelte';
  import AnnotationPanel from './AnnotationPanel.svelte';

  setWorkerUrl(workerUrl);

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  // Keyless, free vector tiles. Offline region packs arrive in milestone 6.
  const DARK = 'https://tiles.openfreemap.org/styles/dark';
  const LIGHT = 'https://tiles.openfreemap.org/styles/positron';
  // Chosen when the map is created; a theme change remounts the map.
  const light = untrack(() => ws.exercise?.branding.theme === 'daylight');
  const STYLE = light ? LIGHT : DARK;
  const TEXT = light ? '#14202b' : '#d6f5dc';
  const HALO = light ? '#ffffff' : '#050a06';
  const FONT = ['Noto Sans Regular'];

  let container: HTMLDivElement;
  let map = $state<MapLibre>();
  let ready = $state(false);
  let framed = false;
  let measure = $state<Position[]>([]);
  let menu = $state<{ x: number; y: number; at: GeoPoint } | null>(null);
  let tip = $state<{ x: number; y: number; title: string; lines: string[] } | null>(null);
  let annotation = $state<string | null>(null);

  type Props = Record<string, unknown>;
  type Feature = GeoJSON.Feature<GeoJSON.Geometry, Props>;
  const fc = (features: Feature[]): GeoJSON.FeatureCollection<GeoJSON.Geometry, Props> => ({ type: 'FeatureCollection', features });
  const pos = (g: GeoPoint): Position => [g.lon, g.lat];

  // ───────────────────────── data ─────────────────────────

  const assetData = $derived.by(() => {
    const sets = ex.taxonomy.status_sets;
    const fill = sets.find((s) => s.display === 'fill');
    const ring = sets.find((s) => s.display === 'ring');
    const features: Feature[] = [];
    for (const a of ws.visible) {
      if (!a.geo) continue;
      const type = ex.taxonomy.asset_types.find((t) => t.id === a.type_id);
      const f = fill ? statusValue(fill, a) : undefined;
      const r = ring ? statusValue(ring, a) : undefined;
      const selected = a.id === ws.selected;
      features.push({
        type: 'Feature',
        geometry: { type: 'Point', coordinates: pos(a.geo) },
        properties: {
          id: a.id,
          name: a.name,
          marker: markerId({
            icon: type?.icon ?? 'circle',
            fill: a.color ?? f?.color ?? type?.color ?? '#888888',
            ring: r?.color ?? '#ffffff',
            ringStyle: r?.glyph ?? 'solid',
            glyph: f?.glyph ?? '',
            selected,
          }),
          sort: selected ? 1 : 0,
          // Heat: worse status, more heat.
          fillHeat: f ? 1 - f.score / 100 : 0,
          ringHeat: r ? 1 - r.score / 100 : 0,
        },
      });
    }
    return fc(features);
  });

  const linkData = $derived.by(() => {
    const features: Feature[] = [];
    for (const l of Object.values(ex.links)) {
      const a = ex.assets[l.from]?.geo;
      const b = ex.assets[l.to]?.geo;
      if (!a || !b) continue;
      features.push({ type: 'Feature', geometry: { type: 'LineString', coordinates: [pos(a), pos(b)] }, properties: { kind: l.kind } });
    }
    return fc(features);
  });

  const zones = $derived.by(() => {
    const health = sectorHealth(ex);
    return ex.taxonomy.sectors.flatMap((s) => {
      const members = Object.values(ex.assets).filter((a) => a.geo && s.asset_types.includes(a.type_id));
      if (!members.length) return [];
      const pts = members.map((a) => pos(a.geo!));
      const ring = pts.length >= 3 && hull(pts).length ? hull(pts) : circle(centroid(pts), 0.3);
      const score = health.find((h) => h.id === s.id)?.score ?? null;
      return [{ sector: s, ring, center: centroid(pts), score, members }];
    });
  });

  const zoneData = $derived(
    fc(
      zones.flatMap((z) => [
        {
          type: 'Feature',
          geometry: { type: 'Polygon', coordinates: [z.ring] },
          properties: { sector: z.sector.id, color: scoreColor(z.score), edge: z.sector.color },
        },
        {
          type: 'Feature',
          geometry: { type: 'Point', coordinates: z.center },
          properties: { sector: z.sector.id, label: `${z.sector.name} ${z.score === null ? '' : `${Math.round(z.score)}%`}` },
        },
      ]),
    ),
  );

  const annotationData = $derived(
    fc(
      Object.values(ex.annotations).map((a) => ({
        type: 'Feature',
        geometry: a.geometry,
        properties: { id: a.id, color: a.color, label: a.label, selected: a.id === annotation },
      })),
    ),
  );

  const measureData = $derived.by(() => {
    const features: Feature[] = measure.map((p) => ({ type: 'Feature', geometry: { type: 'Point', coordinates: p }, properties: {} }));
    if (measure.length >= 2) {
      features.push({
        type: 'Feature',
        geometry: { type: 'LineString', coordinates: measure },
        properties: { label: formatMi(pathMi(measure)) },
      });
    }
    return fc(features);
  });

  // ───────────────────────── map setup ─────────────────────────

  function source(id: string): GeoJSONSource | undefined {
    return map?.getSource(id) as GeoJSONSource | undefined;
  }

  onMount(() => {
    const m = new MapLibre({ container, style: STYLE, center: [-98.5, 39.8], zoom: 3.5, attributionControl: { compact: true } });
    map = m;
    m.addControl(new NavigationControl({ visualizePitch: false }), 'top-right');
    m.addControl(new ScaleControl({ unit: 'imperial' }), 'bottom-right');
    m.doubleClickZoom.disable();

    ws.map = {
      flyTo: (p, zoom) => m.flyTo({ center: [p.lon, p.lat], zoom: Math.max(zoom ?? 14, m.getZoom()) }),
      fit: (points) => {
        if (!points.length) return;
        const b = new LngLatBounds();
        for (const p of points) b.extend([p.lon, p.lat]);
        m.fitBounds(b, { padding: 80, maxZoom: 15 });
      },
      center: () => {
        const c = m.getCenter();
        return { lat: c.lat, lon: c.lng };
      },
    };

    // Markers are drawn on demand: the image id carries everything needed.
    m.setMissingStyleImageResolver((id) => {
      const spec = parseMarkerId(id);
      if (m.hasImage(id)) return;
      if (spec) m.addImage(id, drawMarker(spec, 2), { pixelRatio: 2 });
      // The base style names a few icons its sprite does not have. Give it a
      // blank so it stops warning about them on every tile.
      else m.addImage(id, { width: 1, height: 1, data: new Uint8Array(4) });
    });

    m.on('load', () => {
      m.addSource('zones', { type: 'geojson', data: zoneData });
      m.addSource('links', { type: 'geojson', data: linkData });
      m.addSource('annotations', { type: 'geojson', data: annotationData });
      m.addSource('assets', { type: 'geojson', data: assetData, cluster: false, clusterRadius: 45, clusterMaxZoom: 13 });
      // Heat reads its own copy: clustering would hide most points from it.
      m.addSource('heat', { type: 'geojson', data: assetData });
      m.addSource('measure', { type: 'geojson', data: measureData });

      const polys: ['==', ['geometry-type'], string] = ['==', ['geometry-type'], 'Polygon'];
      m.addLayer({ id: 'zones-fill', type: 'fill', source: 'zones', filter: polys, paint: { 'fill-color': ['get', 'color'], 'fill-opacity': 0.12 } });
      m.addLayer({ id: 'zones-line', type: 'line', source: 'zones', filter: polys, paint: { 'line-color': ['get', 'edge'], 'line-width': 1.5, 'line-dasharray': [3, 2] } });
      m.addLayer({
        id: 'heat',
        type: 'heatmap',
        source: 'heat',
        maxzoom: 17,
        paint: {
          'heatmap-weight': ['get', 'fillHeat'],
          'heatmap-radius': ['interpolate', ['linear'], ['zoom'], 4, 12, 14, 40],
          'heatmap-intensity': ['interpolate', ['linear'], ['zoom'], 4, 0.6, 14, 1.4],
          'heatmap-opacity': 0.75,
          'heatmap-color': ['interpolate', ['linear'], ['heatmap-density'], 0, 'rgba(0,0,0,0)', 0.2, '#ffd70055', 0.5, '#ff8c00aa', 0.8, '#ff3333cc', 1, '#ff0000'],
        },
      });
      m.addLayer({ id: 'ann-fill', type: 'fill', source: 'annotations', filter: polys, paint: { 'fill-color': ['get', 'color'], 'fill-opacity': 0.15 } });
      m.addLayer({
        id: 'ann-line',
        type: 'line',
        source: 'annotations',
        filter: ['!=', ['geometry-type'], 'Point'],
        paint: { 'line-color': ['get', 'color'], 'line-width': ['case', ['get', 'selected'], 4, 2.5] },
      });
      m.addLayer({ id: 'links', type: 'line', source: 'links', filter: ['!=', ['get', 'kind'], 'depends_on'], paint: { 'line-color': '#00ff41', 'line-opacity': 0.55, 'line-width': 1.5 } });
      m.addLayer({ id: 'links-depends', type: 'line', source: 'links', filter: ['==', ['get', 'kind'], 'depends_on'], paint: { 'line-color': '#ffaa00', 'line-width': 2, 'line-dasharray': [2, 1.5] } });
      m.addLayer({
        id: 'ann-point',
        type: 'circle',
        source: 'annotations',
        filter: ['==', ['geometry-type'], 'Point'],
        paint: { 'circle-radius': 6, 'circle-color': ['get', 'color'], 'circle-stroke-color': '#000', 'circle-stroke-width': 1.5 },
      });
      m.addLayer({
        id: 'clusters',
        type: 'circle',
        source: 'assets',
        filter: ['has', 'point_count'],
        paint: {
          'circle-color': light ? '#ffffff' : '#0b130d',
          'circle-stroke-color': '#00ff41',
          'circle-stroke-width': 2,
          'circle-radius': ['step', ['get', 'point_count'], 16, 10, 20, 50, 26],
        },
      });
      m.addLayer({
        id: 'cluster-count',
        type: 'symbol',
        source: 'assets',
        filter: ['has', 'point_count'],
        layout: { 'text-field': ['get', 'point_count_abbreviated'], 'text-font': FONT, 'text-size': 12 },
        paint: { 'text-color': TEXT },
      });
      m.addLayer({
        id: 'assets',
        type: 'symbol',
        source: 'assets',
        filter: ['!', ['has', 'point_count']],
        layout: {
          'icon-image': ['get', 'marker'],
          'icon-size': ['interpolate', ['linear'], ['zoom'], 3, 0.6, 12, 0.9, 16, 1.1],
          'icon-allow-overlap': true,
          'icon-ignore-placement': true,
          'symbol-sort-key': ['get', 'sort'],
        },
      });
      m.addLayer({
        id: 'asset-labels',
        type: 'symbol',
        source: 'assets',
        minzoom: 13,
        filter: ['!', ['has', 'point_count']],
        layout: { 'text-field': ['get', 'name'], 'text-font': FONT, 'text-size': 11, 'text-offset': [0, 1.6], 'text-anchor': 'top', 'text-optional': true },
        paint: { 'text-color': TEXT, 'text-halo-color': HALO, 'text-halo-width': 1.5 },
      });
      m.addLayer({
        id: 'zone-labels',
        type: 'symbol',
        source: 'zones',
        filter: ['==', ['geometry-type'], 'Point'],
        maxzoom: 15,
        layout: { 'text-field': ['get', 'label'], 'text-font': FONT, 'text-size': 13, 'text-allow-overlap': false },
        paint: { 'text-color': '#ffffff', 'text-halo-color': '#000', 'text-halo-width': 2 },
      });
      m.addLayer({
        id: 'ann-labels',
        type: 'symbol',
        source: 'annotations',
        layout: { 'text-field': ['get', 'label'], 'text-font': FONT, 'text-size': 12, 'text-offset': [0, 1.2], 'text-anchor': 'top' },
        paint: { 'text-color': ['get', 'color'], 'text-halo-color': '#000', 'text-halo-width': 1.5 },
      });
      m.addLayer({ id: 'measure-line', type: 'line', source: 'measure', filter: ['==', ['geometry-type'], 'LineString'], paint: { 'line-color': '#ffffff', 'line-width': 2, 'line-dasharray': [2, 2] } });
      m.addLayer({ id: 'measure-points', type: 'circle', source: 'measure', filter: ['==', ['geometry-type'], 'Point'], paint: { 'circle-radius': 4, 'circle-color': '#ffffff' } });
      m.addLayer({
        id: 'measure-label',
        type: 'symbol',
        source: 'measure',
        filter: ['==', ['geometry-type'], 'LineString'],
        layout: { 'text-field': ['get', 'label'], 'text-font': FONT, 'text-size': 14, 'symbol-placement': 'line-center', 'text-offset': [0, -1] },
        paint: { 'text-color': '#ffffff', 'text-halo-color': '#000', 'text-halo-width': 2 },
      });

      wireEvents(m);
      ready = true;
    });
  });

  onDestroy(() => {
    ws.map = null;
    map?.remove();
  });

  // ───────────────────────── interaction ─────────────────────────

  const geoOf = (e: MapMouseEvent): GeoPoint => ({ lat: e.lngLat.lat, lon: e.lngLat.lng });
  const hit = (e: MapMouseEvent, layers: string[]): MapGeoJSONFeature | undefined =>
    map?.queryRenderedFeatures(e.point, { layers: layers.filter((l) => map?.getLayer(l)) })[0];

  function wireEvents(m: MapLibre) {
    m.on('click', (e: MapMouseEvent) => {
      menu = null;
      const mode = ws.mode;
      if (mode.kind === 'draw') return; // terra-draw owns clicks while drawing
      if (mode.kind === 'measure') {
        measure = [...measure, [e.lngLat.lng, e.lngLat.lat]];
        return;
      }
      if (mode.kind === 'pick') {
        ws.mode = { kind: 'select' };
        mode.resolve(geoOf(e));
        return;
      }
      if (mode.kind === 'add') {
        ws.select(null);
        ws.placing = geoOf(e);
        ws.tab = 'assets';
        return;
      }
      const cluster = hit(e, ['clusters']);
      if (cluster) {
        const id = cluster.properties.cluster_id as number;
        void source('assets')
          ?.getClusterExpansionZoom(id)
          .then((zoom) => m.easeTo({ center: (cluster.geometry as GeoJSON.Point).coordinates as [number, number], zoom }));
        return;
      }
      const asset = hit(e, ['assets']);
      if (asset) {
        const id = asset.properties.id as string;
        if (mode.kind === 'link' && id !== mode.from) {
          void ws.send({ type: 'add_link', from: mode.from, to: id, kind: 'connects_to' });
          ws.mode = { kind: 'select' };
        } else {
          ws.select(id);
        }
        annotation = null;
        return;
      }
      const ann = hit(e, ['ann-point', 'ann-line', 'ann-fill']);
      if (ann) {
        annotation = ann.properties.id as string;
        return;
      }
      annotation = null;
      ws.select(null);
    });

    m.on('dblclick', () => {
      if (ws.mode.kind === 'measure') ws.setMode({ kind: 'select' });
    });

    m.on('contextmenu', (e: MapMouseEvent) => {
      e.preventDefault();
      if (ws.mode.kind !== 'select' || ws.readonly) return;
      menu = { x: e.point.x, y: e.point.y, at: geoOf(e) };
    });

    m.on('mousemove', 'assets', (e: MapLayerMouseEvent) => {
      const f = e.features?.[0];
      const a = f && ex.assets[f.properties.id as string];
      if (!a) return;
      const type = ex.taxonomy.asset_types.find((t) => t.id === a.type_id);
      tip = {
        x: e.point.x,
        y: e.point.y,
        title: a.name,
        lines: [type?.name ?? '', ...ex.taxonomy.status_sets.map((s) => `${s.name}: ${statusValue(s, a)?.label ?? ''}`)],
      };
    });
    m.on('mousemove', 'zones-fill', (e: MapLayerMouseEvent) => {
      if (map?.queryRenderedFeatures(e.point, { layers: ['assets'] }).length) return;
      const z = zones.find((x) => x.sector.id === e.features?.[0]?.properties.sector);
      if (!z) return;
      const fill = ex.taxonomy.status_sets.find((s) => s.id === z.sector.status_set);
      tip = {
        x: e.point.x,
        y: e.point.y,
        title: `${z.sector.name} · ${z.score === null ? '—' : `${Math.round(z.score)}%`}`,
        lines: z.members.slice(0, 12).map((a) => `${a.name}: ${fill ? (statusValue(fill, a)?.label ?? '') : ''}`).concat(z.members.length > 12 ? [`…and ${z.members.length - 12} more`] : []),
      };
    });
    for (const layer of ['assets', 'zones-fill']) m.on('mouseleave', layer, () => (tip = null));
    for (const layer of ['assets', 'clusters', 'ann-point', 'ann-line', 'ann-fill']) {
      m.on('mouseenter', layer, () => ws.mode.kind === 'select' && (m.getCanvas().style.cursor = 'pointer'));
      m.on('mouseleave', layer, () => ws.mode.kind === 'select' && (m.getCanvas().style.cursor = ''));
    }
  }

  // ───────────────────────── reactive updates ─────────────────────────

  $effect(() => {
    const d = assetData;
    if (ready) source('assets')?.setData(d);
    // Only keep the heat copy current while it is showing.
    if (ready && ws.layers.heat !== 'off') source('heat')?.setData(d);
  });
  $effect(() => {
    const d = linkData;
    if (ready) source('links')?.setData(d);
  });
  $effect(() => {
    const d = zoneData;
    if (ready) source('zones')?.setData(d);
  });
  $effect(() => {
    const d = annotationData;
    if (ready) source('annotations')?.setData(d);
  });
  $effect(() => {
    const d = measureData;
    if (ready) source('measure')?.setData(d);
  });

  // Layer toggles
  $effect(() => {
    if (!ready || !map) return;
    const L = ws.layers;
    const show = (ids: string[], on: boolean) => ids.forEach((id) => map?.setLayoutProperty(id, 'visibility', on ? 'visible' : 'none'));
    show(['zones-fill', 'zones-line', 'zone-labels'], L.zones);
    show(['links', 'links-depends'], L.links);
    show(['ann-fill', 'ann-line', 'ann-point', 'ann-labels'], L.annotations);
    show(['asset-labels'], L.labels);
    show(['heat'], L.heat !== 'off');
    map.setPaintProperty('heat', 'heatmap-weight', ['get', L.heat === 'ring' ? 'ringHeat' : 'fillHeat']);
    void source('assets')?.setClusterOptions({ cluster: L.clusters, clusterRadius: 45, clusterMaxZoom: 13 });
  });

  // Lock: no panning or zooming, so drawing cannot drag the map by accident.
  $effect(() => {
    if (!map) return;
    const handlers = [map.dragPan, map.scrollZoom, map.boxZoom, map.dragRotate, map.keyboard, map.touchZoomRotate];
    for (const h of handlers) {
      if (ws.locked) h.disable();
      else h.enable();
    }
  });

  // Cursor and measuring follow the mode.
  $effect(() => {
    const kind = ws.mode.kind;
    if (kind !== 'measure') measure = [];
    if (map) map.getCanvas().style.cursor = kind === 'select' || kind === 'draw' ? '' : 'crosshair';
  });

  // Frame the assets once, the first time there are any.
  $effect(() => {
    if (!ready || !map || framed) return;
    const pts = Object.values(ex.assets).flatMap((a) => (a.geo ? [a.geo] : []));
    if (!pts.length) return;
    framed = true;
    const b = new LngLatBounds();
    for (const p of pts) b.extend([p.lon, p.lat]);
    map.fitBounds(b, { padding: 60, maxZoom: 14, duration: 0 });
  });

  const measured = $derived(measure.length >= 2 ? formatMi(pathMi(measure)) : null);
</script>

<div class="map" bind:this={container} role="application" aria-label="Exercise map"></div>

{#if ready && map && !ws.readonly}
  <Drawing {map} />
{/if}
{#if ws.layers.legend}<Legend />{/if}
{#if annotation}
  {@const a = ex.annotations[annotation]}
  {#if a && !ws.readonly}<AnnotationPanel annotation={a} onclose={() => (annotation = null)} />{/if}
{/if}
{#if menu}
  <ContextMenu
    {...menu}
    onclose={() => (menu = null)}
    onmeasure={(p) => {
      ws.setMode({ kind: 'measure' });
      measure = [[p.lon, p.lat]];
    }}
  />
{/if}
{#if tip}
  <div class="tip" style:left="{tip.x + 14}px" style:top="{tip.y + 14}px" role="tooltip">
    <strong>{tip.title}</strong>
    {#each tip.lines as l, i (i)}<span>{l}</span>{/each}
  </div>
{/if}
{#if ws.mode.kind !== 'select'}
  <div class="hint" role="status">
    {#if ws.mode.kind === 'add'}Click the map to place a new asset.
    {:else if ws.mode.kind === 'measure'}{measured ?? 'Click to start measuring.'} · double-click or Esc to finish
    {:else if ws.mode.kind === 'draw'}Drawing a {ws.mode.shape}. {ws.mode.shape === 'polygon' || ws.mode.shape === 'line' ? 'Click to add points; click the last point again to finish.' : 'Click (and drag) on the map.'}
    {:else if ws.mode.kind === 'link'}Click the asset to link to.
    {:else if ws.mode.kind === 'pick'}{ws.mode.prompt}{/if}
    <button class="small" onclick={() => ws.setMode({ kind: 'select' })}>Cancel (Esc)</button>
  </div>
{/if}

<style>
  .map { position: absolute; inset: 0; }
  .tip {
    position: absolute; z-index: 15; pointer-events: none; display: grid; gap: 1px; max-width: 280px;
    background: color-mix(in srgb, var(--panel) 94%, transparent); border: 1px solid var(--line); border-radius: var(--radius); padding: 6px 9px; font-size: 12px;
  }
  .hint {
    position: absolute; bottom: 36px; left: 50%; transform: translateX(-50%); z-index: 12; display: flex; gap: 10px; align-items: center;
    background: color-mix(in srgb, var(--panel) 94%, transparent); border: 1px solid var(--accent); border-radius: var(--radius); padding: 6px 10px; font-size: 13px;
  }
  .small { font-size: 12px; padding: 2px 8px; }
</style>
