import * as L from 'leaflet';

import { pluginL } from './leaflet-global';
import 'leaflet-draw';
import 'leaflet.markercluster';

/**
 * Panne rattrapée ici plutôt qu'en production : ces greffons s'installent sur un `L`
 * global et leurs ajouts n'apparaissent jamais sur le namespace ESM figé. La
 * compilation ne voit rien — l'erreur ne surgit qu'au `ngAfterViewInit`.
 */
describe('greffons Leaflet installés sur un objet extensible', () => {
  const plugin = pluginL as unknown as Record<string, unknown>;

  it('expose markerClusterGroup (page Carte)', () => {
    expect(typeof plugin['markerClusterGroup']).toBe('function');
  });

  it("expose Control.Draw (éditeur de contour), y compris via l'import cœur", () => {
    expect(typeof (pluginL.Control as unknown as Record<string, unknown>)['Draw']).toBe(
      'function',
    );
    // Les sous-objets sont partagés : l'import cœur voit le même Control.
    expect(L.Control).toBe(pluginL.Control);
  });

  it("garde les noms d'événements de dessin alignés sur ceux du greffon", () => {
    const draw = plugin['Draw'] as { Event?: Record<string, string> } | undefined;
    expect(draw?.Event?.['CREATED']).toBe('draw:created');
    expect(draw?.Event?.['EDITED']).toBe('draw:edited');
    expect(draw?.Event?.['DELETED']).toBe('draw:deleted');
  });
});
