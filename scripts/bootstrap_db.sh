#!/bin/bash

if [ -z "$PASSWORD" ]; then
    PASSWORD=`head -c1000 /dev/urandom | tr -dc [:alpha:][:digit:] | head -c 16; echo ;`
fi
DB=calendar_app_cache

sudo apt-get install -y postgresql

sudo -u postgres createuser -E -e $USER
sudo -u postgres psql -c "CREATE ROLE $USER PASSWORD '$PASSWORD' NOSUPERUSER NOCREATEDB NOCREATEROLE INHERIT LOGIN;"
sudo -u postgres psql -c "ALTER ROLE $USER PASSWORD '$PASSWORD' NOSUPERUSER NOCREATEDB NOCREATEROLE INHERIT LOGIN;"
sudo -u postgres createdb $DB
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE $DB TO $USER;"
sudo -u postgres psql $DB -c "GRANT ALL ON SCHEMA public TO $USER;"

mkdir -p ${HOME}/.config/calendar_app_rust
cat > ${HOME}/.config/calendar_app_rust/config.env <<EOL
DATABASE_URL=postgresql://$USER:$PASSWORD@localhost:5432/$DB
GCAL_SECRET_FILE=${HOME}/.config/calendar_app_rust/client_secrets.json
GCAL_TOKEN_PATH=${HOME}/.gcal
EOL

cat > ${HOME}/.config/calendar_app_rust/postgres.toml <<EOL
[calendar_app_rust]
database_url = 'postgresql://$USER:$PASSWORD@localhost:5432/$DB'
destination = 'file:///home/ddboline/setup_files/build/calendar_app_rust/backup'
tables = ['calendar_cache', 'calendar_list']
sequences = {calendar_list_id_seq=['calendar_list', 'id'], calendar_cache_id_seq=['calendar_cache', 'id']}
EOL
