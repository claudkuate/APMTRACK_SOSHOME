import {
  AfterViewInit,
  Component,
  ElementRef,
  OnDestroy,
  effect,
  input,
  model,
  viewChild,
} from '@angular/core';
import * as L from 'leaflet';
import 'leaflet-draw';

import { createBaseMap, fitToLayer, polygonStyle } from './leaflet-shared';
import { GeoGeometry } from '../../core/services/geo.service';

/**
 * Éditeur de contour : trace / modifie un polygone et expose sa géométrie GeoJSON
 * via la liaison bidirectionnelle `boundary`.
 */
@Component({
  selector: 'app-zone-editor',
  standalone: true,
  template: `
    <div class="grid gap-2">
      <div
        #mapEl
        class="overflow-hidden rounded-xl border border-[var(--line-subtle)]"
        [style.height]="height()"
      ></div>
      <div class="flex items-center gap-2 text-xs text-[var(--text-muted)]">
        <span>Utilisez l'outil polygone (en haut à droite) pour dessiner le contour.</span>
        @if (boundary()) {
          <button type="button" class="btn-ghost min-h-8 px-2 py-1" (click)="clear()">
            Effacer le contour
          </button>
        }
      </div>
    </div>
  `,
})
export class ZoneEditorComponent implements AfterViewInit, OnDestroy {
  readonly boundary = model<GeoGeometry | null>(null);
  readonly layer = input<string>('zones');
  readonly height = input<string>('320px');

  private readonly mapEl = viewChild<ElementRef<HTMLElement>>('mapEl');
  private map: L.Map | null = null;
  private drawn: L.FeatureGroup | null = null;
  private syncingFromInput = false;

  constructor() {
    effect(() => {
      const value = this.boundary();
      if (this.map && this.drawn && !this.syncingFromInput) {
        this.loadBoundary(value);
      }
    });
  }

  ngAfterViewInit(): void {
    const el = this.mapEl()?.nativeElement;
    if (!el) {
      return;
    }
    this.map = createBaseMap(el);
    this.drawn = new L.FeatureGroup().addTo(this.map);

    const drawControl = new L.Control.Draw({
      position: 'topright',
      draw: {
        polygon: { allowIntersection: false, showArea: true },
        polyline: false,
        rectangle: {} as L.DrawOptions.RectangleOptions,
        circle: false,
        circlemarker: false,
        marker: false,
      },
      edit: { featureGroup: this.drawn },
    });
    this.map.addControl(drawControl);

    this.map.on(L.Draw.Event.CREATED, (e: L.LeafletEvent) => {
      const layer = (e as L.DrawEvents.Created).layer;
      this.drawn!.clearLayers();
      this.drawn!.addLayer(layer);
      this.emitGeometry();
    });
    this.map.on(L.Draw.Event.EDITED, () => this.emitGeometry());
    this.map.on(L.Draw.Event.DELETED, () => this.emitGeometry());

    this.loadBoundary(this.boundary());
    setTimeout(() => this.map?.invalidateSize(), 150);
  }

  private loadBoundary(value: GeoGeometry | null): void {
    if (!this.drawn || !this.map) {
      return;
    }
    this.drawn.clearLayers();
    if (value) {
      L.geoJSON(value as never, {
        style: () => polygonStyle(this.layer()),
        onEachFeature: (_f, layer) => this.drawn!.addLayer(layer),
      });
      fitToLayer(this.map, this.drawn);
    }
  }

  private emitGeometry(): void {
    if (!this.drawn) {
      return;
    }
    const layers = this.drawn.getLayers();
    if (!layers.length) {
      this.setBoundary(null);
      return;
    }
    const geojson = (layers[0] as L.Polygon).toGeoJSON();
    this.setBoundary(geojson.geometry as GeoGeometry);
  }

  private setBoundary(value: GeoGeometry | null): void {
    this.syncingFromInput = true;
    this.boundary.set(value);
    this.syncingFromInput = false;
  }

  protected clear(): void {
    this.drawn?.clearLayers();
    this.setBoundary(null);
  }

  ngOnDestroy(): void {
    this.map?.remove();
    this.map = null;
  }
}
