use anyhow::{format_err, Error};
use chrono::{DateTime, Utc};
use diesel::{dsl::max, ExpressionMethods, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};
use std::{cmp, io};
use tokio::task::spawn_blocking;

use crate::{
    pgpool::PgPool,
    schema::{authorized_users, calendar_cache, calendar_list, shortened_links},
    stack_string::StackString,
};

#[derive(Queryable, Clone, Debug, Serialize, Deserialize)]
pub struct CalendarList {
    pub id: i32,
    pub calendar_name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub gcal_description: Option<StackString>,
    pub gcal_location: Option<StackString>,
    pub gcal_timezone: Option<StackString>,
    pub sync: bool,
    pub last_modified: DateTime<Utc>,
    pub edit: bool,
    pub display: bool,
}

impl CalendarList {
    fn get_calendars_sync(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::calendar_list;
        let conn = pool.get()?;
        calendar_list.load(&conn).map_err(Into::into)
    }

    pub async fn get_calendars(pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_calendars_sync(&pool)).await?
    }

    fn get_by_id_sync(id_: i32, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, id};
        let conn = pool.get()?;
        calendar_list
            .filter(id.eq(id_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_id(id: i32, pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_by_id_sync(id, &pool)).await?
    }

    fn get_by_gcal_id_sync(gcal_id_: &str, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, gcal_id};
        let conn = pool.get()?;
        calendar_list
            .filter(gcal_id.eq(gcal_id_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id(gcal_id: &str, pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        let gcal_id = gcal_id.to_string();
        spawn_blocking(move || Self::get_by_gcal_id_sync(&gcal_id, &pool)).await?
    }

    fn update_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_list::dsl::{
            calendar_list, display, edit, gcal_description, gcal_id, gcal_location, gcal_name,
            gcal_timezone, id, last_modified, sync,
        };
        let conn = pool.get()?;
        diesel::update(calendar_list.filter(id.eq(&self.id)))
            .set((
                gcal_id.eq(&self.gcal_id),
                gcal_name.eq(&self.gcal_name),
                gcal_description.eq(&self.gcal_description),
                gcal_location.eq(&self.gcal_location),
                gcal_timezone.eq(&self.gcal_timezone),
                display.eq(&self.display),
                sync.eq(&self.sync),
                edit.eq(&self.edit),
                last_modified.eq(Utc::now()),
            ))
            .execute(&conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn update(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.update_sync(&pool).map(|_| self)).await?
    }

    fn get_max_modified_sync(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, last_modified};
        let conn = pool.get()?;
        calendar_list
            .select(max(last_modified))
            .first(&conn)
            .map_err(Into::into)
    }

    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_max_modified_sync(&pool)).await?
    }

    fn get_recent_sync(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, last_modified};
        let conn = pool.get()?;
        calendar_list
            .filter(last_modified.gt(modified))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_recent(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_recent_sync(modified, &pool)).await?
    }
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[table_name = "calendar_list"]
pub struct InsertCalendarList {
    pub calendar_name: StackString,
    pub gcal_id: StackString,
    pub gcal_name: Option<StackString>,
    pub gcal_description: Option<StackString>,
    pub gcal_location: Option<StackString>,
    pub gcal_timezone: Option<StackString>,
    pub sync: bool,
    pub last_modified: DateTime<Utc>,
    pub edit: bool,
}

impl From<CalendarList> for InsertCalendarList {
    fn from(item: CalendarList) -> Self {
        Self {
            calendar_name: item.calendar_name,
            gcal_id: item.gcal_id,
            gcal_name: item.gcal_name,
            gcal_description: item.gcal_description,
            gcal_location: item.gcal_location,
            gcal_timezone: item.gcal_timezone,
            sync: false,
            last_modified: Utc::now(),
            edit: false,
        }
    }
}

impl InsertCalendarList {
    pub fn into_calendar_list(self, id: i32) -> CalendarList {
        CalendarList {
            id,
            calendar_name: self.calendar_name,
            gcal_id: self.gcal_id,
            gcal_name: self.gcal_name,
            gcal_description: self.gcal_description,
            gcal_location: self.gcal_location,
            gcal_timezone: self.gcal_timezone,
            sync: false,
            last_modified: Utc::now(),
            edit: false,
            display: false,
        }
    }

    fn insert_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_list::dsl::calendar_list;
        let conn = pool.get()?;
        diesel::insert_into(calendar_list)
            .values(self)
            .execute(&conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn insert(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.insert_sync(&pool).map(|_| self)).await?
    }

    pub async fn upsert(self, pool: &PgPool) -> Result<Self, Error> {
        let existing = CalendarList::get_by_gcal_id(&self.gcal_id, &pool).await?;
        match existing.len() {
            0 => self.insert(&pool).await,
            1 => {
                let id = existing[0].id;
                self.into_calendar_list(id)
                    .update(&pool)
                    .await
                    .map(Into::into)
            }
            _ => {
                panic!(
                    "this shouldn't be possible... {} must be unique",
                    self.gcal_id
                );
            }
        }
    }
}

#[derive(Queryable, Clone, Debug, Serialize, Deserialize)]
pub struct CalendarCache {
    pub id: i32,
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_time: DateTime<Utc>,
    pub event_end_time: DateTime<Utc>,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
    pub last_modified: DateTime<Utc>,
}

impl CalendarCache {
    fn get_all_events_sync(pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::calendar_cache;
        let conn = pool.get()?;
        calendar_cache.load(&conn).map_err(Into::into)
    }

    pub async fn get_all_events(pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_all_events_sync(&pool)).await?
    }

    fn get_by_gcal_id_sync(gcal_id_: &str, pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, gcal_id};
        let conn = pool.get()?;
        calendar_cache
            .filter(gcal_id.eq(gcal_id_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id(gcal_id: &str, pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
        let pool = pool.clone();
        let gcal_id = gcal_id.to_owned();
        spawn_blocking(move || Self::get_by_gcal_id_sync(&gcal_id, &pool)).await?
    }

    fn get_by_gcal_id_event_id_sync(
        gcal_id_: &str,
        event_id_: &str,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, event_id, gcal_id};
        let conn = pool.get()?;
        calendar_cache
            .filter(gcal_id.eq(gcal_id_))
            .filter(event_id.eq(event_id_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id_event_id(
        gcal_id: &str,
        event_id: &str,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        let pool = pool.clone();
        let gcal_id = gcal_id.to_owned();
        let event_id = event_id.to_owned();
        spawn_blocking(move || Self::get_by_gcal_id_event_id_sync(&gcal_id, &event_id, &pool))
            .await?
    }

    fn get_by_datetime_sync(
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_end_time, event_start_time,
        };
        let conn = pool.get()?;
        calendar_cache
            .filter(event_end_time.gt(min_time))
            .filter(event_start_time.lt(max_time))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_datetime(
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_by_datetime_sync(min_time, max_time, &pool)).await?
    }

    fn get_by_gcal_id_datetime_sync(
        gcal_id_: &str,
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_end_time, event_start_time, gcal_id,
        };
        let conn = pool.get()?;
        calendar_cache
            .filter(gcal_id.eq(gcal_id_))
            .filter(event_end_time.gt(min_time))
            .filter(event_start_time.lt(max_time))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id_datetime(
        gcal_id: &str,
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        let pool = pool.clone();
        let gcal_id = gcal_id.to_string();
        spawn_blocking(move || {
            Self::get_by_gcal_id_datetime_sync(&gcal_id, min_time, max_time, &pool)
        })
        .await?
    }

    fn update_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_description, event_end_time, event_id, event_location_lat,
            event_location_lon, event_location_name, event_name, event_start_time, event_url,
            gcal_id, last_modified,
        };
        let conn = pool.get()?;
        diesel::update(
            calendar_cache
                .filter(gcal_id.eq(&self.gcal_id))
                .filter(event_id.eq(&self.event_id)),
        )
        .set((
            event_start_time.eq(&self.event_start_time),
            event_end_time.eq(&self.event_end_time),
            event_url.eq(&self.event_url),
            event_name.eq(&self.event_name),
            event_description.eq(&self.event_description),
            event_location_name.eq(&self.event_location_name),
            event_location_lat.eq(&self.event_location_lat),
            event_location_lon.eq(&self.event_location_lon),
            last_modified.eq(Utc::now()),
        ))
        .execute(&conn)
        .map(|_| ())
        .map_err(Into::into)
    }

    pub async fn update(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.update_sync(&pool).map(|_| self)).await?
    }

    fn delete_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, id};
        let conn = pool.get()?;
        diesel::delete(calendar_cache.filter(id.eq(&self.id)))
            .execute(&conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn delete(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.delete_sync(&pool).map(|_| self)).await?
    }

    fn get_max_modified_sync(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, last_modified};
        let conn = pool.get()?;
        calendar_cache
            .select(max(last_modified))
            .first(&conn)
            .map_err(Into::into)
    }

    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_max_modified_sync(&pool)).await?
    }

    fn get_recent_sync(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, last_modified};
        let conn = pool.get()?;
        calendar_cache
            .filter(last_modified.gt(modified))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_recent(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_recent_sync(modified, &pool)).await?
    }
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[table_name = "calendar_cache"]
pub struct InsertCalendarCache {
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_time: DateTime<Utc>,
    pub event_end_time: DateTime<Utc>,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
    pub last_modified: DateTime<Utc>,
}

impl From<CalendarCache> for InsertCalendarCache {
    fn from(item: CalendarCache) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time,
            event_end_time: item.event_end_time,
            event_url: item.event_url,
            event_name: item.event_name,
            event_description: item.event_description,
            event_location_name: item.event_location_name,
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: Utc::now(),
        }
    }
}

impl InsertCalendarCache {
    pub fn into_calendar_cache(self, id: i32) -> CalendarCache {
        CalendarCache {
            id,
            gcal_id: self.gcal_id,
            event_id: self.event_id,
            event_start_time: self.event_start_time,
            event_end_time: self.event_end_time,
            event_url: self.event_url,
            event_name: self.event_name,
            event_description: self.event_description,
            event_location_name: self.event_location_name,
            event_location_lat: self.event_location_lat,
            event_location_lon: self.event_location_lon,
            last_modified: Utc::now(),
        }
    }

    fn insert_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_cache::dsl::calendar_cache;
        let conn = pool.get()?;
        diesel::insert_into(calendar_cache)
            .values(self)
            .execute(&conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn insert(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.insert_sync(&pool).map(|_| self)).await?
    }

    pub async fn upsert(self, pool: &PgPool) -> Result<Self, Error> {
        let existing =
            CalendarCache::get_by_gcal_id_event_id(&self.gcal_id, &self.event_id, &pool).await?;
        match existing.len() {
            0 => self.insert(&pool).await,
            1 => {
                let id = existing[0].id;
                self.into_calendar_cache(id)
                    .update(&pool)
                    .await
                    .map(Into::into)
            }
            _ => Err(format_err!(
                "gcal_id {}, event_id {} is not unique",
                self.gcal_id,
                self.event_id
            )),
        }
    }
}

#[derive(Queryable, Insertable, Clone, Debug)]
#[table_name = "authorized_users"]
pub struct AuthorizedUsers {
    pub email: StackString,
}

impl AuthorizedUsers {
    fn get_authorized_users_sync(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::authorized_users::dsl::authorized_users;
        let conn = pool.get()?;
        authorized_users.load(&conn).map_err(Into::into)
    }

    pub async fn get_authorized_users(pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_authorized_users_sync(&pool)).await?
    }
}

#[derive(Queryable, Clone, Debug)]
pub struct ShortenedLinks {
    pub id: i32,
    pub original_url: StackString,
    pub shortened_url: StackString,
    pub last_modified: DateTime<Utc>,
}

impl ShortenedLinks {
    fn get_by_original_url_sync(original_url_: &str, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::{original_url, shortened_links};
        let conn = pool.get()?;
        shortened_links
            .filter(original_url.eq(original_url_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_original_url(
        original_url: &str,
        pool: &PgPool,
    ) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        let original_url = original_url.to_string();
        spawn_blocking(move || Self::get_by_original_url_sync(&original_url, &pool)).await?
    }

    fn get_by_shortened_url_sync(shortened_url_: &str, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::{shortened_links, shortened_url};
        let conn = pool.get()?;
        shortened_links
            .filter(shortened_url.eq(shortened_url_))
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_by_shortened_url(
        shortened_url: &str,
        pool: &PgPool,
    ) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        let shortened_url = shortened_url.to_string();
        spawn_blocking(move || Self::get_by_shortened_url_sync(&shortened_url, &pool)).await?
    }

    fn get_shortened_links_sync(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::shortened_links;
        let conn = pool.get()?;
        shortened_links.load(&conn).map_err(Into::into)
    }

    pub async fn get_shortened_links(pool: &PgPool) -> Result<Vec<Self>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_shortened_links_sync(&pool)).await?
    }
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[table_name = "shortened_links"]
pub struct InsertShortenedLinks {
    pub original_url: StackString,
    pub shortened_url: StackString,
    pub last_modified: DateTime<Utc>,
}

impl From<ShortenedLinks> for InsertShortenedLinks {
    fn from(item: ShortenedLinks) -> Self {
        Self {
            original_url: item.original_url,
            shortened_url: item.shortened_url,
            last_modified: Utc::now(),
        }
    }
}

impl InsertShortenedLinks {
    pub async fn get_or_create(original_url: &str, pool: &PgPool) -> Result<Self, Error> {
        let existing = ShortenedLinks::get_by_original_url(original_url, pool)
            .await?
            .pop();
        if let Some(existing) = existing {
            Ok(existing.into())
        } else {
            let base_hasher = blake3::Hasher::new();
            let output = hash_reader(&base_hasher, original_url.as_bytes())?;
            let len = blake3::OUT_LEN as u64;
            let output = write_hex_output(output, len)?;

            let mut short_chars = 4;

            while short_chars < output.len() {
                let shortened =
                    ShortenedLinks::get_by_shortened_url(&output[..short_chars], pool).await?;
                if shortened.is_empty() {
                    break;
                } else {
                    short_chars += 1;
                }
            }

            let shortened_url = &output[..short_chars];

            Ok(Self {
                original_url: original_url.into(),
                shortened_url: shortened_url.into(),
                last_modified: Utc::now(),
            })
        }
    }

    fn insert_shortened_link_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::shortened_links::dsl::shortened_links;
        let conn = pool.get()?;
        diesel::insert_into(shortened_links)
            .values(self)
            .execute(&conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn insert_shortened_link(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.insert_shortened_link_sync(&pool).map(|_| self)).await?
    }
}

fn write_hex_output(mut output: blake3::OutputReader, mut len: u64) -> Result<String, Error> {
    // Encoding multiples of the block size is most efficient.
    let mut block = [0; blake3::BLOCK_LEN];
    let mut result = Vec::new();
    while len > 0 {
        output.fill(&mut block);
        let hex_str = hex::encode(&block[..]);
        let take_bytes = cmp::min(len, block.len() as u64);
        let hex_str = &hex_str[..2 * take_bytes as usize];
        result.push(hex_str.to_string());
        len -= take_bytes;
    }
    Ok(result.join(""))
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
