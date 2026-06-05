import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, map } from 'rxjs';

import { ApiService } from './api.service';

/** Géométrie GeoJSON minimale (Point, Polygon, MultiPolygon, LineString...). */
export interface GeoGeometry {
  type: string;
  coordinates: unknown;
}

export interface GeoFeature {
  type: 'Feature';
  geometry: GeoGeometry | null;
  properties: Record<string, unknown>;
}

export interface GeoFeatureCollection {
  type: 'FeatureCollection';
  features: GeoFeature[];
}

/** Réponse de /geo/overview : une FeatureCollection par couche demandée. */
export type GeoOverview = Record<string, GeoFeatureCollection>;

export type GeoLayer = 'pvs' | 'signalements' | 'zones' | 'communes' | 'patrouilles';

export interface GeoOverviewParams {
  communeId?: string | null;
  bbox?: string | null;
  layers?: GeoLayer[];
  status?: string | null;
}

export interface NominatimResult {
  displayName: string;
  lat: number;
  lon: number;
}

@Injectable({ providedIn: 'root' })
export class GeoService {
  private readonly api = inject(ApiService);
  private readonly http = inject(HttpClient);

  private readonly nominatimBase = 'https://nominatim.openstreetmap.org';

  /** Récupère toutes les couches cartographiques en un appel. */
  overview(params: GeoOverviewParams = {}): Observable<GeoOverview> {
    return this.api.get<GeoOverview>('/api/v1/geo/overview', {
      commune_id: params.communeId ?? undefined,
      bbox: params.bbox ?? undefined,
      layers: params.layers?.length ? params.layers.join(',') : undefined,
      status: params.status ?? undefined,
    });
  }

  /** Récupère une seule couche (pvs, signalements, zones, communes). */
  layer(
    layer: Exclude<GeoLayer, 'patrouilles'>,
    params: Omit<GeoOverviewParams, 'layers'> = {},
  ): Observable<GeoFeatureCollection> {
    return this.api.get<GeoFeatureCollection>(`/api/v1/geo/${layer}`, {
      commune_id: params.communeId ?? undefined,
      bbox: params.bbox ?? undefined,
      status: params.status ?? undefined,
    });
  }

  /** Trace d'une patrouille (points + ligne reconstruite). */
  patrouilleTrack(patrouilleId: string): Observable<{
    patrouille_id: string;
    count: number;
    points: GeoFeatureCollection;
    line: GeoGeometry | null;
  }> {
    return this.api.get(`/api/v1/patrouilles/${patrouilleId}/track`);
  }

  /** Géocodage d'adresse via Nominatim (OSM). Usage modéré, à débouncer côté appelant. */
  geocode(query: string, limit = 5): Observable<NominatimResult[]> {
    const trimmed = query.trim();
    if (trimmed.length < 3) {
      return new Observable((sub) => {
        sub.next([]);
        sub.complete();
      });
    }
    return this.http
      .get<Array<{ display_name: string; lat: string; lon: string }>>(
        `${this.nominatimBase}/search`,
        { params: { format: 'json', q: trimmed, limit: String(limit), addressdetails: '0' } },
      )
      .pipe(
        map((rows) =>
          rows.map((r) => ({
            displayName: r.display_name,
            lat: Number(r.lat),
            lon: Number(r.lon),
          })),
        ),
      );
  }

  /** Géocodage inverse (coordonnées → adresse) via Nominatim. */
  reverseGeocode(lat: number, lon: number): Observable<string | null> {
    return this.http
      .get<{ display_name?: string }>(`${this.nominatimBase}/reverse`, {
        params: { format: 'json', lat: String(lat), lon: String(lon) },
      })
      .pipe(map((r) => r.display_name ?? null));
  }
}
