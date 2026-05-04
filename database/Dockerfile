FROM docker.io/library/postgres:16-alpine

COPY database/schema.sql /docker-entrypoint-initdb.d/01-schema.sql
COPY database/init.sql /docker-entrypoint-initdb.d/02-init.sql
COPY database/migrations /docker-entrypoint-initdb.d/migrations
