use actix_web::{
    http::StatusCode,
    web::{Data, Json, Query},
    HttpResponse,
};
use chrono::{Local, NaiveDate};
use futures::future::try_join_all;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::task::spawn_blocking;
use url::Url;

use calendar_app_lib::{
    calendar::Event,
    models::{CalendarCache, CalendarList, InsertCalendarCache, InsertCalendarList},
};

use crate::{app::AppState, errors::ServiceError as Error, logged_user::LoggedUser};

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
        .map(|cal| (cal.gcal_id.to_string(), cal))
        .collect();
    let events: Vec<_> = data
        .cal_sync
        .list_agenda()
        .await?
        .into_iter()
        .sorted_by_key(|event| event.start_time)
        .filter_map(|event| {
            let calendar_name = match calendar_map.get(&event.gcal_id) {
                Some(cal) => cal.gcal_name.as_ref().unwrap_or_else(|| &cal.name),
                None => return None,
            };
            Some(format!(
                r#"
                    <tr text-style="center">
                    <td>{calendar_name}</td>
                    <td><input type="button" name="event_detail" value="{event_name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start_time}</td>
                    <td><input type="button" name="delete_event" value="Delete" onclick="deleteEvent('{gcal_id}', '{event_id}')"></td>
                    </tr>
                "#,
                calendar_name=calendar_name,
                gcal_id=event.gcal_id,
                event_id=event.event_id,
                event_name=event.name,
                start_time=event.start_time.with_timezone(&Local),
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
    pub gcal_id: String,
    pub event_id: String,
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
        .filter(|calendar| calendar.sync);
    let calendars: Vec<_> = calendars
        .map(|calendar| {
            format!(r#"
                <tr text-style="center">
                <td><input type="button" name="list_events" value="{gcal_name}" onclick="listEvents('{calendar_name}')"></td>
                <td>{description}</td>
                </tr>"#,
                gcal_name=calendar.gcal_name.as_ref().map_or_else(|| calendar.name.as_str(), String::as_str),
                calendar_name=calendar.name,
                description=calendar.description.as_ref().map_or_else(|| "", String::as_str),
            )
        }).collect();

    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead><th>Calendar</th><th>Description</th></thead>
        <tbody>{}</tbody>
        </table>"#,
        calendars.join("")
    );
    form_http_response(body)
}

#[derive(Serialize, Deserialize)]
pub struct ListEventsRequest {
    pub calendar_name: String,
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
        .map(|cal| (cal.name.to_string(), cal))
        .collect();
    let gcal_id = match calendar_map.get(&query.calendar_name) {
        Some(cal) => &cal.gcal_id,
        None => return form_http_response("".to_string()),
    };
    let events: Vec<_> = data.cal_sync.list_events(&gcal_id, query.min_time, query.max_time).await?
        .into_iter()
        .sorted_by_key(|event| event.start_time)
        .map(|event| {
            format!(r#"
                    <tr text-style="center">
                    <td><input type="button" name="{name}" value="{name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start}</td>
                    <td>{end}</td>
                    <td><input type="button" name="delete_event" value="Delete" onclick="deleteEvent('{gcal_id}', '{event_id}')"></td>
                    </tr>
                "#,
                name=event.name,
                start=event.start_time.with_timezone(&Local),
                end=event.end_time.with_timezone(&Local),
                gcal_id=event.gcal_id,
                event_id=event.event_id,
            )
        }).collect();
    let body = format!(
        r#"
        <table border="1" class="dataframe">
        <thead><th>Event</th><th>Start Time</th><th>End Time</th></thead>
        <tbody>{}</tbody>
        </table>"#,
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
