const apiBaseUrl = String.fromEnvironment(
  'API_URL',
  defaultValue: 'http://192.168.1.113:8080',
);

const appEnvironment = String.fromEnvironment(
  'APP_ENV',
  defaultValue: 'development',
);
 