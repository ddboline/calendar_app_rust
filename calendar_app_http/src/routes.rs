use anyhow::format_err;
use futures::future::try_join_all;
use itertools::Itertools;
use rweb::{delete, get, post, Json, Query, Rejection, Schema};
use rweb_helper::{
    html_response::HtmlResponse as HtmlBase, json_response::JsonResponse as JsonBase, DateTimeType,
    DateType, RwebResponse,
};
use serde::{Deserialize, Serialize};
use stack_string::{format_sstr, StackString};
use std::collections::HashMap;
use time::{macros::format_description, Duration, OffsetDateTime};
use time_tz::OffsetDateTimeExt;
use url::Url;

use calendar_app_lib::{
    calendar::Event,
    calendar_sync::CalendarSync,
    get_default_or_local_time,
    models::{CalendarCache, CalendarList, ShortenedLinks},
    timezone::TimeZone,
};

use crate::{
    app::{AppState, UrlCache},
    errors::ServiceError as Error,
    logged_user::LoggedUser,
    CalendarCacheWrapper, CalendarListWrapper, MinModifiedQuery,
};

pub type WarpResult<T> = Result<T, Rejection>;
pub type HttpResult<T> = Result<T, Error>;

#[derive(RwebResponse)]
#[response(description = "Main Page", content = "html")]
struct IndexResponse(HtmlBase<String, Error>);

#[get("/calendar/index.html")]
pub async fn calendar_index(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
) -> WarpResult<IndexResponse> {
    let body = include_str!("../../templates/index.html").replace("DISPLAY_TEXT", "");
    Ok(HtmlBase::new(body).into())
}

#[derive(RwebResponse)]
#[response(description = "Agenda", content = "html")]
struct AgendaResponse(HtmlBase<StackString, Error>);

#[get("/calendar/agenda")]
pub async fn agenda(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<AgendaResponse> {
    let body = agenda_body(data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn agenda_body(cal_sync: CalendarSync) -> HttpResult<StackString> {
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
                format_sstr!(
                    r#"<input type="button" name="delete_event" value="Delete" onclick="deleteEventAgenda('{gcal_id}', '{event_id}')">"#,
                    gcal_id=event.gcal_id,
                    event_id=event.event_id,
                )
            } else {
                "".into()
            };
            let start_time = get_default_or_local_time(event.start_time.into(), &cal_sync.config);
            Some(format_sstr!(
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
            ))
        })
        .join("");
    let body = format_sstr!(
        r#"
        <table border="1" class="dataframe">
        <thead><th>Calendar</th><th>Event</th><th>Start Time</th></thead>
        <tbody>{events}</tbody>
        </table>"#
    );
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "Sync Output", content = "html")]
struct SyncResponse(HtmlBase<String, Error>);

#[get("/calendar/sync_calendars")]
pub async fn sync_calendars(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<SyncResponse> {
    let body = sync_calendars_body(&data.cal_sync, false).await?;
    Ok(HtmlBase::new(body).into())
}

async fn sync_calendars_body(cal_sync: &CalendarSync, do_full: bool) -> HttpResult<String> {
    Ok(cal_sync.run_syncing(do_full).await?.join("<br>"))
}

#[get("/calendar/sync_calendars_full")]
pub async fn sync_calendars_full(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<SyncResponse> {
    let body = sync_calendars_body(&data.cal_sync, true).await?;
    Ok(HtmlBase::new(body).into())
}

#[derive(Serialize, Deserialize, Debug, Schema)]
pub struct GcalEventID {
    #[schema(description = "GCal ID")]
    pub gcal_id: StackString,
    #[schema(description = "GCal Event ID")]
    pub event_id: StackString,
}

#[derive(RwebResponse)]
#[response(
    description = "Delete Event Output",
    content = "html",
    status = "CREATED"
)]
struct DeleteEventResponse(HtmlBase<StackString, Error>);

#[delete("/calendar/delete_event")]
pub async fn delete_event(
    payload: Json<GcalEventID>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<DeleteEventResponse> {
    let payload = payload.into_inner();
    let body = delete_event_body(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn delete_event_body(
    payload: GcalEventID,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let body = if let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&payload.gcal_id, &payload.event_id, &cal_sync.pool)
            .await?
    {
        let body = format_sstr!("delete {} {}", &payload.gcal_id, &payload.event_id);
        event.delete(&cal_sync.pool).await?;
        cal_sync
            .gcal
            .as_ref()
            .ok_or_else(|| format_err!("No gcal instance found"))?
            .delete_gcal_event(&payload.gcal_id, &payload.event_id)
            .await?;
        body
    } else {
        "Event not deleted".into()
    };
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "List Calendars", content = "html")]
struct ListCalendarsResponse(HtmlBase<StackString, Error>);

#[get("/calendar/list_calendars")]
pub async fn list_calendars(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<ListCalendarsResponse> {
    let body = list_calendars_body(&data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn list_calendars_body(cal_sync: &CalendarSync) -> HttpResult<StackString> {
    let calendars = cal_sync
        .list_calendars()
        .await?
        .filter(|calendar| calendar.sync)
        .sorted_by_key(|calendar| {
            calendar
                .gcal_name
                .as_ref()
                .map_or_else(|| calendar.name.clone(), Clone::clone)
        });
    let calendars = calendars
        .map(|calendar| {
            let create_event = if calendar.edit {
                format_sstr!(r#"
                    <input type="button" name="create_event" value="Create Event" onclick="buildEvent('{}')">
                "#, calendar.gcal_id)
            } else {
                "".into()
            };
            let make_visible = if calendar.display {
                format_sstr!(r#"
                    <input type="button" name="hide_calendar" value="Hide" onclick="calendarDisplay('{}', false)">
                "#, calendar.gcal_id)
            } else {
                format_sstr!(r#"
                <input type="button" name="show_calendar" value="Show" onclick="calendarDisplay('{}', true)">
                "#, calendar.gcal_id)
            };
            format_sstr!(r#"
                <tr text-style="center">
                <td><input type="button" name="list_events" value="{calendar_name}" onclick="listEvents('{calendar_name}')"></td>
                <td>{description}</td>
                <td>{make_visible}</td>
                <td>{create_event}</td>
                </tr>"#,
                calendar_name=calendar.name,
                description=calendar.description.as_ref().map_or_else(|| "", StackString::as_str),
            )
        }).join("");

    let body = format_sstr!(
        r#"
        <table border="1" class="dataframe">
        <thead>
        <th>Calendar</th>
        <th>Description</th>
        <th></th>
        <th><input type="button" name="sync_all" value="Full Sync" onclick="syncCalendarsFull();"/></th>
        </thead>
        <tbody>{calendars}</tbody>
        </table>"#,
    );
    Ok(body)
}

#[derive(Serialize, Deserialize, Schema)]
pub struct ListEventsRequest {
    #[schema(description = "Calendar Name")]
    pub calendar_name: StackString,
    #[schema(description = "Earliest Date")]
    pub min_time: Option<DateType>,
    #[schema(description = "Latest Date")]
    pub max_time: Option<DateType>,
}

#[derive(RwebResponse)]
#[response(description = "List Events", content = "html")]
struct ListEventsResponse(HtmlBase<StackString, Error>);

#[get("/calendar/list_events")]
pub async fn list_events(
    query: Query<ListEventsRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<ListEventsResponse> {
    let query = query.into_inner();
    let body = list_events_body(query, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn list_events_body(
    query: ListEventsRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let calendar_map: HashMap<_, _> = cal_sync
        .list_calendars()
        .await?
        .map(|cal| (cal.name.clone(), cal))
        .collect();
    let cal = match calendar_map.get(&query.calendar_name) {
        Some(cal) => cal,
        None => return Ok("".into()),
    };
    let min_time = query.min_time.map(Into::into);
    let max_time = query.max_time.map(Into::into);
    let events = cal_sync.list_events(&cal.gcal_id, min_time, max_time).await?
        .sorted_by_key(|event| event.start_time)
        .map(|event| {
            let delete = if cal.edit {
                format_sstr!(
                    r#"<input type="button" name="delete_event" value="Delete" onclick="deleteEventList('{gcal_id}', '{event_id}', '{calendar_name}')">"#,
                    gcal_id=event.gcal_id,
                    event_id=event.event_id,
                    calendar_name=query.calendar_name,
                )
            } else {
                "".into()
            };
            let start_time = get_default_or_local_time(event.start_time.into(), &cal_sync.config);
            let end_time = get_default_or_local_time(event.end_time.into(), &cal_sync.config);

            format_sstr!(r#"
                    <tr text-style="center">
                    <td><input type="button" name="{name}" value="{name}" onclick="eventDetail('{gcal_id}', '{event_id}')"></td>
                    <td>{start_time}</td>
                    <td>{end_time}</td>
                    <td>{delete}</td>
                    </tr>
                "#,
                name=event.name,
                gcal_id=event.gcal_id,
                event_id=event.event_id,
            )
        }).join("");
    let body = format_sstr!(
        r#"
        <table border="1" class="dataframe">
        <thead>
        <th>Event</th><th>Start Time</th><th>End Time</th>
        <th><input type="button" name="create_event" value="Create Event" onclick="buildEvent('{gcal_id}')"></th>
        </thead>
        <tbody>{events}</tbody>
        </table>"#,
        gcal_id = cal.gcal_id
    );
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "Event Details", content = "html", status = "CREATED")]
struct EventDetailResponse(HtmlBase<StackString, Error>);

#[post("/calendar/event_detail")]
pub async fn event_detail(
    payload: Json<GcalEventID>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<EventDetailResponse> {
    let payload = payload.into_inner();
    let body = event_detail_body(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn event_detail_body(
    payload: GcalEventID,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let body = if let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&payload.gcal_id, &payload.event_id, &cal_sync.pool)
            .await?
    {
        let event: Event = event.into();
        let mut output = vec![format_sstr!(
            r#"<tr text-style="center"><td>Name</td><td>{}</td></tr>"#,
            &event.name
        )];
        if let Some(description) = &event.description {
            let description = description
                .split('\n')
                .map(|line| {
                    let mut line_length = 0;
                    let words = line
                        .split_whitespace()
                        .map(|word| {
                            let mut output_word = StackString::new();
                            if let Ok(url) = word.parse::<Url>() {
                                if url.scheme() == "https" {
                                    output_word = format_sstr!(r#"<a href="{url}">Link</a>"#);
                                }
                            } else {
                                output_word = word.into();
                            }
                            line_length += output_word.len();
                            if line_length > 60 {
                                output_word = format_sstr!("<br>{output_word}");
                                line_length = 0;
                            }
                            output_word
                        })
                        .join(" ");
                    format_sstr!("\t\t{words}")
                })
                .join("");
            output.push(format_sstr!(
                r#"<tr text-style="center"><td>Description</td><td>{description}</td></tr>"#,
            ));
        }
        if let Some(url) = &event.url {
            output.push(format_sstr!(
                r#"<tr text-style="center"><td>Url</td><td><a href={url}>Link</a></td></tr>"#
            ));
        }
        if let Some(location) = &event.location {
            output.push(format_sstr!(
                r#"<tr text-style="center"><td>Location</td><td>{}</td></tr>"#,
                location.name
            ));
            if let Some((lat, lon)) = &location.lat_lon {
                output.push(format_sstr!(
                    r#"<tr text-style="center"><td>Lat,Lon:</td><td>{lat},{lon}</td></tr>"#
                ));
            }
        }
        output.push(format_sstr!(
            r#"<tr text-style="center"><td>Start Time</td><td>{}</td></tr>"#,
            get_default_or_local_time(event.start_time.into(), &cal_sync.config)
        ));
        output.push(format_sstr!(
            r#"<tr text-style="center"><td>End Time</td><td>{}</td></tr>"#,
            get_default_or_local_time(event.end_time.into(), &cal_sync.config)
        ));
        format_sstr!(
            r#"
            <table border="1" class="dataframe">
            <tbody>{}</tbody>
            </table>"#,
            output.join("")
        )
    } else {
        "".into()
    };
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "Calendar List")]
struct CalendarListResponse(JsonBase<Vec<CalendarListWrapper>, Error>);

#[get("/calendar/calendar_list")]
pub async fn calendar_list(
    query: Query<MinModifiedQuery>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarListResponse> {
    let query = query.into_inner();
    let calendar_list = calendar_list_object(query, &data.cal_sync).await?;
    Ok(JsonBase::new(calendar_list).into())
}

async fn calendar_list_object(
    query: MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarListWrapper>> {
    let min_modified = query
        .min_modified
        .map_or_else(|| OffsetDateTime::now_utc() - Duration::days(7), Into::into);
    let cal_list = CalendarList::get_recent(min_modified, &cal_sync.pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(cal_list)
}

#[derive(Serialize, Deserialize, Schema)]
pub struct CalendarUpdateRequest {
    #[schema(description = "Calendar List Updates")]
    pub updates: Vec<CalendarListWrapper>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar List Update", status = "CREATED")]
struct CalendarListUpdateResponse(JsonBase<Vec<CalendarListWrapper>, Error>);

#[post("/calendar/calendar_list")]
pub async fn calendar_list_update(
    payload: Json<CalendarUpdateRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarListUpdateResponse> {
    let payload = payload.into_inner();
    let calendars = calendar_list_update_object(payload, &data.cal_sync).await?;
    Ok(JsonBase::new(calendars).into())
}

async fn calendar_list_update_object(
    payload: CalendarUpdateRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarListWrapper>> {
    let futures = payload.updates.into_iter().map(|calendar| {
        let pool = cal_sync.pool.clone();
        let calendar: CalendarList = calendar.into();
        async move {
            calendar.upsert(&pool).await?;
            Ok(calendar.into())
        }
    });
    try_join_all(futures).await
}

#[derive(RwebResponse)]
#[response(description = "Calendar Cache")]
struct CalendarCacheResponse(JsonBase<Vec<CalendarCacheWrapper>, Error>);

#[get("/calendar/calendar_cache")]
pub async fn calendar_cache(
    query: Query<MinModifiedQuery>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarCacheResponse> {
    let query = query.into_inner();
    let events = calendar_cache_events(query, &data.cal_sync)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(JsonBase::new(events).into())
}

async fn calendar_cache_events(
    query: MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarCache>> {
    let min_modified = query
        .min_modified
        .map_or_else(|| OffsetDateTime::now_utc() - Duration::days(7), Into::into);
    CalendarCache::get_recent(min_modified, &cal_sync.pool)
        .await
        .map_err(Into::into)
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
pub struct CalendarCacheRequest {
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Calendar Event ID")]
    pub event_id: StackString,
    #[schema(description = "Event Start Time")]
    pub event_start_time: DateTimeType,
    #[schema(description = "Event End Time")]
    pub event_end_time: DateTimeType,
    #[schema(description = "Event URL")]
    pub event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    pub event_name: StackString,
    #[schema(description = "Event Description")]
    pub event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    pub event_location_name: Option<StackString>,
    #[schema(description = "Event Location Latitude")]
    pub event_location_lat: Option<f64>,
    #[schema(description = "Event Location Longitude")]
    pub event_location_lon: Option<f64>,
    #[schema(description = "Last Modified")]
    pub last_modified: DateTimeType,
}

impl From<CalendarCacheRequest> for CalendarCache {
    fn from(item: CalendarCacheRequest) -> Self {
        let event_start_time: OffsetDateTime = item.event_start_time.into();
        let event_end_time: OffsetDateTime = item.event_end_time.into();
        let last_modified: OffsetDateTime = item.last_modified.into();
        Self {
            gcal_id: item.gcal_id,
            event_id: item.event_id,
            event_start_time: event_start_time.into(),
            event_end_time: event_end_time.into(),
            event_url: item.event_url.map(Into::into),
            event_name: item.event_name,
            event_description: item.event_description.map(Into::into),
            event_location_name: item.event_location_name.map(Into::into),
            event_location_lat: item.event_location_lat,
            event_location_lon: item.event_location_lon,
            last_modified: last_modified.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Schema)]
pub struct CalendarCacheUpdateRequest {
    #[schema(description = "Calendar Events Update")]
    pub updates: Vec<CalendarCacheRequest>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar Cache Update")]
struct CalendarCacheUpdateResponse(JsonBase<Vec<CalendarCacheWrapper>, Error>);

#[post("/calendar/calendar_cache")]
pub async fn calendar_cache_update(
    payload: Json<CalendarCacheUpdateRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarCacheUpdateResponse> {
    let payload = payload.into_inner();
    let events = calendar_cache_update_events(payload, &data.cal_sync).await?;
    Ok(JsonBase::new(events).into())
}

async fn calendar_cache_update_events(
    payload: CalendarCacheUpdateRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<Vec<CalendarCacheWrapper>> {
    let futures = payload.updates.into_iter().map(|event| {
        let pool = cal_sync.pool.clone();
        let event: CalendarCache = event.into();
        async move {
            event.upsert(&pool).await?;
            Ok(event.into())
        }
    });
    try_join_all(futures).await
}

#[derive(RwebResponse)]
#[response(description = "Logged in User")]
struct UserResponse(JsonBase<LoggedUser, Error>);

#[get("/calendar/user")]
pub async fn user(#[filter = "LoggedUser::filter"] user: LoggedUser) -> WarpResult<UserResponse> {
    Ok(JsonBase::new(user).into())
}

#[derive(RwebResponse)]
#[response(description = "Shortened Link", content = "html")]
struct ShortenedLinkResponse(HtmlBase<StackString, Error>);

#[get("/calendar/link/{link}")]
pub async fn link_shortener(
    link: StackString,
    #[data] data: AppState,
) -> WarpResult<ShortenedLinkResponse> {
    let body = link_shortener_body(&link, &data.cal_sync, &data.shortened_urls).await?;
    Ok(HtmlBase::new(body).into())
}

async fn link_shortener_body(
    link: &str,
    cal_sync: &CalendarSync,
    shortened_urls: &UrlCache,
) -> HttpResult<StackString> {
    let config = &cal_sync.config;

    if let Some(link) = shortened_urls.read().await.get(link) {
        let body = format_short_link(&config.domain, link);
        return Ok(body);
    }

    let pool = &cal_sync.pool;
    if let Some(link) = ShortenedLinks::get_by_shortened_url(link, pool).await? {
        let body = format_sstr!(
            r#"<script>window.location.replace("{}")</script>"#,
            link.original_url
        );
        shortened_urls
            .write()
            .await
            .insert(link.original_url, link.shortened_url);
        Ok(body)
    } else {
        Ok("No url found".into())
    }
}

fn format_short_link(domain: &str, link: &str) -> StackString {
    format_sstr!(
        r#"<script>window.location.replace("https://{domain}/calendar/link/{link}")</script>"#
    )
}

#[derive(Serialize, Deserialize, Debug, Schema)]
pub struct BuildEventRequest {
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Event ID")]
    pub event_id: Option<StackString>,
}

#[derive(RwebResponse)]
#[response(description = "Build Calendar Event", content = "html")]
struct BuildCalendarEventResponse(HtmlBase<StackString, Error>);

#[get("/calendar/create_calendar_event")]
pub async fn build_calendar_event(
    query: Query<BuildEventRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<BuildCalendarEventResponse> {
    let query = query.into_inner();
    let body = build_calendar_event_body(query, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn build_calendar_event_body(
    query: BuildEventRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let event = if let Some(event_id) = &query.event_id {
        CalendarCache::get_by_gcal_id_event_id(&query.gcal_id, event_id, &cal_sync.pool).await?
    } else {
        None
    };
    let event = event.map_or_else(
        || {
            Event::new(
                query.gcal_id,
                StackString::new(),
                OffsetDateTime::now_utc(),
                OffsetDateTime::now_utc(),
            )
        },
        Into::into,
    );
    let body = format_sstr!(
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
        start_date = event.start_time.date(),
        start_time = event
            .start_time
            .time()
            .format(format_description!("[hour]:[minute]"))
            .unwrap_or_else(|_| "00:00".into()),
        end_date = event.end_time.date(),
        end_time = event
            .end_time
            .time()
            .format(format_description!("[hour]:[minute]"))
            .unwrap_or_else(|_| "00:00".into()),
        event_name = event.name,
        event_location_name = event.location.as_ref().map_or("", |l| l.name.as_str()),
        event_description = event.description.as_ref().map_or("", StackString::as_str),
    );
    Ok(body)
}

#[derive(Serialize, Deserialize, Schema)]
pub struct CreateCalendarEventRequest {
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Event ID")]
    pub event_id: StackString,
    #[schema(description = "Event Start Time")]
    pub event_start_datetime: DateTimeType,
    #[schema(description = "Event End Time")]
    pub event_end_datetime: DateTimeType,
    #[schema(description = "Event URL")]
    pub event_url: Option<StackString>,
    #[schema(description = "Event Name")]
    pub event_name: StackString,
    #[schema(description = "Event Description")]
    pub event_description: Option<StackString>,
    #[schema(description = "Event Location Name")]
    pub event_location_name: Option<StackString>,
}

#[derive(RwebResponse)]
#[response(
    description = "Create Calendar Event",
    content = "html",
    status = "CREATED"
)]
struct CreateCalendarEventResponse(HtmlBase<String, Error>);

#[post("/calendar/create_calendar_event")]
pub async fn create_calendar_event(
    payload: Json<CreateCalendarEventRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CreateCalendarEventResponse> {
    let payload = payload.into_inner();
    let body = create_calendar_event_body(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn create_calendar_event_body(
    payload: CreateCalendarEventRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<String> {
    let local = TimeZone::local().into();
    let start_datetime = payload.event_start_datetime.to_timezone(local);
    let end_datetime = payload.event_end_datetime.to_timezone(local);

    let event = CalendarCache {
        gcal_id: payload.gcal_id,
        event_id: payload.event_id,
        event_start_time: start_datetime.into(),
        event_end_time: end_datetime.into(),
        event_url: payload.event_url,
        event_name: payload.event_name,
        event_description: payload.event_description,
        event_location_name: payload.event_location_name.map(Into::into),
        event_location_lat: None,
        event_location_lon: None,
        last_modified: OffsetDateTime::now_utc().into(),
    };

    event.upsert(&cal_sync.pool).await?;
    let event = match CalendarCache::get_by_gcal_id_event_id(
        &event.gcal_id,
        &event.event_id,
        &cal_sync.pool,
    )
    .await?
    {
        Some(event) => event,
        None => return Err(Error::BadRequest("Failed to store event in db".into())),
    };
    let event: Event = event.into();
    let (gcal_id, event) = event.to_gcal_event();
    cal_sync
        .gcal
        .as_ref()
        .ok_or_else(|| format_err!("No gcal instance found"))?
        .insert_gcal_event(&gcal_id, event)
        .await?;

    Ok("Event Inserted".to_string())
}

#[derive(Serialize, Deserialize, Schema)]
pub struct EditCalendarRequest {
    #[schema(description = "GCal Calendar ID")]
    pub gcal_id: StackString,
    #[schema(description = "Calendar Name")]
    pub calendar_name: Option<StackString>,
    #[schema(description = "Sync Flag")]
    pub sync: Option<bool>,
    #[schema(description = "Edit Flag")]
    pub edit: Option<bool>,
    #[schema(description = "Display Flag")]
    pub display: Option<bool>,
}

#[derive(RwebResponse)]
#[response(description = "Edit Calendar Event")]
struct EditCalendarResponse(JsonBase<CalendarListWrapper, Error>);

#[get("/calendar/edit_calendar")]
pub async fn edit_calendar(
    query: Query<EditCalendarRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<EditCalendarResponse> {
    let query = query.into_inner();
    let calendar_list = edit_calendar_list(query, &data.cal_sync).await?;
    Ok(JsonBase::new(calendar_list).into())
}

async fn edit_calendar_list(
    query: EditCalendarRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<CalendarListWrapper> {
    let mut calendar = if let Some(calendar) =
        CalendarList::get_by_gcal_id(&query.gcal_id, &cal_sync.pool).await?
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
        calendar.update_display(&cal_sync.pool).await?;
        calendar
    } else {
        calendar
    };
    calendar.update(&cal_sync.pool).await?;
    Ok(calendar.into())
}
