import { HttpErrorResponse } from '@angular/common/http';

/**
 * Traduit une erreur HTTP en message utilisateur actionnable.
 *
 * Les handlers `error` des pages affichaient un texte fourre-tout (« Verifie les droits
 * ou la disponibilite API ») quel que soit le statut. On distingue désormais les cas
 * courants pour que l'utilisateur sache quoi faire (se reconnecter, contacter un admin,
 * réessayer). Le refresh silencieux du jeton est déjà géré par `authInterceptor` : un 401
 * qui remonte jusqu'ici signifie que la session est réellement expirée.
 */
export function describeHttpError(error: unknown, subject = 'Chargement'): string {
  if (error instanceof HttpErrorResponse) {
    switch (error.status) {
      case 0:
        return 'API injoignable. Vérifie ta connexion puis réessaie.';
      case 401:
        return 'Session expirée. Reconnecte-toi pour continuer.';
      case 403:
        return "Droits insuffisants pour accéder à cette ressource.";
      case 404:
        return 'Ressource introuvable.';
      case 429:
        return 'Trop de requêtes. Patiente un instant avant de réessayer.';
      default:
        if (error.status >= 500) {
          return 'API momentanément indisponible. Réessaie dans un instant.';
        }
    }
  }
  return `${subject} impossible. Réessaie dans un instant.`;
}
