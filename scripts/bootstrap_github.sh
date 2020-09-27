#!/bin/bash

PASSWORD=`head -c1000 /dev/urandom | tr -dc [:alpha:][:digit:] | head -c 16; echo ;`
JWT_SECRET=`head -c1000 /dev/urandom | tr -dc [:alpha:][:digit:] | head -c 32; echo ;`
SECRET_KEY=`head -c1000 /dev/urandom | tr -dc [:alpha:][:digit:] | head -c 32; echo ;`
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
