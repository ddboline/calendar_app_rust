use anyhow::{format_err, Error};
use derive_more::Into;
use futures::Stream;
use postgres_query::{
    client::GenericClient, query, query_dyn, Error as PqError, FromSqlRow, Parameter, Query,
};
use serde::{Deserialize, Serialize};
use stack_string::{format_sstr, StackString};
use std::{cmp, convert::TryInto, io};
use time::OffsetDateTime;

use gcal_lib::date_time_wrapper::DateTimeWrapper;

use crate::pgpool::{PgPool, PgTransaction};

#[derive(FromSqlRow, Clone, Debug, Serialize, Deserialize)]
pub struct CalendarList {
    pub calendar_name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub gcal_description: Option<StackString>,
    pub gcal_location: Option<StackString>,
    pub gcal_timezone: Option<StackString>,
    pub sync: bool,
    pub last_modified: DateTimeWrapper,
    pub edit: bool,
    pub display: bool,
}

impl CalendarList {
    #[must_use]
    pub fn new(calendar_name: &str, gcal_id: &str) -> Self {
        Self {
            calendar_name: calendar_name.into(),
            gcal_id: gcal_id.into(),
            gcal_name: None,
            gcal_description: None,
            gcal_location: None,
            gcal_timezone: None,
            sync: false,
            last_modified: DateTimeWrapper::now(),
            edit: false,
            display: false,
        }
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_calendars(
        pool: &PgPool,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query = query!("SELECT * FROM calendar_list");
        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    fn get_calendar_list_query<'a>(
        select_str: &'a str,
        order_str: &'a str,
        modified: &'a Option<OffsetDateTime>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Query<'a>, PqError> {
        let mut bindings = Vec::new();
        let mut constraints = Vec::new();
        if let Some(modified) = modified {
            constraints.push(format_sstr!("last_modified > $modified"));
            bindings.push(("modified", modified as Parameter));
        }
        let where_str = if constraints.is_empty() {
            "".into()
        } else {
            format_sstr!("WHERE {}", constraints.join(" AND "))
        };
        let mut query = format_sstr!(
            r#"
                SELECT {select_str} FROM calendar_list
                {where_str}
                {order_str}
            "#
        );
        if let Some(offset) = offset {
            query.push_str(&format_sstr!(" OFFSET {offset}"));
        }
        if let Some(limit) = limit {
            query.push_str(&format_sstr!(" LIMIT {limit}"));
        }
        query_dyn!(&query, ..bindings)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_total(
        pool: &PgPool,
        modified: Option<OffsetDateTime>,
    ) -> Result<usize, Error> {
        #[derive(FromSqlRow)]
        struct Count {
            count: i64,
        }

        let query = Self::get_calendar_list_query("count(*)", "", &modified, None, None)?;

        let conn = pool.get().await?;
        let count: Count = query.fetch_one(&conn).await?;
        Ok(count.count.try_into()?)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_gcal_id(gcal_id: &str, pool: &PgPool) -> Result<Option<Self>, Error> {
        let conn = pool.get().await?;
        Self::get_by_gcal_id_conn(gcal_id, &conn)
            .await
            .map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_gcal_id_conn<C>(gcal_id: &str, conn: &C) -> Result<Option<Self>, Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            "SELECT * FROM calendar_list WHERE gcal_id = $gcal_id",
            gcal_id = gcal_id
        );
        query.fetch_opt(conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn update_display(&self, pool: &PgPool) -> Result<(), Error> {
        let query = query!(
            r#"
                UPDATE calendar_list
                SET display=$display
                WHERE gcal_id=$gcal_id
            "#,
            gcal_id = self.gcal_id,
            display = self.display,
        );
        let conn = pool.get().await?;
        query.execute(&conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn update(&self, pool: &PgPool) -> Result<(), Error> {
        let mut conn = pool.get().await?;
        let tran = conn.transaction().await?;
        let conn: &PgTransaction = &tran;
        self.update_conn(conn).await?;
        tran.commit().await?;
        Ok(())
    }

    async fn update_conn<C>(&self, conn: &C) -> Result<(), Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            r#"
                UPDATE calendar_list
                SET calendar_name=$calendar_name,
                    gcal_name=$gcal_name,
                    gcal_description=$gcal_description,
                    gcal_location=$gcal_location,
                    gcal_timezone=$gcal_timezone,
                    last_modified=now()
                WHERE gcal_id=$gcal_id
            "#,
            calendar_name = self.calendar_name,
            gcal_id = self.gcal_id,
            gcal_name = self.gcal_name,
            gcal_description = self.gcal_description,
            gcal_location = self.gcal_location,
            gcal_timezone = self.gcal_timezone,
        );
        query.execute(&conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<OffsetDateTime>, Error> {
        #[derive(FromSqlRow, Into)]
        struct Wrap(OffsetDateTime);

        let query = query!("SELECT max(last_modified) FROM calendar_list");
        let conn = pool.get().await?;
        let result: Option<Wrap> = query.fetch_opt(&conn).await?;
        Ok(result.map(Into::into))
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_recent(
        pool: &PgPool,
        modified: Option<OffsetDateTime>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query =
            Self::get_calendar_list_query("*", "ORDER BY calendar_name", &modified, offset, limit)?;

        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    async fn insert_conn<C>(&self, conn: &C) -> Result<(), Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            r#"
                INSERT INTO calendar_list (
                    calendar_name, gcal_id, gcal_name, gcal_description, gcal_location,
                    gcal_timezone, sync, last_modified, edit, display
                ) VALUES (
                    $calendar_name, $gcal_id, $gcal_name, $gcal_description, $gcal_location,
                    $gcal_timezone, $sync, now(), $edit, $display
                )
            "#,
            calendar_name = self.calendar_name,
            gcal_id = self.gcal_id,
            gcal_name = self.gcal_name,
            gcal_description = self.gcal_description,
            gcal_location = self.gcal_location,
            gcal_timezone = self.gcal_timezone,
            sync = self.sync,
            edit = self.edit,
            display = self.display,
        );
        query.execute(conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn upsert(&self, pool: &PgPool) -> Result<Option<Self>, Error> {
        let mut conn = pool.get().await?;
        let tran = conn.transaction().await?;
        let conn: &PgTransaction = &tran;
        let existing = CalendarList::get_by_gcal_id_conn(&self.gcal_id, conn).await?;
        if existing.is_some() {
            self.update_conn(conn).await?;
        } else {
            println!("insert {} {}", self.calendar_name, self.gcal_id);
            self.insert_conn(conn).await?;
        }
        tran.commit().await?;
        Ok(existing)
    }
}

#[derive(FromSqlRow, Clone, Debug, Serialize, Deserialize)]
pub struct CalendarCache {
    pub event_id: StackString,
    pub gcal_id: StackString,
    pub event_start_time: DateTimeWrapper,
    pub event_end_time: DateTimeWrapper,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
    pub last_modified: DateTimeWrapper,
}

impl CalendarCache {
    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_gcal_id(
        gcal_id: &str,
        pool: &PgPool,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query = query!(
            "SELECT * FROM calendar_cache WHERE gcal_id=$gcal_id",
            gcal_id = gcal_id
        );
        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_gcal_id_event_id(
        gcal_id: impl AsRef<str>,
        event_id: impl AsRef<str>,
        pool: &PgPool,
    ) -> Result<Option<CalendarCache>, Error> {
        let conn = pool.get().await?;
        Self::get_by_gcal_id_event_id_conn(gcal_id, event_id, &conn)
            .await
            .map_err(Into::into)
    }

    async fn get_by_gcal_id_event_id_conn<C>(
        gcal_id: impl AsRef<str>,
        event_id: impl AsRef<str>,
        conn: &C,
    ) -> Result<Option<CalendarCache>, Error>
    where
        C: GenericClient + Sync,
    {
        let gcal_id = gcal_id.as_ref();
        let event_id = event_id.as_ref();
        let query = query!(
            r#"
                SELECT * FROM calendar_cache
                WHERE gcal_id=$gcal_id AND event_id=$event_id
            "#,
            gcal_id = gcal_id,
            event_id = event_id,
        );
        query.fetch_opt(conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_datetime(
        min_time: OffsetDateTime,
        max_time: OffsetDateTime,
        pool: &PgPool,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query = query!(
            r#"
                SELECT * FROM calendar_cache
                WHERE event_end_time >= $min_time
                  AND event_start_time <= $max_time
                ORDER BY event_start_time
            "#,
            min_time = min_time,
            max_time = max_time,
        );
        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_gcal_id_datetime(
        gcal_id: &str,
        min_time: Option<OffsetDateTime>,
        max_time: Option<OffsetDateTime>,
        pool: &PgPool,
    ) -> Result<impl Stream<Item = Result<CalendarCache, PqError>>, Error> {
        let mut conditions = vec!["gcal_id = $gcal_id"];
        let mut bindings = vec![("gcal_id", &gcal_id as Parameter)];

        if let Some(max_time) = &max_time {
            conditions.push("event_start_time <= $max_time");
            bindings.push(("max_time", max_time as Parameter));
        }
        if let Some(min_time) = &min_time {
            conditions.push("event_end_time >= $min_time");
            bindings.push(("min_time", min_time as Parameter));
        }
        let query = format_sstr!(
            "SELECT * FROM calendar_cache WHERE {}",
            conditions.join(" AND "),
        );
        let query = query_dyn!(&query, ..bindings)?;
        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn update(&self, pool: &PgPool) -> Result<(), Error> {
        let conn = pool.get().await?;
        self.update_conn(&conn).await?;
        Ok(())
    }

    async fn update_conn<C>(&self, conn: &C) -> Result<(), Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            r#"
                UPDATE calendar_cache
                SET event_start_time=$event_start_time,
                    event_end_time=$event_end_time,
                    event_url=$event_url,
                    event_name=$event_name,
                    event_description=$event_description,
                    event_location_name=$event_location_name,
                    event_location_lat=$event_location_lat,
                    event_location_lon=$event_location_lon,
                    last_modified=now()
                WHERE gcal_id=$gcal_id AND event_id=$event_id
            "#,
            gcal_id = self.gcal_id,
            event_id = self.event_id,
            event_start_time = self.event_start_time,
            event_end_time = self.event_end_time,
            event_url = self.event_url,
            event_name = self.event_name,
            event_description = self.event_description,
            event_location_name = self.event_location_name,
            event_location_lat = self.event_location_lat,
            event_location_lon = self.event_location_lon,
        );
        query.execute(conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn delete(&self, pool: &PgPool) -> Result<(), Error> {
        let query = query!(
            "DELETE FROM calendar_cache WHERE event_id=$event_id AND gcal_id=$gcal_id",
            event_id = self.event_id,
            gcal_id = self.gcal_id,
        );
        let conn = pool.get().await?;
        query.execute(&conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<OffsetDateTime>, Error> {
        #[derive(FromSqlRow, Into)]
        struct Wrap(OffsetDateTime);
        let query = query!("SELECT max(last_modified) FROM calendar_cache");
        let conn = pool.get().await?;
        let result: Option<Wrap> = query.fetch_opt(&conn).await?;
        Ok(result.map(Into::into))
    }

    fn get_calendar_cache_query<'a>(
        select_str: &'a str,
        order_str: &'a str,
        modified: &'a Option<OffsetDateTime>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Query<'a>, PqError> {
        let mut bindings = Vec::new();
        let mut constraints = Vec::new();
        if let Some(modified) = modified {
            constraints.push(format_sstr!("last_modified > $modified"));
            bindings.push(("modified", modified as Parameter));
        }
        let where_str = if constraints.is_empty() {
            "".into()
        } else {
            format_sstr!("WHERE {}", constraints.join(" AND "))
        };

        let mut query = format_sstr!(
            r#"
                SELECT {select_str} FROM calendar_cache
                {where_str}
                {order_str}
            "#
        );
        if let Some(offset) = &offset {
            query.push_str(&format_sstr!(" OFFSET {offset}"));
        }
        if let Some(limit) = &limit {
            query.push_str(&format_sstr!(" LIMIT {limit}"));
        }
        query_dyn!(&query, ..bindings)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_recent(
        pool: &PgPool,
        modified: Option<OffsetDateTime>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query = Self::get_calendar_cache_query(
            "*",
            "ORDER BY event_start_time",
            &modified,
            offset,
            limit,
        )?;

        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_total(
        pool: &PgPool,
        modified: Option<OffsetDateTime>,
    ) -> Result<usize, Error> {
        #[derive(FromSqlRow)]
        struct Count {
            count: i64,
        }

        let query = Self::get_calendar_cache_query("count(*)", "", &modified, None, None)?;

        let conn = pool.get().await?;
        let count: Count = query.fetch_one(&conn).await?;
        Ok(count.count.try_into()?)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn insert(&self, pool: &PgPool) -> Result<(), Error> {
        let conn = pool.get().await?;
        self.insert_conn(&conn).await?;
        Ok(())
    }

    async fn insert_conn<C>(&self, conn: &C) -> Result<(), Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            r#"
                INSERT INTO calendar_cache (
                    gcal_id, event_id, event_start_time, event_end_time, event_url,
                    event_name, event_description, event_location_name,
                    event_location_lat, event_location_lon, last_modified
                ) VALUES (
                    $gcal_id, $event_id, $event_start_time, $event_end_time, $event_url,
                    $event_name, $event_description, $event_location_name,
                    $event_location_lat, $event_location_lon, now()
                )
            "#,
            gcal_id = self.gcal_id,
            event_id = self.event_id,
            event_start_time = self.event_start_time,
            event_end_time = self.event_end_time,
            event_url = self.event_url,
            event_name = self.event_name,
            event_description = self.event_description,
            event_location_name = self.event_location_name,
            event_location_lat = self.event_location_lat,
            event_location_lon = self.event_location_lon,
        );
        query.execute(conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn upsert(&self, pool: &PgPool) -> Result<(), Error> {
        let mut conn = pool.get().await?;
        let tran = conn.transaction().await?;
        let conn: &PgTransaction = &tran;
        let existing =
            CalendarCache::get_by_gcal_id_event_id_conn(&self.gcal_id, &self.event_id, conn)
                .await?;
        if existing.is_some() {
            self.update_conn(conn).await?;
        } else {
            self.insert_conn(conn).await?;
        }
        tran.commit().await?;
        Ok(())
    }
}

#[derive(FromSqlRow, Clone, Debug)]
pub struct AuthorizedUsers {
    pub email: StackString,
    pub telegram_userid: Option<i64>,
    pub telegram_chatid: Option<i64>,
    pub created_at: OffsetDateTime,
}

impl AuthorizedUsers {
    /// # Errors
    /// Returns error if db query fails
    pub async fn get_authorized_users(
        pool: &PgPool,
    ) -> Result<impl Stream<Item = Result<Self, PqError>>, Error> {
        let query = query!("SELECT * FROM authorized_users WHERE deleted_at IS NULL");
        let conn = pool.get().await?;
        query.fetch_streaming(&conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn update_authorized_users(&self, pool: &PgPool) -> Result<(), Error> {
        let query = query!(
            r#"
                UPDATE authorized_users
                SET telegram_userid=$telegram_userid,
                    telegram_chatid=$telegram_chatid
                WHERE email=$email
            "#,
            email = self.email,
            telegram_userid = self.telegram_userid,
            telegram_chatid = self.telegram_chatid,
        );
        let conn = pool.get().await?;
        query.execute(&conn).await?;
        Ok(())
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_most_recent(
        pool: &PgPool,
    ) -> Result<(Option<OffsetDateTime>, Option<OffsetDateTime>), Error> {
        #[derive(FromSqlRow)]
        struct CreatedDeleted {
            created_at: Option<OffsetDateTime>,
            deleted_at: Option<OffsetDateTime>,
        }

        let query = query!(
            "SELECT max(created_at) as created_at, max(deleted_at) as deleted_at FROM users"
        );
        let conn = pool.get().await?;
        let result: Option<CreatedDeleted> = query.fetch_opt(&conn).await?;
        match result {
            Some(result) => Ok((result.created_at, result.deleted_at)),
            None => Ok((None, None)),
        }
    }
}

#[derive(FromSqlRow, Clone, Debug)]
pub struct ShortenedLinks {
    pub shortened_url: StackString,
    pub original_url: StackString,
    pub last_modified: DateTimeWrapper,
}

impl ShortenedLinks {
    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_original_url(
        original_url: &str,
        pool: &PgPool,
    ) -> Result<Option<Self>, Error> {
        let conn = pool.get().await?;
        Self::get_latest_original_url_conn(original_url, &conn)
            .await
            .map_err(Into::into)
    }

    async fn get_latest_original_url_conn<C>(
        original_url: &str,
        conn: &C,
    ) -> Result<Option<Self>, Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            "SELECT * FROM shortened_links WHERE original_url=$original_url ORDER BY \
             last_modified DESC LIMIT 1",
            original_url = original_url,
        );
        query.fetch_opt(conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_by_shortened_url(
        shortened_url: &str,
        pool: &PgPool,
    ) -> Result<Option<Self>, Error> {
        let conn = pool.get().await?;
        Self::get_by_shortened_url_conn(shortened_url, &conn)
            .await
            .map_err(Into::into)
    }

    async fn get_by_shortened_url_conn<C>(
        shortened_url: &str,
        conn: &C,
    ) -> Result<Option<Self>, Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            "SELECT * FROM shortened_links WHERE shortened_url=$shortened_url",
            shortened_url = shortened_url,
        );
        query.fetch_opt(conn).await.map_err(Into::into)
    }

    /// # Errors
    /// Returns error if db query fails
    pub async fn get_or_create(original_url: &str, pool: &PgPool) -> Result<Self, Error> {
        let mut conn = pool.get().await?;
        let tran = conn.transaction().await?;
        let conn: &PgTransaction = &tran;

        if let Some(existing) = Self::get_latest_original_url_conn(original_url, conn).await? {
            Ok(existing)
        } else {
            let base_hasher = blake3::Hasher::new();
            let output = hash_reader(&base_hasher, original_url.as_bytes())?;
            let len = blake3::OUT_LEN as u64;
            let output = write_hex_output(output, len);

            let mut short_chars = 4;

            while short_chars < output.len() {
                let shortened =
                    ShortenedLinks::get_by_shortened_url_conn(&output[..short_chars], conn).await?;
                if shortened.is_none() {
                    break;
                }
                short_chars += 1;
            }

            let shortened_url = &output[..short_chars];

            let output = Self {
                original_url: original_url.into(),
                shortened_url: shortened_url.into(),
                last_modified: DateTimeWrapper::now(),
            };
            output.insert_shortened_link_conn(conn).await?;

            let output = ShortenedLinks::get_by_shortened_url_conn(shortened_url, conn)
                .await?
                .ok_or_else(|| format_err!("Something went wrong"))?;
            tran.commit().await?;
            Ok(output)
        }
    }

    async fn insert_shortened_link_conn<C>(&self, conn: &C) -> Result<(), Error>
    where
        C: GenericClient + Sync,
    {
        let query = query!(
            r#"
                INSERT INTO shortened_links (
                    original_url, shortened_url, last_modified
                ) VALUES (
                    $original_url, $shortened_url, now()
                )
            "#,
            original_url = self.original_url,
            shortened_url = self.shortened_url,
        );
        query.execute(conn).await?;
        Ok(())
    }
}

fn write_hex_output(mut output: blake3::OutputReader, mut len: u64) -> StackString {
    // Encoding multiples of the block size is most efficient.
    let mut block = [0; blake3::guts::BLOCK_LEN];
    let mut result: Vec<StackString> = Vec::new();
    while len > 0 {
        output.fill(&mut block);
        let hex_str = hex::encode(&block[..]);
        let take_bytes = cmp::min(len, block.len() as u64);
        let hex_str = &hex_str[..2 * take_bytes as usize];
        result.push(hex_str.into());
        len -= take_bytes;
    }
    result.join("").into()
}

fn copy_wide(mut reader: impl io::Read, hasher: &mut blake3::Hasher) -> Result<u64, Error> {
    let mut buffer = [0; 65536];
    let mut total = 0;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => {
                hasher.update(&buffer[..n]);
                total += n as u64;
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
}

fn hash_reader(
    base_hasher: &blake3::Hasher,
    reader: impl io::Read,
) -> Result<blake3::OutputReader, Error> {
    let mut hasher = base_hasher.clone();
    copy_wide(reader, &mut hasher)?;
    Ok(hasher.finalize_xof())
}
