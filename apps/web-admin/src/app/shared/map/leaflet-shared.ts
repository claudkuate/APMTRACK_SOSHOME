import * as L from 'leaflet';

/** Centre par défaut : Yaoundé, Cameroun. */
export const DEFAULT_CENTER: L.LatLngTuple = [3.848, 11.502];
export const DEFAULT_ZOOM = 12;

/** Couleurs par couche (alignées sur l'identité visuelle de l'app). */
export const LAYER_COLORS: Record<string, string> = {
  pvs: '#ce1126', // rouge — verbalisation
  signalements: '#fcd116', // or — signalement citoyen
  zones: '#0a6b3b', // vert — zone administrative
  communes: '#117a43', // vert — commune
  patrouilles: '#1d4ed8', // bleu — trace patrouille
  picker: '#0a6b3b',
};

const OSM_TILES = 'https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png';
const OSM_ATTRIBUTION =
  '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors';

/**
 * Crée une carte Leaflet de base avec la couche de tuiles OSM (gratuite, sans clé)
 * et l'attribution obligatoire. À détruire via `map.remove()` côté composant.
 */
export function createBaseMap(
  element: HTMLElement,
  options: L.MapOptions = {},
): L.Map {
  const map = L.map(element, {
    center: DEFAULT_CENTER,
    zoom: DEFAULT_ZOOM,
    zoomControl: true,
    ...options,
  });
  L.tileLayer(OSM_TILES, {
    maxZoom: 19,
    attribution: OSM_ATTRIBUTION,
  }).addTo(map);
  return map;
}

/**
 * Marqueur SVG coloré (DivIcon) — évite la dépendance aux PNG par défaut de Leaflet
 * (souvent cassés par les bundlers) et reste cohérent avec le style de l'app.
 */
export function coloredDivIcon(color: string): L.DivIcon {
  const svg = `
    <svg width="26" height="36" viewBox="0 0 26 36" xmlns="http://www.w3.org/2000/svg">
      <path d="M13 0C5.8 0 0 5.8 0 13c0 9.7 13 23 13 23s13-13.3 13-23C26 5.8 20.2 0 13 0z"
            fill="${color}" stroke="white" stroke-width="2"/>
      <circle cx="13" cy="13" r="5" fill="white"/>
    </svg>`;
  return L.divIcon({
    html: svg,
    className: 'apm-map-pin',
    iconSize: [26, 36],
    iconAnchor: [13, 36],
    popupAnchor: [0, -32],
  });
}

/** Style d'un polygone (zone/commune) selon la couche. */
export function polygonStyle(layer: string): L.PathOptions {
  const color = LAYER_COLORS[layer] ?? '#0a6b3b';
  return {
    color,
    weight: 2,
    opacity: 0.9,
    fillColor: color,
    fillOpacity: 0.12,
  };
}

/** Ajuste la vue de la carte à une couche GeoJSON si elle contient des données. */
export function fitToLayer(map: L.Map, layer: L.GeoJSON | L.FeatureGroup): void {
  try {
    const bounds = layer.getBounds();
    if (bounds.isValid()) {
      map.fitBounds(bounds, { padding: [32, 32], maxZoom: 16 });
    }
  } catch {
    // Couche vide : on garde la vue par défaut.
  }
}
