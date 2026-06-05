import {
  AfterViewInit,
  Component,
  ElementRef,
  OnDestroy,
  effect,
  inject,
  input,
  model,
  output,
  signal,
  viewChild,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import * as L from 'leaflet';

import { coloredDivIcon, createBaseMap, DEFAULT_CENTER, DEFAULT_ZOOM } from './leaflet-shared';
import { GeoService, NominatimResult } from '../../core/services/geo.service';

/**
 * Sélecteur de position : clic / marqueur déplaçable + recherche d'adresse (Nominatim)
 * + bouton « ma position ». Deux liaisons bidirectionnelles `latitude` / `longitude`.
 */
@Component({
  selector: 'app-location-picker',
  standalone: true,
  imports: [FormsModule],
  template: `
    <div class="grid gap-2">
      <div class="relative">
        <input
          type="text"
          class="w-full rounded-lg border border-[var(--line-subtle)] px-3 py-2 text-sm"
          placeholder="Rechercher une adresse, un quartier..."
          [(ngModel)]="searchTerm"
          (ngModelChange)="onSearchChange($event)"
          (keyup.enter)="runSearch()"
        />
        @if (results().length) {
          <ul
            class="absolute z-[1000] mt-1 max-h-56 w-full overflow-y-auto rounded-lg border border-[var(--line-subtle)] bg-white shadow-lg"
          >
            @for (r of results(); track r.lat + '/' + r.lon) {
              <li>
                <button
                  type="button"
                  class="block w-full px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)]"
                  (click)="selectResult(r)"
                >
                  {{ r.displayName }}
                </button>
              </li>
            }
          </ul>
        }
      </div>

      <div
        #mapEl
        class="overflow-hidden rounded-xl border border-[var(--line-subtle)]"
        [style.height]="height()"
      ></div>

      <div class="flex flex-wrap items-center gap-2 text-xs">
        <button type="button" class="btn-secondary min-h-8 px-3 py-1" (click)="useMyPosition()">
          📍 Ma position
        </button>
        @if (latitude() != null && longitude() != null) {
          <span class="text-[var(--text-muted)]">
            {{ latitude()!.toFixed(5) }}, {{ longitude()!.toFixed(5) }}
          </span>
          <button type="button" class="btn-ghost min-h-8 px-2 py-1" (click)="clear()">
            Effacer
          </button>
        } @else {
          <span class="text-[var(--text-muted)]">Cliquez sur la carte pour définir la position.</span>
        }
        @if (statusMessage()) {
          <span class="text-[var(--cameroon-red)]">{{ statusMessage() }}</span>
        }
      </div>
    </div>
  `,
})
export class LocationPickerComponent implements AfterViewInit, OnDestroy {
  readonly latitude = model<number | null>(null);
  readonly longitude = model<number | null>(null);
  readonly height = input<string>('300px');
  /** Émis avec l'adresse trouvée (sélection de résultat ou reverse-geocode au clic). */
  readonly addressResolved = output<string>();

  private readonly geo = inject(GeoService);
  private readonly mapEl = viewChild<ElementRef<HTMLElement>>('mapEl');

  protected searchTerm = '';
  protected readonly results = signal<NominatimResult[]>([]);
  protected readonly statusMessage = signal<string | null>(null);

  private map: L.Map | null = null;
  private marker: L.Marker | null = null;
  private searchTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    // Synchronise le marqueur si les coordonnées changent depuis l'extérieur.
    effect(() => {
      const lat = this.latitude();
      const lon = this.longitude();
      if (this.map && lat != null && lon != null) {
        this.placeMarker(lat, lon, false);
      }
    });
  }

  ngAfterViewInit(): void {
    const el = this.mapEl()?.nativeElement;
    if (!el) {
      return;
    }
    const lat = this.latitude();
    const lon = this.longitude();
    this.map = createBaseMap(el, {
      center: lat != null && lon != null ? [lat, lon] : DEFAULT_CENTER,
      zoom: lat != null && lon != null ? 15 : DEFAULT_ZOOM,
    });
    this.map.on('click', (e: L.LeafletMouseEvent) => {
      this.placeMarker(e.latlng.lat, e.latlng.lng, true);
      this.reverseGeocode(e.latlng.lat, e.latlng.lng);
    });
    if (lat != null && lon != null) {
      this.placeMarker(lat, lon, false);
    }
    setTimeout(() => this.map?.invalidateSize(), 150);
  }

  private placeMarker(lat: number, lon: number, propagate: boolean): void {
    if (!this.map) {
      return;
    }
    if (this.marker) {
      this.marker.setLatLng([lat, lon]);
    } else {
      this.marker = L.marker([lat, lon], {
        icon: coloredDivIcon('#0a6b3b'),
        draggable: true,
      }).addTo(this.map);
      this.marker.on('dragend', () => {
        const pos = this.marker!.getLatLng();
        this.latitude.set(round(pos.lat));
        this.longitude.set(round(pos.lng));
        this.reverseGeocode(pos.lat, pos.lng);
      });
    }
    if (propagate) {
      this.latitude.set(round(lat));
      this.longitude.set(round(lon));
    }
  }

  protected onSearchChange(value: string): void {
    this.searchTerm = value;
    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
    }
    if (value.trim().length < 3) {
      this.results.set([]);
      return;
    }
    this.searchTimer = setTimeout(() => this.runSearch(), 400);
  }

  protected runSearch(): void {
    const term = this.searchTerm.trim();
    if (term.length < 3) {
      return;
    }
    this.geo.geocode(term).subscribe({
      next: (rows) => this.results.set(rows),
      error: () => this.statusMessage.set('Recherche d’adresse indisponible.'),
    });
  }

  protected selectResult(r: NominatimResult): void {
    this.results.set([]);
    this.searchTerm = r.displayName;
    this.placeMarker(r.lat, r.lon, true);
    this.map?.setView([r.lat, r.lon], 16);
    this.addressResolved.emit(r.displayName);
  }

  protected useMyPosition(): void {
    if (!navigator.geolocation) {
      this.statusMessage.set('Géolocalisation non disponible sur cet appareil.');
      return;
    }
    this.statusMessage.set(null);
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        const { latitude, longitude } = pos.coords;
        this.placeMarker(latitude, longitude, true);
        this.map?.setView([latitude, longitude], 16);
        this.reverseGeocode(latitude, longitude);
      },
      () => this.statusMessage.set('Position refusée ou indisponible.'),
      { enableHighAccuracy: true, timeout: 8000 },
    );
  }

  protected clear(): void {
    this.latitude.set(null);
    this.longitude.set(null);
    if (this.marker) {
      this.marker.remove();
      this.marker = null;
    }
  }

  private reverseGeocode(lat: number, lon: number): void {
    this.geo.reverseGeocode(lat, lon).subscribe({
      next: (address) => {
        if (address) {
          this.addressResolved.emit(address);
        }
      },
      error: () => {},
    });
  }

  ngOnDestroy(): void {
    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
    }
    this.map?.remove();
    this.map = null;
  }
}

function round(value: number): number {
  return Math.round(value * 1e7) / 1e7;
}
