use anyhow::{format_err, Error};
use chrono::{DateTime, Utc};
use diesel::{dsl::max, ExpressionMethods, QueryDsl};
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use std::{cmp, io};
use tokio_diesel::{AsyncRunQueryDsl, OptionalExtension};

use crate::{
    pgpool::PgPool,
    schema::{authorized_users, calendar_cache, calendar_list, shortened_links},
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
    pub async fn get_calendars(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::calendar_list;
        calendar_list.load_async(pool).await.map_err(Into::into)
    }

    pub async fn get_by_id(id_: i32, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, id};
        calendar_list
            .filter(id.eq(id_))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id(gcal_id_: &str, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, gcal_id};
        calendar_list
            .filter(gcal_id.eq(gcal_id_))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn update_display(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, display, id};
        diesel::update(calendar_list.filter(id.eq(&self.id)))
            .set(display.eq(&self.display))
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
    }

    pub async fn update(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_list::dsl::{
            calendar_list, gcal_description, gcal_id, gcal_location, gcal_name, gcal_timezone, id,
            last_modified,
        };
        diesel::update(calendar_list.filter(id.eq(&self.id)))
            .set((
                gcal_id.eq(&self.gcal_id),
                gcal_name.eq(&self.gcal_name),
                gcal_description.eq(&self.gcal_description),
                gcal_location.eq(&self.gcal_location),
                gcal_timezone.eq(&self.gcal_timezone),
                last_modified.eq(Utc::now()),
            ))
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
    }

    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, last_modified};
        calendar_list
            .select(max(last_modified))
            .first_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_recent(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_list::dsl::{calendar_list, last_modified};
        calendar_list
            .filter(last_modified.gt(modified))
            .load_async(pool)
            .await
            .map_err(Into::into)
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
            last_modified: Utc::now().into(),
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
            last_modified: Utc::now().into(),
            edit: false,
            display: false,
        }
    }

    pub async fn insert(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_list::dsl::calendar_list;
        diesel::insert_into(calendar_list)
            .values(&self)
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
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
            _ => Err(format_err!(
                "this shouldn't be possible... {} must be unique",
                self.gcal_id
            )),
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
    pub async fn get_all_events(pool: &PgPool) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::calendar_cache;
        calendar_cache.load_async(pool).await.map_err(Into::into)
    }

    pub async fn get_by_gcal_id(
        gcal_id_: &str,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, gcal_id};
        calendar_cache
            .filter(gcal_id.eq(gcal_id_))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id_event_id(
        gcal_id_: &str,
        event_id_: &str,
        pool: &PgPool,
    ) -> Result<Option<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, event_id, gcal_id};
        calendar_cache
            .filter(gcal_id.eq(gcal_id_))
            .filter(event_id.eq(event_id_))
            .get_result_async(pool)
            .await
            .optional()
            .map_err(Into::into)
    }

    pub async fn get_by_datetime(
        min_time: DateTime<Utc>,
        max_time: DateTime<Utc>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_end_time, event_start_time,
        };
        calendar_cache
            .filter(event_end_time.gt(min_time))
            .filter(event_start_time.lt(max_time))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_gcal_id_datetime(
        gcal_id_: &str,
        min_time: Option<DateTime<Utc>>,
        max_time: Option<DateTime<Utc>>,
        pool: &PgPool,
    ) -> Result<Vec<CalendarCache>, Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_end_time, event_start_time, gcal_id,
        };
        if let Some(min_time) = min_time {
            if let Some(max_time) = max_time {
                calendar_cache
                    .filter(gcal_id.eq(gcal_id_))
                    .filter(event_end_time.gt(min_time))
                    .filter(event_start_time.lt(max_time))
                    .load_async(pool)
                    .await
            } else {
                calendar_cache
                    .filter(gcal_id.eq(gcal_id_))
                    .filter(event_end_time.gt(min_time))
                    .load_async(pool)
                    .await
            }
        } else {
            calendar_cache
                .filter(gcal_id.eq(gcal_id_))
                .load_async(pool)
                .await
        }
        .map_err(Into::into)
    }

    pub async fn update(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_description, event_end_time, event_id, event_location_lat,
            event_location_lon, event_location_name, event_name, event_start_time, event_url,
            gcal_id, last_modified,
        };
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
        .execute_async(pool)
        .await
        .map(|_| self)
        .map_err(Into::into)
    }

    pub async fn delete(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, id};
        diesel::delete(calendar_cache.filter(id.eq(&self.id)))
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
    }

    pub async fn get_max_modified(pool: &PgPool) -> Result<Option<DateTime<Utc>>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, last_modified};
        calendar_cache
            .select(max(last_modified))
            .first_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_recent(modified: DateTime<Utc>, pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, last_modified};
        calendar_cache
            .filter(last_modified.gt(modified))
            .load_async(pool)
            .await
            .map_err(Into::into)
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
            last_modified: Utc::now().into(),
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
            last_modified: Utc::now().into(),
        }
    }

    pub async fn insert(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::calendar_cache::dsl::calendar_cache;
        diesel::insert_into(calendar_cache)
            .values(&self)
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
    }

    pub async fn upsert(self, pool: &PgPool) -> Result<Self, Error> {
        let existing =
            CalendarCache::get_by_gcal_id_event_id(&self.gcal_id, &self.event_id, &pool).await?;
        match existing {
            None => self.insert(&pool).await,
            Some(event) => {
                let id = event.id;
                self.into_calendar_cache(id)
                    .update(&pool)
                    .await
                    .map(Into::into)
            }
        }
    }
}

#[derive(Queryable, Insertable, Clone, Debug)]
#[table_name = "authorized_users"]
pub struct AuthorizedUsers {
    pub email: StackString,
    pub telegram_userid: Option<i64>,
    pub telegram_chatid: Option<i64>,
}

impl AuthorizedUsers {
    pub async fn get_authorized_users(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::authorized_users::dsl::authorized_users;
        authorized_users.load_async(pool).await.map_err(Into::into)
    }

    pub async fn update_authorized_users(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::authorized_users::dsl::{
            authorized_users, email, telegram_chatid, telegram_userid,
        };
        diesel::update(authorized_users.filter(email.eq(&self.email)))
            .set((
                telegram_userid.eq(self.telegram_userid),
                telegram_chatid.eq(self.telegram_chatid),
            ))
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
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
    pub async fn get_by_original_url(
        original_url_: &str,
        pool: &PgPool,
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::{original_url, shortened_links};
        shortened_links
            .filter(original_url.eq(original_url_))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_shortened_url(
        shortened_url_: &str,
        pool: &PgPool,
    ) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::{shortened_links, shortened_url};
        shortened_links
            .filter(shortened_url.eq(shortened_url_))
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn get_shortened_links(pool: &PgPool) -> Result<Vec<Self>, Error> {
        use crate::schema::shortened_links::dsl::shortened_links;
        shortened_links.load_async(pool).await.map_err(Into::into)
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
            let output = write_hex_output(output, len);

            let mut short_chars = 4;

            while short_chars < output.len() {
                let shortened =
                    ShortenedLinks::get_by_shortened_url(&output[..short_chars], pool).await?;
                if shortened.is_empty() {
                    break;
                }
                short_chars += 1;
            }

            let shortened_url = &output[..short_chars];

            Ok(Self {
                original_url: original_url.into(),
                shortened_url: shortened_url.into(),
                last_modified: Utc::now(),
            })
        }
    }

    pub async fn insert_shortened_link(self, pool: &PgPool) -> Result<Self, Error> {
        use crate::schema::shortened_links::dsl::shortened_links;
        diesel::insert_into(shortened_links)
            .values(&self)
            .execute_async(pool)
            .await
            .map(|_| self)
            .map_err(Into::into)
    }
}

fn write_hex_output(mut output: blake3::OutputReader, mut len: u64) -> StackString {
    // Encoding multiples of the block size is most efficient.
    let mut block = [0; blake3::guts::BLOCK_LEN];
    let mut result = Vec::new();
    while len > 0 {
        output.fill(&mut block);
        let hex_str = hex::encode(&block[..]);
        let take_bytes = cmp::min(len, block.len() as u64);
        let hex_str = &hex_str[..2 * take_bytes as usize];
        result.push(hex_str.to_string());
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
