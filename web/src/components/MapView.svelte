<script lang="ts">
  // The geographic canvas. Assets and links are GeoJSON sources drawn by the
  // GPU, so thousands of markers stay smooth; every change to the exercise
  // becomes one setData call per source.
  import {
    LngLatBounds,
    Map as MapLibre,
    NavigationControl,
    ScaleControl,
    type GeoJSONSource,
    type MapLayerMouseEvent,
    type MapMouseEvent,
  } from 'maplibre-gl';
  // MapLibre 6 runs tile parsing in a separate worker file; point it at the
  // copy Vite emits next to the bundle so it is served by our own binary.
  import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?url';
  import { setWorkerUrl } from 'maplibre-gl';

  setWorkerUrl(workerUrl);
  import { onDestroy, onMount } from 'svelte';
  import { statusValue } from '../lib/sitrep';
  import type { Exercise } from '../lib/types/Exercise';
  import type { GeoPoint } from '../lib/types/GeoPoint';

  let {
    exercise,
    selected,
    adding,
    onselect,
    onplace,
  }: {
    exercise: Exercise;
    selected: string | null;
    adding: boolean;
    onselect: (id: string | null) => void;
    onplace: (p: GeoPoint) => void;
  } = $props();

  // Keyless, free vector tiles. Offline region packs arrive in milestone 6.
  const STYLE = 'https://tiles.openfreemap.org/styles/dark';

  let container: HTMLDivElement;
  let map: MapLibre | undefined;
  let ready = $state(false);
  let framed = false;

  type Feature = GeoJSON.Feature<GeoJSON.Geometry, Record<string, unknown>>;
  const collection = (features: Feature[]): GeoJSON.FeatureCollection<GeoJSON.Geometry, Record<string, unknown>> => ({
    type: 'FeatureCollection',
    features,
  });

  const assetData = $derived.by(() => {
    const sets = exercise.taxonomy.status_sets;
    const fill = sets.find((s) => s.display === 'fill');
    const ring = sets.find((s) => s.display === 'ring');
    const features: Feature[] = [];
    for (const a of Object.values(exercise.assets)) {
      if (!a.geo) continue;
      const type = exercise.taxonomy.asset_types.find((t) => t.id === a.type_id);
      features.push({
        type: 'Feature',
        id: features.length,
        geometry: { type: 'Point', coordinates: [a.geo.lon, a.geo.lat] },
        properties: {
          id: a.id,
          name: a.name,
          fill: (fill && statusValue(fill, a)?.color) ?? a.color ?? type?.color ?? '#888888',
          ring: (ring && statusValue(ring, a)?.color) ?? '#ffffff',
          selected: a.id === selected,
        },
      });
    }
    return collection(features);
  });

  const linkData = $derived.by(() => {
    const features: Feature[] = [];
    for (const l of Object.values(exercise.links)) {
      const a = exercise.assets[l.from]?.geo;
      const b = exercise.assets[l.to]?.geo;
      if (!a || !b) continue;
      features.push({
        type: 'Feature',
        geometry: { type: 'LineString', coordinates: [[a.lon, a.lat], [b.lon, b.lat]] },
        properties: { kind: l.kind, label: l.label },
      });
    }
    return collection(features);
  });

  onMount(() => {
    map = new MapLibre({
      container,
      style: STYLE,
      center: [-98.5, 39.8],
      zoom: 3.5,
      attributionControl: { compact: true },
    });
    map.addControl(new NavigationControl({ visualizePitch: false }), 'top-right');
    map.addControl(new ScaleControl({ unit: 'imperial' }), 'bottom-left');

    map.on('load', () => {
      if (!map) return;
      map.addSource('links', { type: 'geojson', data: linkData });
      map.addSource('assets', { type: 'geojson', data: assetData });
      map.addLayer({
        id: 'links',
        type: 'line',
        source: 'links',
        paint: { 'line-color': '#00ff41', 'line-opacity': 0.55, 'line-width': 1.5 },
      });
      map.addLayer({
        id: 'links-depends',
        type: 'line',
        source: 'links',
        filter: ['==', ['get', 'kind'], 'depends_on'],
        paint: { 'line-color': '#ffaa00', 'line-width': 2, 'line-dasharray': [2, 2] },
      });
      map.addLayer({
        id: 'assets',
        type: 'circle',
        source: 'assets',
        paint: {
          'circle-radius': ['interpolate', ['linear'], ['zoom'], 3, 4, 12, 8, 16, 11],
          'circle-color': ['get', 'fill'],
          'circle-stroke-color': ['case', ['get', 'selected'], '#ffffff', ['get', 'ring']],
          'circle-stroke-width': ['case', ['get', 'selected'], 4, 2.5],
        },
      });
      map.addLayer({
        id: 'asset-labels',
        type: 'symbol',
        source: 'assets',
        minzoom: 12,
        layout: {
          'text-field': ['get', 'name'],
          'text-font': ['Noto Sans Regular'],
          'text-size': 11,
          'text-offset': [0, 1.3],
          'text-anchor': 'top',
          'text-optional': true,
        },
        paint: { 'text-color': '#d6f5dc', 'text-halo-color': '#050a06', 'text-halo-width': 1.5 },
      });

      map.on('click', 'assets', (e: MapLayerMouseEvent) => {
        const id = e.features?.[0]?.properties?.id;
        if (typeof id === 'string') onselect(id);
        e.preventDefault();
      });
      map.on('click', (e: MapMouseEvent) => {
        if (e.defaultPrevented) return;
        if (adding) onplace({ lat: e.lngLat.lat, lon: e.lngLat.lng });
        else onselect(null);
      });
      map.on('mouseenter', 'assets', () => map && (map.getCanvas().style.cursor = 'pointer'));
      map.on('mouseleave', 'assets', () => map && (map.getCanvas().style.cursor = ''));
      ready = true;
    });
  });

  onDestroy(() => map?.remove());

  // Push data whenever the exercise changes.
  $effect(() => {
    const assets = assetData;
    const links = linkData;
    if (!ready || !map) return;
    (map.getSource('assets') as GeoJSONSource | undefined)?.setData(assets);
    (map.getSource('links') as GeoJSONSource | undefined)?.setData(links);
  });

  // Frame the assets once, the first time there are any.
  $effect(() => {
    if (!ready || !map || framed) return;
    const pts = Object.values(exercise.assets).flatMap((a) => (a.geo ? [a.geo] : []));
    if (!pts.length) return;
    framed = true;
    const bounds = new LngLatBounds();
    for (const p of pts) bounds.extend([p.lon, p.lat]);
    map.fitBounds(bounds, { padding: 60, maxZoom: 14, duration: 0 });
  });

  $effect(() => {
    if (map) map.getCanvas().style.cursor = adding ? 'crosshair' : '';
  });
</script>

<div class="map" bind:this={container} role="application" aria-label="Exercise map"></div>

<style>
  .map { position: absolute; inset: 0; }
</style>
