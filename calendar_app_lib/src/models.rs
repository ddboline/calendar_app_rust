use anyhow::{format_err, Error};
use chrono::{DateTime, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use tokio::task::spawn_blocking;

use crate::pgpool::PgPool;
use crate::schema::calendar_cache;

#[derive(Queryable, Clone, Debug)]
pub struct CalendarCache {
    pub id: i32,
    pub calendar_name: String,
    pub gcal_id: String,
    pub event_id: String,
    pub event_start_time: DateTime<Utc>,
    pub event_end_time: DateTime<Utc>,
    pub event_url: Option<String>,
    pub event_name: String,
    pub event_description: Option<String>,
    pub event_location_name: Option<String>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Calendar {
    pub gcal_id: String,
    pub calendar_name: String,
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

    fn update_sync(&self, pool: &PgPool) -> Result<(), Error> {
        use crate::schema::calendar_cache::dsl::{
            calendar_cache, event_description, event_end_time, event_id, event_location_lat,
            event_location_lon, event_location_name, event_name, event_start_time, event_url,
            gcal_id,
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
        ))
        .execute(&conn)
        .map(|_| ())
        .map_err(Into::into)
    }

    pub async fn update(self, pool: &PgPool) -> Result<Self, Error> {
        let pool = pool.clone();
        spawn_blocking(move || self.update_sync(&pool).map(|_| self)).await?
    }

    fn get_calendars_sync(pool: &PgPool) -> Result<Vec<(String, String)>, Error> {
        use crate::schema::calendar_cache::dsl::{calendar_cache, calendar_name, gcal_id};
        let conn = pool.get()?;
        calendar_cache
            .select((gcal_id, calendar_name))
            .distinct()
            .load(&conn)
            .map_err(Into::into)
    }

    pub async fn get_calendars(pool: &PgPool) -> Result<Vec<Calendar>, Error> {
        let pool = pool.clone();
        spawn_blocking(move || Self::get_calendars_sync(&pool))
            .await?
            .map(|x| {
                x.into_iter()
                    .map(|(gcal_id, calendar_name)| Calendar {
                        gcal_id,
                        calendar_name,
                    })
                    .collect()
            })
    }
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "calendar_cache"]
pub struct InsertCalendarCache {
    pub calendar_name: String,
    pub gcal_id: String,
    pub event_id: String,
    pub event_start_time: DateTime<Utc>,
    pub event_end_time: DateTime<Utc>,
    pub event_url: Option<String>,
    pub event_name: String,
    pub event_description: Option<String>,
    pub event_location_name: Option<String>,
    pub event_location_lat: Option<f64>,
    pub event_location_lon: Option<f64>,
}

impl From<CalendarCache> for InsertCalendarCache {
    fn from(item: CalendarCache) -> Self {
        Self {
            calendar_name: item.calendar_name,
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
        }
    }
}

impl InsertCalendarCache {
    pub fn into_calendar_cache(self, id: i32) -> CalendarCache {
        CalendarCache {
            id,
            calendar_name: self.calendar_name,
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
        if existing.len() > 1 {
            Err(format_err!(
                "gcal_id {}, event_id {} is not unique",
                self.gcal_id,
                self.event_id
            ))
        } else if existing.len() == 1 {
            let id = existing[0].id;
            self.into_calendar_cache(id)
                .update(&pool)
                .await
                .map(Into::into)
        } else {
            let insertable: InsertCalendarCache = self.into();
            insertable.insert(&pool).await
        }
    }
}
