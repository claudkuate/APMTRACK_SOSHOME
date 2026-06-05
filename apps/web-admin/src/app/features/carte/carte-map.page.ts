import {
  AfterViewInit,
  Component,
  ElementRef,
  OnDestroy,
  effect,
  inject,
  signal,
  viewChild,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import * as L from 'leaflet';
import 'leaflet.markercluster';

import {
  coloredDivIcon,
  createBaseMap,
  fitToLayer,
  LAYER_COLORS,
  polygonStyle,
} from '../../shared/map/leaflet-shared';
import { CommuneContextService } from '../../core/services/commune-context.service';
import {
  GeoFeature,
  GeoFeatureCollection,
  GeoLayer,
  GeoOverview,
  GeoService,
} from '../../core/services/geo.service';

interface LayerToggle {
  key: GeoLayer;
  label: string;
  kind: 'point' | 'polygon' | 'line';
}

@Component({
  selector: 'app-carte-map',
  standalone: true,
  imports: [FormsModule],
  template: `
    <section class="grid gap-4">
      <header class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Cartographie</p>
          <h1 class="text-2xl font-black">Carte opérationnelle</h1>
          <p class="text-sm text-[var(--text-muted)]">
            Vue géographique des PV, signalements, zones et patrouilles.
          </p>
        </div>
        <div class="flex items-center gap-2">
          <label class="text-xs font-semibold text-[var(--text-muted)]" for="carte-status">Statut</label>
          <select
            id="carte-status"
            class="rounded-lg border border-[var(--line-subtle)] px-3 py-2 text-sm"
            [(ngModel)]="statusFilter"
            (ngModelChange)="reload()"
          >
            <option value="">Tous</option>
            <option value="EN_ATTENTE_PAIEMENT">PV — en attente</option>
            <option value="PAYE">PV — payé</option>
            <option value="EN_RETARD">PV — en retard</option>
            <option value="RECU">Signalement — reçu</option>
            <option value="EN_COURS">Signalement — en cours</option>
            <option value="TRAITE">Signalement — traité</option>
          </select>
          <button type="button" class="btn-secondary min-h-9 px-3" (click)="reload()">Actualiser</button>
        </div>
      </header>

      <div class="grid gap-3 lg:grid-cols-[260px_1fr]">
        <aside class="panel grid h-max gap-3 p-4">
          <p class="text-xs font-black uppercase text-[var(--text-muted)]">Couches</p>
          @for (t of toggles; track t.key) {
            <label class="flex items-center gap-2 text-sm">
              <input type="checkbox" [(ngModel)]="enabled[t.key]" (ngModelChange)="reload()" />
              <span
                class="inline-block h-3 w-3 rounded-full"
                [style.background]="colorFor(t.key)"
              ></span>
              <span>{{ t.label }}</span>
            </label>
          }
          @if (loading()) {
            <p class="text-xs text-[var(--text-muted)]">Chargement…</p>
          }
          @if (errorMessage()) {
            <p class="text-xs text-[var(--cameroon-red)]">{{ errorMessage() }}</p>
          }
          <p class="mt-2 text-[0.7rem] leading-tight text-[var(--text-muted)]">
            Fond de carte © OpenStreetMap. Déplacez la carte pour charger la zone visible.
          </p>
        </aside>

        <div
          #mapEl
          class="overflow-hidden rounded-2xl border border-[var(--line-subtle)]"
          style="height: calc(100vh - 230px); min-height: 420px;"
        ></div>
      </div>
    </section>
  `,
})
export class CarteMapPage implements AfterViewInit, OnDestroy {
  private readonly geo = inject(GeoService);
  private readonly commune = inject(CommuneContextService);
  private readonly router = inject(Router);
  private readonly mapEl = viewChild<ElementRef<HTMLElement>>('mapEl');

  protected readonly toggles: LayerToggle[] = [
    { key: 'pvs', label: 'Procès-verbaux', kind: 'point' },
    { key: 'signalements', label: 'Signalements', kind: 'point' },
    { key: 'zones', label: 'Zones', kind: 'polygon' },
    { key: 'communes', label: 'Communes', kind: 'polygon' },
    { key: 'patrouilles', label: 'Patrouilles', kind: 'line' },
  ];
  protected enabled: Record<GeoLayer, boolean> = {
    pvs: true,
    signalements: true,
    zones: true,
    communes: false,
    patrouilles: false,
  };
  protected statusFilter = '';
  protected readonly loading = signal(false);
  protected readonly errorMessage = signal<string | null>(null);

  private map: L.Map | null = null;
  private readonly groups = new Map<GeoLayer, L.LayerGroup>();
  private reloadTimer: ReturnType<typeof setTimeout> | null = null;
  private firstLoad = true;

  constructor() {
    // Recharge quand la commune active change.
    effect(() => {
      this.commune.communeId();
      if (this.map) {
        this.reload();
      }
    });
  }

  ngAfterViewInit(): void {
    const el = this.mapEl()?.nativeElement;
    if (!el) {
      return;
    }
    this.map = createBaseMap(el);
    this.map.on('moveend', () => this.scheduleReload());
    setTimeout(() => {
      this.map?.invalidateSize();
      this.reload();
    }, 150);
  }

  protected colorFor(key: GeoLayer): string {
    return LAYER_COLORS[key] ?? '#0a6b3b';
  }

  private scheduleReload(): void {
    if (this.reloadTimer) {
      clearTimeout(this.reloadTimer);
    }
    this.reloadTimer = setTimeout(() => this.reload(), 350);
  }

  protected reload(): void {
    if (!this.map) {
      return;
    }
    const layers = this.toggles.map((t) => t.key).filter((k) => this.enabled[k]);
    if (!layers.length) {
      this.clearAll();
      return;
    }
    this.loading.set(true);
    this.errorMessage.set(null);
    this.geo
      .overview({
        communeId: this.commune.communeId() ?? null,
        bbox: this.map.getBounds().toBBoxString(),
        layers,
        status: this.statusFilter || null,
      })
      .subscribe({
        next: (data) => {
          this.loading.set(false);
          this.render(data);
        },
        error: () => {
          this.loading.set(false);
          this.errorMessage.set('Chargement de la carte indisponible.');
        },
      });
  }

  private clearAll(): void {
    for (const group of this.groups.values()) {
      group.remove();
    }
    this.groups.clear();
  }

  private render(data: GeoOverview): void {
    if (!this.map) {
      return;
    }
    this.clearAll();
    const aggregate = L.featureGroup();

    for (const toggle of this.toggles) {
      if (!this.enabled[toggle.key]) {
        continue;
      }
      const fc = data[toggle.key];
      if (!fc?.features?.length) {
        continue;
      }
      const group =
        toggle.kind === 'point'
          ? this.buildPointLayer(toggle.key, fc, aggregate)
          : this.buildVectorLayer(toggle.key, fc, aggregate);
      group.addTo(this.map);
      this.groups.set(toggle.key, group);
    }

    // Au premier chargement, on cadre la vue sur les données.
    if (this.firstLoad && aggregate.getLayers().length) {
      fitToLayer(this.map, aggregate);
      this.firstLoad = false;
    }
  }

  private buildPointLayer(
    key: GeoLayer,
    fc: GeoFeatureCollection,
    aggregate: L.FeatureGroup,
  ): L.LayerGroup {
    const cluster = L.markerClusterGroup({ showCoverageOnHover: false });
    const icon = coloredDivIcon(this.colorFor(key));
    for (const feature of fc.features) {
      const coords = this.pointCoords(feature);
      if (!coords) {
        continue;
      }
      const marker = L.marker([coords[1], coords[0]], { icon });
      marker.bindPopup(this.popupHtml(feature));
      cluster.addLayer(marker);
      aggregate.addLayer(L.marker([coords[1], coords[0]], { opacity: 0 }));
    }
    this.wirePopupNavigation(cluster);
    return cluster;
  }

  private buildVectorLayer(
    key: GeoLayer,
    fc: GeoFeatureCollection,
    aggregate: L.FeatureGroup,
  ): L.LayerGroup {
    const group = L.geoJSON(fc as never, {
      style: () => polygonStyle(key),
      onEachFeature: (feature, layer) => {
        layer.bindPopup(this.popupHtml(feature as unknown as GeoFeature));
        aggregate.addLayer(layer);
      },
    });
    this.wirePopupNavigation(group);
    return group;
  }

  private pointCoords(feature: GeoFeature): [number, number] | null {
    const geom = feature.geometry;
    if (!geom || geom.type !== 'Point' || !Array.isArray(geom.coordinates)) {
      return null;
    }
    const [lon, lat] = geom.coordinates as number[];
    return [lon, lat];
  }

  private popupHtml(feature: GeoFeature): string {
    const p = feature.properties ?? {};
    const route = typeof p['route'] === 'string' ? (p['route'] as string) : '';
    const title =
      (p['pv_number'] as string) ||
      (p['signalement_number'] as string) ||
      (p['nom'] as string) ||
      'Élément';
    const subtitleParts: string[] = [];
    if (p['type_incident']) subtitleParts.push(String(p['type_incident']));
    if (p['type_zone']) subtitleParts.push(String(p['type_zone']));
    if (p['status']) subtitleParts.push(String(p['status']));
    if (p['region']) subtitleParts.push(String(p['region']));
    const subtitle = subtitleParts.join(' · ');
    const button = route
      ? `<button class="apm-popup-link" data-route="${route}"
            style="margin-top:6px;color:#0a6b3b;font-weight:700;cursor:pointer;background:none;border:none;padding:0;">
            Ouvrir la fiche →</button>`
      : '';
    return `<div style="min-width:160px">
        <strong>${escapeHtml(title)}</strong>
        ${subtitle ? `<div style="color:#5b6b60;font-size:12px">${escapeHtml(subtitle)}</div>` : ''}
        ${button}
      </div>`;
  }

  private wirePopupNavigation(layer: L.Layer): void {
    layer.on('popupopen', (e: L.LeafletEvent) => {
      const popup = (e as L.PopupEvent).popup;
      const el = popup.getElement()?.querySelector<HTMLButtonElement>('.apm-popup-link');
      if (el) {
        el.onclick = () => {
          const route = el.dataset['route'];
          if (route) {
            this.router.navigateByUrl(route);
          }
        };
      }
    });
  }

  ngOnDestroy(): void {
    if (this.reloadTimer) {
      clearTimeout(this.reloadTimer);
    }
    this.clearAll();
    this.map?.remove();
    this.map = null;
  }
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
