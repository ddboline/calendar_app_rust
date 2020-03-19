#!/bin/bash

DB="calendar_app_cache"

TABLES="
calendar_cache
calendar_list
"

for T in $TABLES;
do
    psql $DB -c "DELETE FROM $T";
done
