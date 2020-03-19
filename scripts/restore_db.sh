#!/bin/bash

DB="calendar_app_cache"
BUCKET="calendar-app-rust-db-backup"

TABLES="
calendar_cache
calendar_list
"

mkdir -p backup/

for T in $TABLES;
do
    calendar s3 cp s3://${BUCKET}/${T}.sql.gz backup/${T}.sql.gz
    gzip -dc backup/${T}.sql.gz | psql $DB -c "COPY $T FROM STDIN"
done
