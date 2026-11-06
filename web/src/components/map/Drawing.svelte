<script lang="ts">
  // Map markup through terra-draw. A finished shape is sent to the server as
  // an annotation and then dropped from terra-draw's own store: the shared
  // annotation layer in MapView is what everyone sees.
  import type { Map as MapLibre } from 'maplibre-gl';
  import { onDestroy, untrack } from 'svelte';
  import {
    TerraDraw,
    TerraDrawCircleMode,
    TerraDrawFreehandLineStringMode,
    TerraDrawLineStringMode,
    TerraDrawPointMode,
    TerraDrawPolygonMode,
    TerraDrawRectangleMode,
    type HexColor,
  } from 'terra-draw';
  import { TerraDrawMapLibreGLAdapter } from 'terra-draw-maplibre-gl-adapter';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Shape } from '../../lib/types/Shape';

  let { map }: { map: MapLibre } = $props();
  const ws = useWorkspace();

  // Our shape names → terra-draw mode names.
  const MODES: Record<Shape, string> = {
    point: 'point',
    line: 'linestring',
    freehand: 'freehand-linestring',
    polygon: 'polygon',
    rectangle: 'rectangle',
    circle: 'circle',
  };
  const SHAPE_OF = Object.fromEntries(Object.entries(MODES).map(([k, v]) => [v, k as Shape]));

  const styles = (c: string) => {
    const color = c as HexColor;
    return {
      fillColor: color,
      outlineColor: color,
      lineStringColor: color,
      pointColor: color,
      closingPointColor: color,
      fillOpacity: 0.2,
      outlineWidth: 2,
      lineStringWidth: 3,
    };
  };

  const draw = new TerraDraw({
    // Mounted once per map, so the map never changes underneath it.
    adapter: new TerraDrawMapLibreGLAdapter({ map: untrack(() => map) }),
    modes: [
      new TerraDrawPointMode({ styles: styles(ws.drawColor) }),
      new TerraDrawLineStringMode({ styles: styles(ws.drawColor) }),
      new TerraDrawFreehandLineStringMode({ styles: styles(ws.drawColor) }),
      new TerraDrawPolygonMode({ styles: styles(ws.drawColor) }),
      new TerraDrawRectangleMode({ styles: styles(ws.drawColor) }),
      new TerraDrawCircleMode({ styles: styles(ws.drawColor) }),
    ],
  });
  draw.start();

  draw.on('finish', (id, ctx) => {
    if (ctx.action !== 'draw') return;
    const feature = draw.getSnapshotFeature(id);
    draw.removeFeatures([id]);
    const shape = SHAPE_OF[ctx.mode];
    if (!feature || !shape) return;
    void ws.send({ type: 'add_annotation', shape, geometry: feature.geometry as GeoJSON.Geometry, color: ws.drawColor });
  });

  $effect(() => {
    draw.setMode(ws.mode.kind === 'draw' ? MODES[ws.mode.shape] : 'static');
  });

  $effect(() => {
    const s = styles(ws.drawColor);
    for (const mode of Object.values(MODES)) {
      (draw.updateModeOptions as (m: string, o: object) => void)(mode, { styles: s });
    }
  });

  onDestroy(() => draw.stop());
</script>
