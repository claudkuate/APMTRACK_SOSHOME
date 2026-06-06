const apiBaseUrl = String.fromEnvironment(
  'API_URL',
  defaultValue: 'https://apmtrack-api.soshome-cameroun.net',
);

const appEnvironment = String.fromEnvironment(
  'APP_ENV',
  defaultValue: 'development',
);
