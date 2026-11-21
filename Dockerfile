# equievents.ch: one image for the web and the worker (`equievents-server web|worker`), as
# Gestion Concours'. Pinned images: a rebuild never silently moves to an unvetted version.
# The toolchain is the project's (1.84, Cargo.toml's rust-version).
ARG RUST_IMAGE=rust:1.84.0-alpine3.21
# The runtime has the Postgres client of the server's version (18): the worker's nightly pg_dump.
ARG ALPINE_IMAGE=alpine:3.24
# The interface (app/) is built with Node, as the release workflow builds it for the desktop app.
ARG NODE_IMAGE=node:22.11.0-alpine3.20

FROM ${RUST_IMAGE} AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p server && cp target/release/equievents-server /equievents-server

# The online version's interface (the user, 2026-11-21): the app's UI, built to be served at /app/.
FROM ${NODE_IMAGE} AS interface
WORKDIR /app
COPY app/package.json app/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY app/ ./
RUN npx tsc --noEmit && npx vite build --base /app/

# The desktop apps' release on the download page and for the apps' updates: a GitHub release's
# files, the one RELEASE_TAG names (a rebuild never moves to another release by itself); a
# private repository's token as the build secret github_token (docs/architecture/releases.md).
FROM ${ALPINE_IMAGE} AS downloads
ARG RELEASES_REPO=
ARG RELEASE_TAG=
RUN apk add --no-cache curl jq
COPY scripts/fetch-release.sh /fetch-release.sh
RUN --mount=type=secret,id=github_token sh /fetch-release.sh "$RELEASES_REPO" "$RELEASE_TAG" /downloads

FROM ${ALPINE_IMAGE}
RUN apk upgrade --no-cache && apk add --no-cache postgresql18-client tzdata \
 && adduser -D -u 10001 equievents && mkdir -p /data/backups /data/downloads /data/offices && chown -R equievents /data
COPY --from=build /equievents-server /usr/local/bin/equievents-server
COPY --from=downloads --chown=equievents /downloads/ /data/downloads/
COPY --from=interface /app/dist/ /srv/app/
ENV TZ=Europe/Zurich PORT=3000 BACKUP_DIR=/data/backups DOWNLOADS_DIR=/data/downloads OFFICES_DIR=/data/offices WEB_DIR=/srv/app
USER equievents
EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/equievents-server"]
CMD ["web"]
