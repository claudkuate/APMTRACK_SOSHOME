FROM node:24-alpine AS builder

WORKDIR /app

ARG NG_APP_API_URL=http://localhost:8080
ARG NG_APP_ENV=development

COPY apps/web-admin/package*.json ./
RUN npm ci

COPY apps/web-admin ./
RUN printf "window.__APMTRACK_CONFIG__ = { apiUrl: '%s', environment: '%s' };\n" "$NG_APP_API_URL" "$NG_APP_ENV" > public/env.js \
    && npm run build

FROM nginx:alpine

COPY infra/nginx/web-admin.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist/web-admin/browser /usr/share/nginx/html

EXPOSE 80
