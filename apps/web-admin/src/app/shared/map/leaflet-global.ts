import * as leaflet from 'leaflet';

/**
 * Objet Leaflet extensible, seul endroit où les greffons UMD peuvent s'installer.
 *
 * `leaflet-draw` et `leaflet.markercluster` écrivent leurs ajouts (`L.Draw`,
 * `L.markerClusterGroup`) sur un `L` **global**. Or `import * as L from 'leaflet'`
 * produit un namespace ESM **figé** : l'écriture y échoue silencieusement, puis le
 * greffon plante en voulant compléter ce qu'il croit avoir créé. On expose donc une
 * copie extensible, publiée en global avant le chargement des greffons.
 *
 * Les sous-objets (`Control`, `FeatureGroup`, …) restent les mêmes références que
 * dans le namespace : `import * as L from 'leaflet'` demeure valable pour tout le
 * cœur de Leaflet et pour les types. Seuls les membres **ajoutés par un greffon**
 * doivent être lus ici.
 *
 * Ce module doit être importé AVANT tout `import 'leaflet-*'` — l'évaluation ESM suit
 * l'ordre de déclaration des imports.
 */
export const pluginL: typeof leaflet = { ...leaflet };

(globalThis as unknown as { L: unknown }).L = pluginL;
