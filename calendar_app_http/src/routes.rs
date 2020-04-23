use actix_web::{
    http::StatusCode,
    web::{Data, Json, Path, Query},
    HttpResponse,
};
use anyhow::format_err;
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use futures::future::try_join_all;
use itertools::Itertools;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::{sync::RwLock, task::spawn_blocking};
use url::Url;

use calendar_app_lib::{
    calendar::Event,
    models::{
        CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList, ShortenedLinks,
    },
    stack_string::StackString,
};

use crate::{
    app::AppState,
    errors::{ServiceError as Error, ServiceError},
    logged_user::LoggedUser,
};

lazy_static! {
    static ref SHORTENED_URLS: RwLock<HashMap<StackString, StackString>> =
        RwLock::new(HashMap::new());
}

fn form_http_response(body: String) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(body))
}

fn to_json<T>(js: T) -> Result<HttpResponse, Error>
where
    T: Serialize,
{
    Ok(HttpResponse::Ok().json(js))
}

pub async fn calendar_index(_: LoggedUser, _: Data<AppState>) -> Result<HttpResponse, Error> {
    let body = include_str!("../../templates/index.html").replace("DISPLAY_TEXT", "");

    form_http_response(body)
}

pub async fn agenda(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    let calendar_map: HashMap<_, _> = data
        .cal_sync
        .list_calendars()
        .await?
        .into_iter()
        .filter_map(|cal| {
            if cal.display {
                Some((cal.gcal_id.clone(), cal))
            } else {
                None
            }
        })
        .collect();
    let events: Vec<_> = data
        .cal_sync
        .list_agenda()
        .await?
        .into_iter()
        .sorted_by_key(|event| event.start_time)
        .filter_map(|event| {
            let cal = match calendar_map.get(&event.gcal_id) {
                Some(cal) => cal,
                None => return None,
            };
            let calendar_name = cal.gcal_name.as_ref().unwrap_or_else(|| &cal.name);
            let delete = if cal.edit {
                format!(
                    r#"<input type="button" name="delete_event" value="Delete" onclick="deleteEventAgenda('{gcal_id}', '{event_id}')">"#,
                    gcal_id=event.gcal_id,
                    event_id=event.event_id,
                )
            } else {
                "".to_string()
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
                start_time=event.start_time.with_timezone(&Local),
                delete=delete,
            ))
        })
        .collect();
    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead><th>Calendar</th><th>Event</th><th>Start Time</th></thead>
        <tbody>{}</tbody>
        </table>"#,
        events.join("")
    );
    form_http_response(body)
}

pub async fn sync_calendars(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    let body = data.cal_sync.run_syncing(false).await?.join("<br>");
    form_http_response(body)
}

pub async fn sync_calendars_full(
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let body = data.cal_sync.run_syncing(true).await?.join("<br>");
    form_http_response(body)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteEventPath {
    pub gcal_id: StackString,
    pub event_id: StackString,
}

pub async fn delete_event(
    payload: Json<DeleteEventPath>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let payload = payload.into_inner();

    let body = if let Some(event) = CalendarCache::get_by_gcal_id_event_id(
        &payload.gcal_id,
        &payload.event_id,
        &data.cal_sync.pool,
    )
    .await?
    .pop()
    {
        let body = format!("delete {} {}", &payload.gcal_id, &payload.event_id);
        event.delete(&data.cal_sync.pool).await?;
        let gcal = data.cal_sync.gcal.clone();
        spawn_blocking(move || gcal.delete_gcal_event(&payload.gcal_id, &payload.event_id))
            .await??;
        body
    } else {
        "Event not deleted".to_string()
    };
    form_http_response(body)
}

pub async fn list_calendars(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    let calendars = data
        .cal_sync
        .list_calendars()
        .await?
        .into_iter()
        .filter(|calendar| calendar.sync)
        .sorted_by_key(|calendar| {
            calendar
                .gcal_name
                .as_ref()
                .map_or_else(|| calendar.name.to_string(), ToString::to_string)
        });
    let calendars: Vec<_> = calendars
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
                <td><input type="button" name="list_events" value="{gcal_name}" onclick="listEvents('{calendar_name}')"></td>
                <td>{description}</td>
                <td>{make_visible}</td>
                <td>{create_event}</td>
                </tr>"#,
                gcal_name=calendar.gcal_name.as_ref().map_or_else(|| calendar.name.as_str(), StackString::as_str),
                calendar_name=calendar.name,
                description=calendar.description.as_ref().map_or_else(|| "", StackString::as_str),
                create_event=create_event,
                make_visible=make_visible,
            )
        }).collect();

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
        calendars.join("")
    );
    form_http_response(body)
}

#[derive(Serialize, Deserialize)]
pub struct ListEventsRequest {
    pub calendar_name: StackString,
    pub min_time: Option<NaiveDate>,
    pub max_time: Option<NaiveDate>,
}

pub async fn list_events(
    query: Query<ListEventsRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let query = query.into_inner();
    let calendar_map: HashMap<_, _> = data
        .cal_sync
        .list_calendars()
        .await?
        .into_iter()
        .map(|cal| (cal.name.clone(), cal))
        .collect();
    let cal = match calendar_map.get(&query.calendar_name) {
        Some(cal) => cal,
        None => return form_http_response("".to_string()),
    };
    let events: Vec<_> = data.cal_sync.list_events(&cal.gcal_id, query.min_time, query.max_time).await?
        .into_iter()
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
            format!(r#"
                    <tr text-style="center">
                    <td><input type="button" name="{name}" value="{name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start}</td>
                    <td>{end}</td>
                    <td>{delete}</td>
                    </tr>
                "#,
                name=event.name,
                start=event.start_time.with_timezone(&Local),
                end=event.end_time.with_timezone(&Local),
                gcal_id=event.gcal_id,
                event_id=event.event_id,
                delete=delete
            )
        }).collect();
    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead>
        <th>Event</th><th>Start Time</th><th>End Time</th>
        <th><input type="button" name="create_event" value="Create Event" onclick="buildEvent('{}')"></th>
        </thead>
        <tbody>{}</tbody>
        </table>"#,
        cal.gcal_id,
        events.join("")
    );
    form_http_response(body)
}

pub async fn event_detail(
    payload: Json<DeleteEventPath>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let payload = payload.into_inner();
    let body = if let Some(event) = CalendarCache::get_by_gcal_id_event_id(
        &payload.gcal_id,
        &payload.event_id,
        &data.cal_sync.pool,
    )
    .await?
    .pop()
    {
        let event: Event = event.into();
        let mut output = Vec::new();
        output.push(format!(
            r#"<tr text-style="center"><td>Name</td><td>{}</td></tr>"#,
            &event.name
        ));
        if let Some(description) = &event.description {
            let description: Vec<_> = description
                .split('\n')
                .map(|line| {
                    let mut line_length = 0;
                    let words: Vec<_> = line
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
                        .collect();
                    format!("\t\t{}", words.join(" "))
                })
                .collect();
            output.push(format!(
                r#"<tr text-style="center"><td>Description</td><td>{}</td></tr>"#,
                &description.join("")
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
    form_http_response(body)
}

pub async fn calendar_list(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    let calendars =
        if let Some(max_modified) = CalendarList::get_max_modified(&data.cal_sync.pool).await? {
            CalendarList::get_recent(max_modified, &data.cal_sync.pool).await?
        } else {
            Vec::new()
        };
    to_json(calendars)
}

#[derive(Serialize, Deserialize)]
pub struct CalendarUpdateRequest {
    pub updates: Vec<CalendarList>,
}

pub async fn calendar_list_update(
    payload: Json<CalendarUpdateRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let payload = payload.into_inner();
    let futures = payload.updates.into_iter().map(|calendar| {
        let pool = data.cal_sync.pool.clone();
        let calendar: InsertCalendarList = calendar.into();
        async move { calendar.upsert(&pool).await.map_err(Into::into) }
    });
    let results: Result<Vec<_>, Error> = try_join_all(futures).await;
    let calendars = results?;
    to_json(calendars)
}

pub async fn calendar_cache(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    let events =
        if let Some(max_modified) = CalendarCache::get_max_modified(&data.cal_sync.pool).await? {
            CalendarCache::get_recent(max_modified, &data.cal_sync.pool).await?
        } else {
            Vec::new()
        };
    to_json(events)
}

#[derive(Serialize, Deserialize)]
pub struct CalendarCacheUpdateRequest {
    pub updates: Vec<CalendarCache>,
}

pub async fn calendar_cache_update(
    payload: Json<CalendarCacheUpdateRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let payload = payload.into_inner();
    let futures = payload.updates.into_iter().map(|event| {
        let pool = data.cal_sync.pool.clone();
        let event: InsertCalendarCache = event.into();
        async move { event.upsert(&pool).await.map_err(Into::into) }
    });
    let results: Result<Vec<_>, Error> = try_join_all(futures).await;
    let events = results?;
    to_json(events)
}

pub async fn user(user: LoggedUser) -> Result<HttpResponse, Error> {
    to_json(user)
}

#[derive(Serialize, Deserialize)]
pub struct LinkRequest {
    pub link: StackString,
}

pub async fn link_shortener(
    link: Path<LinkRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let link = &link.link;
    let config = &data.cal_sync.config;

    if let Some(link) = SHORTENED_URLS.read().await.get(link) {
        let body = format_short_link(&config.domain, &link);
        return form_http_response(body);
    }

    let pool = &data.cal_sync.pool;
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
        form_http_response(body)
    } else {
        form_http_response("No url found".to_string())
    }
}

fn format_short_link(domain: &str, link: &str) -> String {
    format!(
        r#"<script>window.location.replace("https://{}/calendar/link/{}")</script>"#,
        domain, link
    )
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildEventRequest {
    pub gcal_id: StackString,
    pub event_id: Option<StackString>,
}

pub async fn build_calendar_event(
    query: Query<BuildEventRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let query = query.into_inner();
    let mut events = if let Some(event_id) = &query.event_id {
        CalendarCache::get_by_gcal_id_event_id(&query.gcal_id, &event_id, &data.cal_sync.pool)
            .await?
    } else {
        Vec::new()
    };
    let event = events.pop().map_or_else(
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
    form_http_response(body)
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
    pub event_description: Option<String>,
    pub event_location_name: Option<StackString>,
}

pub async fn create_calendar_event(
    payload: Json<CreateCalendarEventRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let payload = payload.into_inner();
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
        event_description: payload.event_description.as_ref().map(Into::into),
        event_location_name: payload.event_location_name,
        event_location_lat: None,
        event_location_lon: None,
        last_modified: Utc::now(),
    };

    let event = event.upsert(&data.cal_sync.pool).await?;
    let event = match CalendarCache::get_by_gcal_id_event_id(
        &event.gcal_id,
        &event.event_id,
        &data.cal_sync.pool,
    )
    .await?
    .pop()
    {
        Some(event) => event,
        None => {
            return Err(ServiceError::BadRequest(
                "Failed to store event in db".to_string(),
            ))
        }
    };
    let event: Event = event.into();
    let (gcal_id, event) = event.to_gcal_event()?;
    spawn_blocking(move || data.cal_sync.gcal.insert_gcal_event(&gcal_id, event)).await??;

    form_http_response("Event Inserted".to_string())
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
    query: Query<EditCalendarRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let mut calendar = if let Some(calendar) =
        CalendarList::get_by_gcal_id(&query.gcal_id, &data.cal_sync.pool)
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
    if let Some(display) = query.display {
        calendar.display = display;
    }
    to_json(calendar.update(&data.cal_sync.pool).await?)
}
