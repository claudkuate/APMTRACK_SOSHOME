/// Domaine canonique de l'API. `apmtrack-api.soshome-cameroun.net` reste servi
/// pour les APK déjà distribués, mais tout nouveau build pointe ici.
///
/// ⚠️ Un APK construit sur un hôte sans certificat TLS échoue à chaque appel et
/// l'agent ne voit qu'un profil qui ne charge pas : vérifier l'hôte avant build
/// (`curl -sI https://api.apmtrack.cm/health`).
const apiBaseUrl = String.fromEnvironment(
  'API_URL',
  defaultValue: 'https://api.apmtrack.cm',
);

const appEnvironment = String.fromEnvironment(
  'APP_ENV',
  defaultValue: 'development',
);
