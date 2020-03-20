#!/bin/bash

DB="calendar_app_cache"
BUCKET="calendar-app-rust-db-backup"

TABLES="
calendar_cache
calendar_list
"

mkdir -p backup

for T in $TABLES;
do
    psql $DB -c "COPY $T TO STDOUT" | gzip > backup/${T}.sql.gz
    aws s3 cp backup/${T}.sql.gz s3://${BUCKET}/${T}.sql.gz
done
