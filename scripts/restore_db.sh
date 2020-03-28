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
    aws s3 cp s3://${BUCKET}/${T}.sql.gz backup/${T}.sql.gz
    gzip -dc backup/${T}.sql.gz | psql $DB -c "COPY $T FROM STDIN"
done

psql $DB -c "select setval('calendar_list_id_seq', (select max(id) from calendar_list), TRUE)"
psql $DB -c "select setval('calendar_cache_id_seq', (select max(id) from calendar_cache), TRUE)"
