#!/bin/bash

if [ -z "$PASSWORD" ]; then
    PASSWORD=`head -c1000 /dev/urandom | tr -dc [:alpha:][:digit:] | head -c 16; echo ;`
fi
DB=calendar_app_cache

docker run --name $DB -p 12345:5432 -e POSTGRES_PASSWORD=$PASSWORD -d postgres
sleep 10
DATABASE_URL="postgresql://postgres:$PASSWORD@localhost:12345/postgres"

psql $DATABASE_URL -c "CREATE DATABASE $DB"

DATABASE_URL="postgresql://postgres:$PASSWORD@localhost:12345/$DB"

mkdir -p ${HOME}/.config/calendar_app_rust
cat > ${HOME}/.config/calendar_app_rust/config.env <<EOL
DATABASE_URL=$DATABASE_URL
EOL

cat > ${HOME}/.config/calendar_app_rust/postgres.toml <<EOL
[calendar_app_rust]
database_url = '$DATABASE_URL'
destination = 'file:///home/ddboline/setup_files/build/calendar_app_rust/backup'
tables = ['calendar_cache', 'calendar_list']
sequences = {calendar_list_id_seq=['calendar_list', 'id'], calendar_cache_id_seq=['calendar_cache', 'id']}
EOL
