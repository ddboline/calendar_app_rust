use actix_web::{
    http::StatusCode,
    web::{Data, Json, Query},
    HttpResponse,
};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::task::spawn_blocking;

use calendar_app_lib::calendar::Event;
use calendar_app_lib::models::CalendarCache;

use crate::app::AppState;
use crate::errors::ServiceError as Error;
use crate::logged_user::LoggedUser;

fn form_http_response(body: String) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(body))
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
        events.join("<br>")
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
    let body = format!("delete {} {}", &payload.gcal_id, &payload.event_id);
    spawn_blocking(move || {
        data.cal_sync
            .gcal
            .delete_gcal_event(&payload.gcal_id, &payload.event_id)
    })
    .await??;
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
    let events: Vec<_> = data.cal_sync.list_events(&gcal_id, query.min_time, query.max_time).await?.into_iter().map(|event| {
        format!(r#"
                <tr text-style="center">
                <td><input type="button" name="{name}" value="{name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                <td>{start}</td>
                <td>{end}</td>
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
        output.push(event.name);
        if let Some(description) = &event.description {
            let description: Vec<_> = description
                .split('\n')
                .map(|x| format!("\t\t{}", x))
                .collect();
            output.push(description.join("\n"));
        }
        if let Some(url) = &event.url {
            output.push(url.as_str().to_string());
        }
        if let Some(location) = &event.location {
            output.push(location.name.to_string());
            if let Some((lat, lon)) = &location.lat_lon {
                output.push(format!("{},{}", lat, lon));
            }
        }
        output.push(event.start_time.with_timezone(&Local).to_string());
        output.push(event.end_time.with_timezone(&Local).to_string());
        output.join("<br>")
    } else {
        "".to_string()
    };
    form_http_response(body)
}
