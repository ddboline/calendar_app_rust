use anyhow::format_err;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use futures::future::try_join_all;
use itertools::Itertools;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use stack_string::StackString;
use std::collections::HashMap;
use tokio::sync::RwLock;
use url::Url;
use warp::{Rejection, Reply};

use calendar_app_lib::{
    calendar::Event,
    calendar_sync::CalendarSync,
    models::{
        CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList, ShortenedLinks,
    },
};

use crate::{
    app::AppState,
    errors::{ServiceError as Error, ServiceError},
    logged_user::LoggedUser,
};

pub type WarpResult<T> = Result<T, Rejection>;
pub type HttpResult<T> = Result<T, Error>;
type UrlCache = RwLock<HashMap<StackString, StackString>>;

lazy_static! {
    static ref SHORTENED_URLS: UrlCache = RwLock::new(HashMap::new());
}

pub async fn calendar_index(_: LoggedUser) -> WarpResult<impl Reply> {
    let body = include_str!("../../templates/index.html").replace("DISPLAY_TEXT", "");
    Ok(warp::reply::html(body))
}

pub async fn agenda(_: LoggedUser, data: AppState) -> WarpResult<impl Reply> {
    let body = tokio::task::spawn(async move { agenda_body(data.cal_sync).await })
        .await
        .unwrap()?;
    Ok(warp::reply::html(body))
}

async fn agenda_body(cal_sync: CalendarSync) -> HttpResult<String> {
    let calendar_map: HashMap<_, _> = cal_sync
        .list_calendars()
        .await?
        .filter_map(|cal| {
            if cal.display {
                Some((cal.gcal_id.clone(), cal))
            } else {
                None
            }
        })
        .collect();
    let events = cal_sync
        .list_agenda(1, 2)
        .await?
        .sorted_by_key(|event| event.start_time)
        .filter_map(|event| {
            let cal = match calendar_map.get(&event.gcal_id) {
                Some(cal) => cal,
                None => return None,
            };
            let calendar_name = cal.gcal_name.as_ref().unwrap_or(&cal.name);
            let delete = if cal.edit {
                format!(
                    r#"<input type="button" name="delete_event" value="Delete" onclick="deleteEventAgenda('{gcal_id}', '{event_id}')">"#,
                    gcal_id=event.gcal_id,
                    event_id=event.event_id,
                )
            } else {
                "".to_string()
            };
            let start_time = match cal_sync.config.default_time_zone {
                Some(tz) => {
                    let tz: Tz = tz.into();
                    event.start_time.with_timezone(&tz).to_string()
                },
                None => event.start_time.with_timezone(&Local).to_string(),
            };
            Some(format!(
                r#"
                    <tr text-style="center">
                    <td><input type="button" name="list_events" value="{calendar_name}" onclick="listEvents('{cal_name}')"></td>
                    <td><input type="button" name="event_detail" value="{event_name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start_time}</td>
                    <td>{delete}</td>
                    </tr>
                "#,
                calendar_name=calendar_name,
                cal_name=cal.name,
                gcal_id=event.gcal_id,
                event_id=event.event_id,
                event_name=event.name,
                start_time=start_time,
                delete=delete,
            ))
        })
        .join("");
    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead><th>Calendar</th><th>Event</th><th>Start Time</th></thead>
        <tbody>{}</tbody>
        </table>"#,
        events
    );
    Ok(body)
}

pub async fn sync_calendars(_: LoggedUser, data: AppState) -> WarpResult<impl Reply> {
    let body = sync_calendars_body(&data.cal_sync, false).await?;
    Ok(warp::reply::html(body))
}

async fn sync_calendars_body(cal_sync: &CalendarSync, do_full: bool) -> HttpResult<String> {
    Ok(cal_sync.run_syncing(do_full).await?.join("<br>"))
}

pub async fn sync_calendars_full(_: LoggedUser, data: AppState) -> WarpResult<impl Reply> {
    let body = sync_calendars_body(&data.cal_sync, true).await?;
    Ok(warp::reply::html(body))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteEventPath {
    pub gcal_id: StackString,
    pub event_id: StackString,
}

pub async fn delete_event(
    payload: DeleteEventPath,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let body = delete_event_body(payload, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn delete_event_body(
    payload: DeleteEventPath,
    cal_sync: &CalendarSync,
) -> HttpResult<String> {
    let body = if let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&payload.gcal_id, &payload.event_id, &cal_sync.pool)
            .await?
    {
        let body = format!("delete {} {}", &payload.gcal_id, &payload.event_id);
        event.delete(&cal_sync.pool).await?;
        cal_sync
            .gcal
            .as_ref()
            .ok_or_else(|| format_err!("No gcal instance found"))?
            .delete_gcal_event(&payload.gcal_id, &payload.event_id)
            .await?;
        body
    } else {
        "Event not deleted".to_string()
    };
    Ok(body)
}

pub async fn list_calendars(_: LoggedUser, data: AppState) -> WarpResult<impl Reply> {
    let body = list_calendars_body(&data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn list_calendars_body(cal_sync: &CalendarSync) -> HttpResult<String> {
    let calendars = cal_sync
        .list_calendars()
        .await?
        .filter(|calendar| calendar.sync)
        .sorted_by_key(|calendar| {
            calendar
                .gcal_name
                .as_ref()
                .map_or_else(|| calendar.name.to_string(), ToString::to_string)
        });
    let calendars = calendars
        .map(|calendar| {
            let create_event = if calendar.edit {
                format!(r#"
                    <input type="button" name="create_event" value="Create Event" onclick="buildEvent('{}')">
                "#, calendar.gcal_id)
            } else {
                "".to_string()
            };
            let make_visible = if calendar.display {
                format!(r#"
                    <input type="button" name="hide_calendar" value="Hide" onclick="calendarDisplay('{}', false)">
                "#, calendar.gcal_id)
            } else {
                format!(r#"
                <input type="button" name="show_calendar" value="Show" onclick="calendarDisplay('{}', true)">
                "#, calendar.gcal_id)
            };
            format!(r#"
                <tr text-style="center">
                <td><input type="button" name="list_events" value="{calendar_name}" onclick="listEvents('{calendar_name}')"></td>
                <td>{description}</td>
                <td>{make_visible}</td>
                <td>{create_event}</td>
                </tr>"#,
                calendar_name=calendar.name,
                description=calendar.description.as_ref().map_or_else(|| "", StackString::as_str),
                create_event=create_event,
                make_visible=make_visible,
            )
        }).join("");

    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead>
        <th>Calendar</th>
        <th>Description</th>
        <th></th>
        <th><input type="button" name="sync_all" value="Full Sync" onclick="syncCalendarsFull();"/></th>
        </thead>
        <tbody>{}</tbody>
        </table>"#,
        calendars
    );
    Ok(body)
}

#[derive(Serialize, Deserialize)]
pub struct ListEventsRequest {
    pub calendar_name: StackString,
    pub min_time: Option<NaiveDate>,
    pub max_time: Option<NaiveDate>,
}

pub async fn list_events(
    query: ListEventsRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let body = list_events_body(query, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn list_events_body(query: ListEventsRequest, cal_sync: &CalendarSync) -> HttpResult<String> {
    let calendar_map: HashMap<_, _> = cal_sync
        .list_calendars()
        .await?
        .map(|cal| (cal.name.clone(), cal))
        .collect();
    let cal = match calendar_map.get(&query.calendar_name) {
        Some(cal) => cal,
        None => return Ok("".to_string()),
    };
    let events = cal_sync.list_events(&cal.gcal_id, query.min_time, query.max_time).await?
        .sorted_by_key(|event| event.start_time)
        .map(|event| {
            let delete = if cal.edit {
                format!(
                    r#"<input type="button" name="delete_event" value="Delete" onclick="deleteEventList('{gcal_id}', '{event_id}', '{calendar_name}')">"#,
                    gcal_id=event.gcal_id,
                    event_id=event.event_id,
                    calendar_name=query.calendar_name,
                )
            } else {
                "".to_string()
            };
            let start_time = match cal_sync.config.default_time_zone {
                Some(tz) => {
                    let tz: Tz = tz.into();
                    event.start_time.with_timezone(&tz).to_string()
                },
                None => event.start_time.with_timezone(&Local).to_string(),
            };

            format!(r#"
                    <tr text-style="center">
                    <td><input type="button" name="{name}" value="{name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start}</td>
                    <td>{end}</td>
                    <td>{delete}</td>
                    </tr>
                "#,
                name=event.name,
                start=start_time,
                end=event.end_time.with_timezone(&Local),
                gcal_id=event.gcal_id,
                event_id=event.event_id,
                delete=delete
            )
        }).join("");
    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead>
        <th>Event</th><th>Start Time</th><th>End Time</th>
        <th><input type="button" name="create_event" value="Create Event" onclick="buildEvent('{}')"></th>
        </thead>
        <tbody>{}</tbody>
        </table>"#,
        cal.gcal_id, events
    );
    Ok(body)
}

pub async fn event_detail(
    payload: DeleteEventPath,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let body = event_detail_body(payload, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn event_detail_body(
    payload: DeleteEventPath,
    cal_sync: &CalendarSync,
) -> HttpResult<String> {
    let body = if let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&payload.gcal_id, &payload.event_id, &cal_sync.pool)
            .await?
    {
        let event: Event = event.into();
        let mut output = Vec::new();
        output.push(format!(
            r#"<tr text-style="center"><td>Name</td><td>{}</td></tr>"#,
            &event.name
        ));
        if let Some(description) = &event.description {
            let description = description
                .split('\n')
                .map(|line| {
                    let mut line_length = 0;
                    let words = line
                        .split_whitespace()
                        .map(|word| {
                            let mut output_word = word.to_string();
                            if let Ok(url) = word.parse::<Url>() {
                                if url.scheme() == "https" {
                                    output_word = format!(r#"<a href="{url}">Link</a>"#, url = url);
                                }
                            }
                            line_length += output_word.len();
                            if line_length > 60 {
                                output_word = format!("<br>{}", output_word);
                                line_length = 0;
                            }
                            output_word
                        })
                        .join(" ");
                    format!("\t\t{}", words)
                })
                .join("");
            output.push(format!(
                r#"<tr text-style="center"><td>Description</td><td>{}</td></tr>"#,
                &description
            ));
        }
        if let Some(url) = &event.url {
            output.push(format!(
                r#"<tr text-style="center"><td>Url</td><td><a href={url}>Link</a></td></tr>"#,
                url = url.as_str()
            ));
        }
        if let Some(location) = &event.location {
            output.push(format!(
                r#"<tr text-style="center"><td>Location</td><td>{}</td></tr>"#,
                location.name
            ));
            if let Some((lat, lon)) = &location.lat_lon {
                output.push(format!(
                    r#"<tr text-style="center"><td>Lat,Lon:</td><td>{},{}</td></tr>"#,
                    lat, lon
                ));
            }
        }
        output.push(format!(
            r#"<tr text-style="center"><td>Start Time</td><td>{}</td></tr>"#,
            event.start_time.with_timezone(&Local)
        ));
        output.push(format!(
            r#"<tr text-style="center"><td>End Time</td><td>{}</td></tr>"#,
            event.end_time.with_timezone(&Local)
        ));
        format!(
            r#"
            <table border="1" class="dataframe">
            <tbody>{}</tbody>
            </table>"#,
            output.join("")
        )
    } else {
        "".to_string()
    };
    Ok(body)
}

#[derive(Serialize, Deserialize)]
pub struct MaxModifiedQuery {
    pub max_modified: Option<DateTime<Utc>>,
}

pub async fn calendar_list(
    query: MaxModifiedQuery,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let calendar_list = calendar_list_object(query, &data.cal_sync).await?;
    Ok(warp::reply::json(&calendar_list))
}

async fn calendar_list_object(
    query: MaxModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarList>> {
    let max_modified = query
        .max_modified
        .unwrap_or_else(|| Utc::now() - Duration::days(7));
    CalendarList::get_recent(max_modified, &cal_sync.pool)
        .await
        .map_err(Into::into)
}

#[derive(Serialize, Deserialize)]
pub struct CalendarUpdateRequest {
    pub updates: Vec<CalendarList>,
}

pub async fn calendar_list_update(
    payload: CalendarUpdateRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let calendars = calendar_list_update_object(payload, &data.cal_sync).await?;
    Ok(warp::reply::json(&calendars))
}

async fn calendar_list_update_object(
    payload: CalendarUpdateRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<InsertCalendarList>> {
    let futures = payload.updates.into_iter().map(|calendar| {
        let pool = cal_sync.pool.clone();
        let calendar: InsertCalendarList = calendar.into();
        async move { calendar.upsert(&pool).await.map_err(Into::into) }
    });
    try_join_all(futures).await
}

pub async fn calendar_cache(
    query: MaxModifiedQuery,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let events = calendar_cache_events(query, &data.cal_sync).await?;
    Ok(warp::reply::json(&events))
}

async fn calendar_cache_events(
    query: MaxModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarCache>> {
    let max_modified = query
        .max_modified
        .unwrap_or_else(|| Utc::now() - Duration::days(7));
    CalendarCache::get_recent(max_modified, &cal_sync.pool)
        .await
        .map_err(Into::into)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarCacheRequest {
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

impl From<CalendarCacheRequest> for InsertCalendarCache {
    fn from(item: CalendarCacheRequest) -> Self {
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: item.event_start_time,
            event_end_time: item.event_end_time,
            event_url: item.event_url.map(Into::into),
            event_name: item.event_name,
            event_description: item.event_description.map(Into::into),
            event_location_name: item.event_location_name.map(Into::into),
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: item.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CalendarCacheUpdateRequest {
    pub updates: Vec<CalendarCacheRequest>,
}

pub async fn calendar_cache_update(
    payload: CalendarCacheUpdateRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let events = calendar_cache_update_events(payload, &data.cal_sync).await?;
    Ok(warp::reply::json(&events))
}

async fn calendar_cache_update_events(
    payload: CalendarCacheUpdateRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<InsertCalendarCache>> {
    let futures = payload.updates.into_iter().map(|event| {
        let pool = cal_sync.pool.clone();
        let event: InsertCalendarCache = event.into();
        async move { event.upsert(&pool).await.map_err(Into::into) }
    });
    try_join_all(futures).await
}

pub async fn user(user: LoggedUser) -> WarpResult<impl Reply> {
    Ok(warp::reply::json(&user))
}

pub async fn link_shortener(link: StackString, data: AppState) -> WarpResult<impl Reply> {
    let body = link_shortener_body(&link, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn link_shortener_body(link: &str, cal_sync: &CalendarSync) -> HttpResult<String> {
    let config = &cal_sync.config;

    if let Some(link) = SHORTENED_URLS.read().await.get(link) {
        let body = format_short_link(&config.domain, &link);
        return Ok(body.into());
    }

    let pool = &cal_sync.pool;
    if let Some(link) = ShortenedLinks::get_by_shortened_url(&link, pool)
        .await?
        .pop()
    {
        let body = format!(
            r#"<script>window.location.replace("{}")</script>"#,
            link.original_url
        );
        SHORTENED_URLS
            .write()
            .await
            .insert(link.original_url, link.shortened_url);
        Ok(body)
    } else {
        Ok("No url found".to_string())
    }
}

fn format_short_link(domain: &str, link: &str) -> StackString {
    format!(
        r#"<script>window.location.replace("https://{}/calendar/link/{}")</script>"#,
        domain, link
    )
    .into()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildEventRequest {
    pub gcal_id: StackString,
    pub event_id: Option<StackString>,
}

pub async fn build_calendar_event(
    query: BuildEventRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let body = build_calendar_event_body(query, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn build_calendar_event_body(
    query: BuildEventRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<String> {
    let event = if let Some(event_id) = &query.event_id {
        CalendarCache::get_by_gcal_id_event_id(&query.gcal_id, &event_id, &cal_sync.pool).await?
    } else {
        None
    };
    let event = event.map_or_else(
        || Event::new(&query.gcal_id, "", Utc::now(), Utc::now()),
        |event| event.into(),
    );
    let body = format!(
        r#"
        <form action="javascript:createCalendarEvent();">
            Calendar ID: <input type="text" name="gcal_id" id="gcal_id" value="{gcal_id}"/><br>
            Event ID: <input type="text" name="event_id" id="event_id" value="{event_id}"/><br>
            Start Date: <input type="date" name="start_date" id="start_date" value="{start_date}"/><br>
            Start Time: <input type="time" name="start_time" id="start_time" value="{start_time}"/><br>
            End Date: <input type="date" name="end_date" id="end_date" value="{end_date}"/><br>
            End Time: <input type="time" name="end_time" id="end_time" value="{end_time}"/><br>
            Event Name: <input type="text" name="event_name" id="event_name" value="{event_name}"/><br>
            Event Url: <input type="url" name="event_url" id="event_url" value="https://localhost"/><br>
            Event Location Name: <input type="text" name="event_location_name" id="event_location_name" value="{event_location_name}"/><br>
            Event Description: <br><textarea cols=40 rows=20 name="event_description" id="event_description">{event_description}</textarea><br>
            <input type="button" name="create_event" value="Create Event" onclick="createCalendarEvent();"/><br>
        </form>
    "#,
        gcal_id = event.gcal_id,
        event_id = event.event_id,
        start_date = event.start_time.naive_local().date(),
        start_time = event.start_time.naive_local().time().format("%H:%M"),
        end_date = event.end_time.naive_local().date(),
        end_time = event.end_time.naive_local().time().format("%H:%M"),
        event_name = event.name,
        event_location_name = event.location.as_ref().map_or("", |l| l.name.as_str()),
        event_description = event.description.as_ref().map_or("", StackString::as_str),
    );
    Ok(body)
}

#[derive(Serialize, Deserialize)]
pub struct CreateCalendarEventRequest {
    pub gcal_id: StackString,
    pub event_id: StackString,
    pub event_start_date: NaiveDate,
    pub event_start_time: NaiveTime,
    pub event_end_date: NaiveDate,
    pub event_end_time: NaiveTime,
    pub event_url: Option<StackString>,
    pub event_name: StackString,
    pub event_description: Option<StackString>,
    pub event_location_name: Option<StackString>,
}

pub async fn create_calendar_event(
    payload: CreateCalendarEventRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let body = create_calendar_event_body(payload, &data.cal_sync).await?;
    Ok(warp::reply::html(body))
}

async fn create_calendar_event_body(
    payload: CreateCalendarEventRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<String> {
    let start_datetime = NaiveDateTime::new(payload.event_start_date, payload.event_start_time);
    let start_datetime = Local
        .from_local_datetime(&start_datetime)
        .single()
        .unwrap()
        .with_timezone(&Utc);
    let end_datetime = NaiveDateTime::new(payload.event_end_date, payload.event_end_time);
    let end_datetime = Local
        .from_local_datetime(&end_datetime)
        .single()
        .unwrap()
        .with_timezone(&Utc);

    let event = InsertCalendarCache {
        gcal_id: payload.gcal_id,
        event_id: payload.event_id,
        event_start_time: start_datetime,
        event_end_time: end_datetime,
        event_url: payload.event_url,
        event_name: payload.event_name,
        event_description: payload.event_description,
        event_location_name: payload.event_location_name.map(Into::into),
        event_location_lat: None,
        event_location_lon: None,
        last_modified: Utc::now(),
    };

    let event = event.upsert(&cal_sync.pool).await?;
    let event = match CalendarCache::get_by_gcal_id_event_id(
        &event.gcal_id,
        &event.event_id,
        &cal_sync.pool,
    )
    .await?
    {
        Some(event) => event,
        None => {
            return Err(ServiceError::BadRequest(
                "Failed to store event in db".into(),
            ))
        }
    };
    let event: Event = event.into();
    let (gcal_id, event) = event.to_gcal_event()?;
    cal_sync
        .gcal
        .as_ref()
        .ok_or_else(|| format_err!("No gcal instance found"))?
        .insert_gcal_event(&gcal_id, event)
        .await?;

    Ok("Event Inserted".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct EditCalendarRequest {
    pub gcal_id: StackString,
    pub calendar_name: Option<StackString>,
    pub sync: Option<bool>,
    pub edit: Option<bool>,
    pub display: Option<bool>,
}

pub async fn edit_calendar(
    query: EditCalendarRequest,
    _: LoggedUser,
    data: AppState,
) -> WarpResult<impl Reply> {
    let calendar_list = edit_calendar_list(query, &data.cal_sync).await?;
    Ok(warp::reply::json(&calendar_list))
}

async fn edit_calendar_list(
    query: EditCalendarRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<CalendarList> {
    let mut calendar = if let Some(calendar) =
        CalendarList::get_by_gcal_id(&query.gcal_id, &cal_sync.pool)
            .await?
            .pop()
    {
        calendar
    } else {
        return Err(format_err!("No such calendar {}", query.gcal_id).into());
    };
    if let Some(calendar_name) = query.calendar_name.as_ref() {
        calendar.calendar_name = calendar_name.clone();
    }
    if let Some(sync) = query.sync {
        calendar.sync = sync;
    }
    if let Some(edit) = query.edit {
        calendar.edit = edit;
    }
    let calendar = if let Some(display) = query.display {
        calendar.display = display;
        calendar.update_display(&cal_sync.pool).await?
    } else {
        calendar
    };
    calendar.update(&cal_sync.pool).await.map_err(Into::into)
}
