import {
  AfterViewInit,
  Component,
  ElementRef,
  OnDestroy,
  effect,
  input,
  viewChild,
} from '@angular/core';
import * as L from 'leaflet';

import {
  coloredDivIcon,
  createBaseMap,
  fitToLayer,
  LAYER_COLORS,
  polygonStyle,
} from './leaflet-shared';
import { GeoGeometry } from '../../core/services/geo.service';

/**
 * Mini-carte en lecture seule : affiche un point (lat/lon) et/ou un contour GeoJSON.
 * Utilisée dans les tiroirs et fiches de détail.
 */
@Component({
  selector: 'app-mini-map',
  standalone: true,
  template: `
    @if (hasData()) {
      <div
        #mapEl
        class="apm-mini-map overflow-hidden rounded-xl border border-[var(--line-subtle)]"
        [style.height]="height()"
      ></div>
    } @else {
      <p class="text-sm text-[var(--text-muted)]">Aucune localisation enregistrée.</p>
    }
  `,
})
export class MiniMapComponent implements AfterViewInit, OnDestroy {
  readonly latitude = input<number | null>(null);
  readonly longitude = input<number | null>(null);
  readonly boundary = input<GeoGeometry | null>(null);
  readonly layer = input<string>('pvs');
  readonly height = input<string>('220px');

  private readonly mapEl = viewChild<ElementRef<HTMLElement>>('mapEl');
  private map: L.Map | null = null;
  private overlay: L.LayerGroup | null = null;

  constructor() {
    // Redessine le contenu dès que les entrées changent (une fois la carte prête).
    effect(() => {
      // Suivi explicite des signaux.
      this.latitude();
      this.longitude();
      this.boundary();
      this.layer();
      if (this.map) {
        this.render();
      }
    });
  }

  protected hasData(): boolean {
    return (
      (this.latitude() != null && this.longitude() != null) || this.boundary() != null
    );
  }

  ngAfterViewInit(): void {
    const el = this.mapEl()?.nativeElement;
    if (!el || !this.hasData()) {
      return;
    }
    this.map = createBaseMap(el, { scrollWheelZoom: false });
    this.overlay = L.layerGroup().addTo(this.map);
    this.render();
    // La carte est souvent montée dans un conteneur animé (drawer) : recalcul de taille.
    setTimeout(() => this.map?.invalidateSize(), 150);
  }

  private render(): void {
    if (!this.map || !this.overlay) {
      return;
    }
    this.overlay.clearLayers();
    const layerKey = this.layer();
    const color = LAYER_COLORS[layerKey] ?? '#0a6b3b';

    const group = L.featureGroup();

    const boundary = this.boundary();
    if (boundary) {
      L.geoJSON(boundary as never, { style: () => polygonStyle(layerKey) }).addTo(group);
    }

    const lat = this.latitude();
    const lon = this.longitude();
    if (lat != null && lon != null) {
      L.marker([lat, lon], { icon: coloredDivIcon(color) }).addTo(group);
    }

    group.addTo(this.overlay);
    fitToLayer(this.map, group);
  }

  ngOnDestroy(): void {
    this.map?.remove();
    this.map = null;
  }
}
