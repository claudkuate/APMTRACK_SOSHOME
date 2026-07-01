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
    const apiMessage = extractApiErrorMessage(error);
    switch (error.status) {
      case 0:
        return 'API injoignable. Vérifie ta connexion puis réessaie.';
      case 401:
        return 'Session expirée. Reconnecte-toi pour continuer.';
      case 403:
        return apiMessage ?? "Droits insuffisants pour accéder à cette ressource.";
      case 404:
        return apiMessage ?? 'Ressource introuvable.';
      case 429:
        return 'Trop de requêtes. Patiente un instant avant de réessayer.';
      default:
        if (error.status >= 500) {
          return 'API momentanément indisponible. Réessaie dans un instant.';
        }
        // 400/409/422 : le message métier du serveur (ex. « Montant insuffisant :
        // 27500 FCFA requis… ») dit exactement quoi corriger — le montrer tel quel.
        if (apiMessage) {
          return apiMessage;
        }
    }
  }
  return `${subject} impossible. Réessaie dans un instant.`;
}

/** Extrait `error.message` du corps d'erreur normalisé de l'API ({ error: { code, message } }). */
function extractApiErrorMessage(error: HttpErrorResponse): string | null {
  const body = error.error as { error?: { message?: unknown } } | null | undefined;
  const message = body?.error?.message;
  return typeof message === 'string' && message.trim() ? message : null;
}
