use anyhow::format_err;
use futures::{future, stream::FuturesUnordered, TryStreamExt};
use rweb::{delete, get, post, Json, Query, Rejection, Schema};
use rweb_helper::{
    html_response::HtmlResponse as HtmlBase, json_response::JsonResponse as JsonBase, DateType,
    RwebResponse,
};
use serde::{Deserialize, Serialize};
use stack_string::{format_sstr, StackString};
use std::collections::HashMap;
use time::OffsetDateTime;
use time_tz::OffsetDateTimeExt;

use calendar_app_lib::{
    calendar::Event,
    calendar_sync::CalendarSync,
    models::{CalendarCache, CalendarList, ShortenedLinks},
    timezone::TimeZone,
};

use crate::{
    app::{AppState, UrlCache},
    elements::{
        agenda_body, build_event_body, event_detail_body, index_body, list_calendars_body,
        list_events_body,
    },
    errors::ServiceError as Error,
    logged_user::LoggedUser,
    CalendarCacheRequest, CalendarCacheWrapper, CalendarListWrapper, CreateCalendarEventRequest,
    MinModifiedQuery,
};

pub type WarpResult<T> = Result<T, Rejection>;
pub type HttpResult<T> = Result<T, Error>;

#[derive(RwebResponse)]
#[response(description = "Main Page", content = "html")]
struct IndexResponse(HtmlBase<String, Error>);

#[get("/calendar/index.html")]
#[openapi(description = "Calendar App Main Page")]
pub async fn calendar_index(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
) -> WarpResult<IndexResponse> {
    let body = index_body()?;
    Ok(HtmlBase::new(body).into())
}

#[derive(RwebResponse)]
#[response(description = "Agenda", content = "html")]
struct AgendaResponse(HtmlBase<StackString, Error>);

#[get("/calendar/agenda")]
#[openapi(description = "Calendar Agenda Page")]
pub async fn agenda(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<AgendaResponse> {
    let body = get_agenda(data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_agenda(cal_sync: CalendarSync) -> HttpResult<StackString> {
    let calendar_map: HashMap<_, _> = cal_sync
        .list_calendars()
        .await?
        .try_filter_map(|cal| async move {
            if cal.display {
                Ok(Some((cal.gcal_id.clone(), cal)))
            } else {
                Ok(None)
            }
        })
        .try_collect()
        .await?;
    let mut events = cal_sync.list_agenda(1, 2).await?;
    events.sort_by_key(|event| event.start_time);
    let body = agenda_body(calendar_map, events, cal_sync.config.clone())?.into();
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "Sync Output", content = "html")]
struct SyncResponse(HtmlBase<String, Error>);

#[post("/calendar/sync_calendars")]
#[openapi(description = "Sync Calendars")]
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

#[post("/calendar/sync_calendars_full")]
#[openapi(description = "Fully Sync All Calendars")]
pub async fn sync_calendars_full(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<SyncResponse> {
    let body = sync_calendars_body(&data.cal_sync, true).await?;
    Ok(HtmlBase::new(body).into())
}

#[derive(Serialize, Deserialize, Debug, Schema)]
#[schema(component = "GcalEventID")]
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
    status = "NO_CONTENT"
)]
struct DeleteEventResponse(HtmlBase<StackString, Error>);

#[delete("/calendar/delete_event")]
#[openapi(description = "Delete Calendar Event")]
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
#[openapi(description = "List Calendars")]
pub async fn list_calendars(
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<ListCalendarsResponse> {
    let body = get_calendars_list(&data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_calendars_list(cal_sync: &CalendarSync) -> HttpResult<StackString> {
    let mut calendars: Vec<_> = cal_sync
        .list_calendars()
        .await?
        .try_filter(|calendar| future::ready(calendar.sync))
        .try_collect()
        .await?;
    calendars.sort_by_key(|calendar| {
        calendar
            .gcal_name
            .as_ref()
            .map_or_else(|| calendar.name.clone(), Clone::clone)
    });
    let body = list_calendars_body(calendars)?.into();
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
#[openapi(description = "List Events")]
pub async fn list_events(
    query: Query<ListEventsRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<ListEventsResponse> {
    let query = query.into_inner();
    let body = get_events_list(query, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_events_list(
    query: ListEventsRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let calendars: Vec<_> = cal_sync.list_calendars().await?.try_collect().await?;
    let Some(calendar) = calendars
        .into_iter()
        .find(|cal| cal.name == query.calendar_name)
    else {
        return Ok("".into());
    };
    let min_time = query.min_time.map(Into::into);
    let max_time = query.max_time.map(Into::into);
    let mut events = cal_sync
        .list_events(&calendar.gcal_id, min_time, max_time)
        .await?;
    events.sort_by_key(|event| event.start_time);
    let body = list_events_body(calendar, events, cal_sync.config.clone())?.into();
    Ok(body)
}

#[derive(RwebResponse)]
#[response(description = "Event Details", content = "html", status = "CREATED")]
struct EventDetailResponse(HtmlBase<StackString, Error>);

#[get("/calendar/event_detail")]
#[openapi(description = "Get Calendar Event Detail")]
pub async fn event_detail(
    payload: Query<GcalEventID>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<EventDetailResponse> {
    let payload = payload.into_inner();
    let body = get_event_detail(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_event_detail(
    payload: GcalEventID,
    cal_sync: &CalendarSync,
) -> HttpResult<StackString> {
    let body = if let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&payload.gcal_id, &payload.event_id, &cal_sync.pool)
            .await?
    {
        let event: Event = event.into();
        event_detail_body(event, cal_sync.config.clone())?.into()
    } else {
        "".into()
    };
    Ok(body)
}

#[derive(Debug, Serialize, Deserialize, Schema)]
#[schema(component = "Pagination")]
struct Pagination {
    #[schema(description = "Number of Entries Returned")]
    limit: usize,
    #[schema(description = "Number of Entries to Skip")]
    offset: usize,
    #[schema(description = "Total Number of Entries")]
    total: usize,
}

#[derive(Debug, Serialize, Deserialize, Schema)]
#[schema(component = "PaginatedCalendarList")]
struct PaginatedCalendarList {
    pagination: Pagination,
    data: Vec<CalendarListWrapper>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar List")]
struct CalendarListResponse(JsonBase<PaginatedCalendarList, Error>);

#[get("/calendar/calendar_list")]
#[openapi(description = "List Calendars")]
pub async fn calendar_list(
    query: Query<MinModifiedQuery>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarListResponse> {
    let query = query.into_inner();
    let result = calendar_list_object(&query, &data.cal_sync).await?;
    Ok(JsonBase::new(result).into())
}

async fn calendar_list_object(
    query: &MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<PaginatedCalendarList> {
    let min_modified = query.min_modified.map(Into::into);
    let total = CalendarList::get_total(&cal_sync.pool, min_modified).await?;
    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);
    let pagination = Pagination {
        limit,
        offset,
        total,
    };
    let data = CalendarList::get_recent(&cal_sync.pool, min_modified, Some(offset), Some(limit))
        .await?
        .map_ok(Into::into)
        .try_collect()
        .await?;
    Ok(PaginatedCalendarList { pagination, data })
}

#[derive(Serialize, Deserialize, Schema)]
#[schema(component = "CalendarUpdateRequest")]
pub struct CalendarUpdateRequest {
    #[schema(description = "Calendar List Updates")]
    pub updates: Vec<CalendarListWrapper>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar List Update", status = "CREATED")]
struct CalendarListUpdateResponse(JsonBase<Vec<CalendarListWrapper>, Error>);

#[post("/calendar/calendar_list")]
#[openapi(description = "Update Calendars")]
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
    let futures: FuturesUnordered<_> = payload
        .updates
        .into_iter()
        .map(|calendar| {
            let pool = cal_sync.pool.clone();
            let calendar: CalendarList = calendar.into();
            async move {
                calendar.upsert(&pool).await?;
                Ok(calendar.into())
            }
        })
        .collect();
    futures.try_collect().await
}

#[derive(Debug, Serialize, Deserialize, Schema)]
#[schema(component = "PaginatedCalendarCache")]
struct PaginatedCalendarCache {
    pagination: Pagination,
    data: Vec<CalendarCacheWrapper>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar Cache")]
struct CalendarCacheResponse(JsonBase<PaginatedCalendarCache, Error>);

#[get("/calendar/calendar_cache")]
#[openapi(description = "List Recent Calendar Events")]
pub async fn calendar_cache(
    query: Query<MinModifiedQuery>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<CalendarCacheResponse> {
    let query = query.into_inner();
    let result = calendar_cache_events(&query, &data.cal_sync).await?;
    Ok(JsonBase::new(result).into())
}

async fn calendar_cache_events(
    query: &MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> HttpResult<PaginatedCalendarCache> {
    let min_modified = query.min_modified.map(Into::into);
    let total = CalendarCache::get_total(&cal_sync.pool, min_modified).await?;
    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);
    let pagination = Pagination {
        limit,
        offset,
        total,
    };
    let data = CalendarCache::get_recent(&cal_sync.pool, min_modified, Some(offset), Some(limit))
        .await?
        .map_ok(Into::into)
        .try_collect()
        .await?;
    Ok(PaginatedCalendarCache { pagination, data })
}

#[derive(Serialize, Deserialize, Schema)]
#[schema(component = "CalendarCacheUpdateRequest")]
pub struct CalendarCacheUpdateRequest {
    #[schema(description = "Calendar Events Update")]
    pub updates: Vec<CalendarCacheRequest>,
}

#[derive(RwebResponse)]
#[response(description = "Calendar Cache Update", status = "CREATED")]
struct CalendarCacheUpdateResponse(JsonBase<Vec<CalendarCacheWrapper>, Error>);

#[post("/calendar/calendar_cache")]
#[openapi(description = "Update Calendar Events")]
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
    let futures: FuturesUnordered<_> = payload
        .updates
        .into_iter()
        .map(|event| {
            let pool = cal_sync.pool.clone();
            let event: CalendarCache = event.into();
            async move {
                event.upsert(&pool).await?;
                Ok(event.into())
            }
        })
        .collect();
    futures.try_collect().await
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
#[openapi(description = "Get Full URL from Shortened URL")]
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
#[openapi(description = "Get Calendar Event Creation Form")]
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
    let body = build_event_body(event)?.into();
    Ok(body)
}

#[derive(RwebResponse)]
#[response(
    description = "Create Calendar Event",
    content = "html",
    status = "CREATED"
)]
struct CreateCalendarEventResponse(HtmlBase<String, Error>);

#[post("/calendar/create_calendar_event")]
#[openapi(description = "Create Calendar Event")]
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
    let Some(event) =
        CalendarCache::get_by_gcal_id_event_id(&event.gcal_id, &event.event_id, &cal_sync.pool)
            .await?
    else {
        return Err(Error::BadRequest("Failed to store event in db".into()));
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

#[post("/calendar/edit_calendar/{gcal_id}")]
#[openapi(description = "Edit Google Calendar Event")]
pub async fn edit_calendar(
    gcal_id: StackString,
    query: Json<EditCalendarRequest>,
    #[filter = "LoggedUser::filter"] _: LoggedUser,
    #[data] data: AppState,
) -> WarpResult<EditCalendarResponse> {
    let query = query.into_inner();
    let calendar_list = edit_calendar_list(&gcal_id, query, &data.cal_sync).await?;
    Ok(JsonBase::new(calendar_list).into())
}

async fn edit_calendar_list(
    gcal_id: &str,
    query: EditCalendarRequest,
    cal_sync: &CalendarSync,
) -> HttpResult<CalendarListWrapper> {
    let Some(mut calendar) = CalendarList::get_by_gcal_id(gcal_id, &cal_sync.pool).await? else {
        return Err(format_err!("No such calendar {gcal_id}").into());
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
