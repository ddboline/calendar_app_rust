use anyhow::format_err;
use axum::extract::{Json, Path, Query, State};
use derive_more::{From, Into};
use futures::{TryStreamExt, future, stream::FuturesUnordered};
use serde::{Deserialize, Serialize};
use stack_string::{StackString, format_sstr};
use std::{collections::HashMap, sync::Arc};
use time::{Date, OffsetDateTime};
use time_tz::OffsetDateTimeExt;
use utoipa::{OpenApi, PartialSchema, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_helper::{
    UtoipaResponse, html_response::HtmlResponse as HtmlBase,
    json_response::JsonResponse as JsonBase,
};

use calendar_app_lib::{
    calendar::Event,
    calendar_sync::CalendarSync,
    models::{CalendarCache, CalendarList, ShortenedLinks},
    timezone::TimeZone,
};

use crate::{
    CalendarCacheRequest, CalendarCacheWrapper, CalendarListWrapper, CreateCalendarEventRequest,
    MinModifiedQuery,
    app::{AppState, UrlCache},
    elements::{
        agenda_body, build_event_body, event_detail_body, index_body, list_calendars_body,
        list_events_body,
    },
    errors::ServiceError as Error,
    logged_user::LoggedUser,
};

type WarpResult<T> = Result<T, Error>;

#[derive(UtoipaResponse)]
#[response(description = "Main Page", content = "text/html")]
#[rustfmt::skip]
struct IndexResponse(HtmlBase::<String>);

#[utoipa::path(get, path = "/calendar/index.html", responses(IndexResponse, Error))]
// Calendar App Main Page")]
async fn calendar_index(_: LoggedUser) -> WarpResult<IndexResponse> {
    let body = index_body()?;
    Ok(HtmlBase::new(body).into())
}

#[derive(UtoipaResponse)]
#[response(description = "Agenda", content = "text/html")]
#[rustfmt::skip]
struct AgendaResponse(HtmlBase::<StackString>);

#[utoipa::path(get, path = "/calendar/agenda", responses(AgendaResponse, Error))]
// Calendar Agenda Page")]
async fn agenda(_: LoggedUser, data: State<Arc<AppState>>) -> WarpResult<AgendaResponse> {
    let body = get_agenda(&data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_agenda(cal_sync: &CalendarSync) -> WarpResult<StackString> {
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

#[derive(UtoipaResponse)]
#[response(description = "Sync Output", content = "text/html")]
#[rustfmt::skip]
struct SyncResponse(HtmlBase::<String>);

#[utoipa::path(
    post,
    path = "/calendar/sync_calendars",
    responses(SyncResponse, Error)
)]
// Sync Calendars")]
async fn sync_calendars(_: LoggedUser, data: State<Arc<AppState>>) -> WarpResult<SyncResponse> {
    let body = sync_calendars_body(&data.cal_sync, false).await?;
    Ok(HtmlBase::new(body).into())
}

async fn sync_calendars_body(cal_sync: &CalendarSync, do_full: bool) -> WarpResult<String> {
    Ok(cal_sync.run_syncing(do_full).await?.join("<br>"))
}

#[utoipa::path(
    post,
    path = "/calendar/sync_calendars_full",
    responses(SyncResponse, Error)
)]
// Fully Sync All Calendars")]
async fn sync_calendars_full(
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<SyncResponse> {
    let body = sync_calendars_body(&data.cal_sync, true).await?;
    Ok(HtmlBase::new(body).into())
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
// GcalEventID")]
struct GcalEventID {
    // GCal ID")]
    gcal_id: StackString,
    // GCal Event ID")]
    event_id: StackString,
}

#[derive(UtoipaResponse)]
#[response(
    description = "Delete Event Output",
    content = "text/html",
    status = "NO_CONTENT"
)]
#[rustfmt::skip]
struct DeleteEventResponse(HtmlBase::<StackString>);

#[utoipa::path(
    delete,
    path = "/calendar/delete_event",
    responses(DeleteEventResponse, Error)
)]
// Delete Calendar Event")]
async fn delete_event(
    data: State<Arc<AppState>>,
    _: LoggedUser,
    payload: Json<GcalEventID>,
) -> WarpResult<DeleteEventResponse> {
    let Json(payload) = payload;
    let body = delete_event_body(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn delete_event_body(
    payload: GcalEventID,
    cal_sync: &CalendarSync,
) -> WarpResult<StackString> {
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

#[derive(UtoipaResponse)]
#[response(description = "List Calendars", content = "text/html")]
#[rustfmt::skip]
struct ListCalendarsResponse(HtmlBase::<StackString>);

#[utoipa::path(
    get,
    path = "/calendar/list_calendars",
    responses(ListCalendarsResponse, Error)
)]
// List Calendars")]
async fn list_calendars(
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<ListCalendarsResponse> {
    let body = get_calendars_list(&data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_calendars_list(cal_sync: &CalendarSync) -> WarpResult<StackString> {
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

#[derive(Serialize, Deserialize, ToSchema)]
struct ListEventsRequest {
    // Calendar Name")]
    calendar_name: StackString,
    // Earliest Date")]
    min_time: Option<Date>,
    // Latest Date")]
    max_time: Option<Date>,
}

#[derive(UtoipaResponse)]
#[response(description = "List Events", content = "text/html")]
#[rustfmt::skip]
struct ListEventsResponse(HtmlBase::<StackString>);

#[utoipa::path(
    get,
    path = "/calendar/list_events",
    responses(ListEventsResponse, Error)
)]
// List Events")]
async fn list_events(
    query: Query<ListEventsRequest>,
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<ListEventsResponse> {
    let Query(query) = query;
    let body = get_events_list(query, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_events_list(
    query: ListEventsRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<StackString> {
    let calendars: Vec<_> = cal_sync.list_calendars().await?.try_collect().await?;
    let Some(calendar) = calendars
        .into_iter()
        .find(|cal| cal.name == query.calendar_name)
    else {
        return Ok("".into());
    };
    let min_time = query.min_time;
    let max_time = query.max_time;
    let mut events = cal_sync
        .list_events(&calendar.gcal_id, min_time, max_time)
        .await?;
    events.sort_by_key(|event| event.start_time);
    let body = list_events_body(calendar, events, cal_sync.config.clone())?.into();
    Ok(body)
}

#[derive(UtoipaResponse)]
#[response(
    description = "Event Details",
    content = "text/html",
    status = "CREATED"
)]
#[rustfmt::skip]
struct EventDetailResponse(HtmlBase::<StackString>);

#[utoipa::path(
    get,
    path = "/calendar/event_detail",
    responses(EventDetailResponse, Error)
)]
// Get Calendar Event Detail")]
async fn event_detail(
    payload: Query<GcalEventID>,
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<EventDetailResponse> {
    let Query(payload) = payload;
    let body = get_event_detail(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn get_event_detail(
    payload: GcalEventID,
    cal_sync: &CalendarSync,
) -> WarpResult<StackString> {
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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
// Pagination")]
struct Pagination {
    // Number of Entries Returned")]
    limit: usize,
    // Number of Entries to Skip")]
    offset: usize,
    // Total Number of Entries")]
    total: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
// PaginatedCalendarList")]
struct PaginatedCalendarList {
    pagination: Pagination,
    data: Vec<CalendarListWrapper>,
}

#[derive(UtoipaResponse)]
#[response(description = "Calendar List")]
#[rustfmt::skip]
struct CalendarListResponse(JsonBase::<PaginatedCalendarList>);

#[utoipa::path(
    get,
    path = "/calendar/calendar_list",
    responses(CalendarListResponse, Error)
)]
// List Calendars")]
async fn calendar_list(
    query: Query<MinModifiedQuery>,
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<CalendarListResponse> {
    let Query(query) = query;
    let result = calendar_list_object(&query, &data.cal_sync).await?;
    Ok(JsonBase::new(result).into())
}

async fn calendar_list_object(
    query: &MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> WarpResult<PaginatedCalendarList> {
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

#[derive(Serialize, Deserialize, ToSchema)]
// CalendarUpdateRequest")]
struct CalendarUpdateRequest {
    // Calendar List Updates")]
    updates: Vec<CalendarListWrapper>,
}

#[derive(Serialize, ToSchema, Into, From)]
struct CalendarListInner(Vec<CalendarListWrapper>);

#[derive(UtoipaResponse)]
#[response(description = "Calendar List Update", status = "CREATED")]
#[rustfmt::skip]
struct CalendarListUpdateResponse(JsonBase::<CalendarListInner>);

#[utoipa::path(
    post,
    path = "/calendar/calendar_list",
    responses(CalendarListUpdateResponse, Error)
)]
// Update Calendars")]
async fn calendar_list_update(
    data: State<Arc<AppState>>,
    _: LoggedUser,
    payload: Json<CalendarUpdateRequest>,
) -> WarpResult<CalendarListUpdateResponse> {
    let Json(payload) = payload;
    let calendars = calendar_list_update_object(payload, &data.cal_sync).await?;
    Ok(JsonBase::new(calendars.into()).into())
}

async fn calendar_list_update_object(
    payload: CalendarUpdateRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<Vec<CalendarListWrapper>> {
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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
// PaginatedCalendarCache")]
struct PaginatedCalendarCache {
    pagination: Pagination,
    data: Vec<CalendarCacheWrapper>,
}

#[derive(UtoipaResponse)]
#[response(description = "Calendar Cache")]
#[rustfmt::skip]
struct CalendarCacheResponse(JsonBase::<PaginatedCalendarCache>);

#[utoipa::path(
    get,
    path = "/calendar/calendar_cache",
    responses(CalendarCacheResponse, Error)
)]
// List Recent Calendar Events")]
async fn calendar_cache(
    query: Query<MinModifiedQuery>,
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<CalendarCacheResponse> {
    let Query(query) = query;
    let result = calendar_cache_events(&query, &data.cal_sync).await?;
    Ok(JsonBase::new(result).into())
}

async fn calendar_cache_events(
    query: &MinModifiedQuery,
    cal_sync: &CalendarSync,
) -> WarpResult<PaginatedCalendarCache> {
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

#[derive(Serialize, Deserialize, ToSchema)]
// CalendarCacheUpdateRequest")]
struct CalendarCacheUpdateRequest {
    // Calendar Events Update")]
    updates: Vec<CalendarCacheRequest>,
}

#[derive(Serialize, ToSchema, Into, From)]
struct CalendarCacheInner(Vec<CalendarCacheWrapper>);

#[derive(UtoipaResponse)]
#[response(description = "Calendar Cache Update", status = "CREATED")]
#[rustfmt::skip]
struct CalendarCacheUpdateResponse(JsonBase::<CalendarCacheInner>);

#[utoipa::path(
    post,
    path = "/calendar/calendar_cache",
    responses(CalendarCacheUpdateResponse, Error)
)]
// Update Calendar Events")]
async fn calendar_cache_update(
    data: State<Arc<AppState>>,
    _: LoggedUser,
    payload: Json<CalendarCacheUpdateRequest>,
) -> WarpResult<CalendarCacheUpdateResponse> {
    let Json(payload) = payload;
    let events = calendar_cache_update_events(payload, &data.cal_sync).await?;
    Ok(JsonBase::new(events.into()).into())
}

async fn calendar_cache_update_events(
    payload: CalendarCacheUpdateRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<Vec<CalendarCacheWrapper>> {
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

#[derive(UtoipaResponse)]
#[response(description = "Logged in User")]
#[rustfmt::skip]
struct UserResponse(JsonBase::<LoggedUser>);

#[utoipa::path(get, path = "/calendar/user", responses(UserResponse, Error))]
async fn user(user: LoggedUser) -> WarpResult<UserResponse> {
    Ok(JsonBase::new(user).into())
}

#[derive(UtoipaResponse)]
#[response(description = "Shortened Link", content = "text/html")]
#[rustfmt::skip]
struct ShortenedLinkResponse(HtmlBase::<StackString>);

#[utoipa::path(
    get,
    path = "/calendar/link/{link}",
    responses(ShortenedLinkResponse, Error)
)]
// Get Full URL from Shortened URL")]
async fn link_shortener(
    data: State<Arc<AppState>>,
    link: Path<StackString>,
) -> WarpResult<ShortenedLinkResponse> {
    let Path(link) = link;
    let body = link_shortener_body(&link, &data.cal_sync, &data.shortened_urls).await?;
    Ok(HtmlBase::new(body).into())
}

async fn link_shortener_body(
    link: &str,
    cal_sync: &CalendarSync,
    shortened_urls: &UrlCache,
) -> WarpResult<StackString> {
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

#[derive(Serialize, Deserialize, Debug, ToSchema)]
struct BuildEventRequest {
    // GCal Calendar ID")]
    gcal_id: StackString,
    // Event ID")]
    event_id: Option<StackString>,
}

#[derive(UtoipaResponse)]
#[response(description = "Build Calendar Event", content = "text/html")]
#[rustfmt::skip]
struct BuildCalendarEventResponse(HtmlBase::<StackString>);

#[utoipa::path(
    get,
    path = "/calendar/create_calendar_event",
    responses(BuildCalendarEventResponse, Error)
)]
// Get Calendar Event Creation Form")]
async fn build_calendar_event(
    query: Query<BuildEventRequest>,
    _: LoggedUser,
    data: State<Arc<AppState>>,
) -> WarpResult<BuildCalendarEventResponse> {
    let Query(query) = query;
    let body = build_calendar_event_body(query, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn build_calendar_event_body(
    query: BuildEventRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<StackString> {
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

#[derive(UtoipaResponse)]
#[response(
    description = "Create Calendar Event",
    content = "text/html",
    status = "CREATED"
)]
#[rustfmt::skip]
struct CreateCalendarEventResponse(HtmlBase::<String>);

#[utoipa::path(
    post,
    path = "/calendar/create_calendar_event",
    responses(CreateCalendarEventResponse, Error)
)]
// Create Calendar Event")]
async fn create_calendar_event(
    data: State<Arc<AppState>>,
    _: LoggedUser,
    payload: Json<CreateCalendarEventRequest>,
) -> WarpResult<CreateCalendarEventResponse> {
    let Json(payload) = payload;
    let body = create_calendar_event_body(payload, &data.cal_sync).await?;
    Ok(HtmlBase::new(body).into())
}

async fn create_calendar_event_body(
    payload: CreateCalendarEventRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<String> {
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
        event_location_name: payload.event_location_name,
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

#[derive(Serialize, Deserialize, ToSchema)]
struct EditCalendarRequest {
    // Calendar Name")]
    calendar_name: Option<StackString>,
    // Sync Flag")]
    sync: Option<bool>,
    // Edit Flag")]
    edit: Option<bool>,
    // Display Flag")]
    display: Option<bool>,
}

#[derive(UtoipaResponse)]
#[response(description = "Edit Calendar Event")]
#[rustfmt::skip]
struct EditCalendarResponse(JsonBase::<CalendarListWrapper>);

#[utoipa::path(
    post,
    path = "/calendar/edit_calendar/{gcal_id}",
    responses(EditCalendarResponse, Error)
)]
// Edit Google Calendar Event")]
async fn edit_calendar(
    data: State<Arc<AppState>>,
    gcal_id: Path<StackString>,
    _: LoggedUser,
    query: Json<EditCalendarRequest>,
) -> WarpResult<EditCalendarResponse> {
    let Json(query) = query;
    let Path(gcal_id) = gcal_id;
    let calendar_list = edit_calendar_list(&gcal_id, query, &data.cal_sync).await?;
    Ok(JsonBase::new(calendar_list).into())
}

async fn edit_calendar_list(
    gcal_id: &str,
    query: EditCalendarRequest,
    cal_sync: &CalendarSync,
) -> WarpResult<CalendarListWrapper> {
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

pub fn get_calendar_path(app: &AppState) -> OpenApiRouter {
    let app = Arc::new(app.clone());

    OpenApiRouter::new()
        .routes(routes!(calendar_index))
        .routes(routes!(agenda))
        .routes(routes!(sync_calendars))
        .routes(routes!(sync_calendars_full))
        .routes(routes!(delete_event))
        .routes(routes!(list_calendars))
        .routes(routes!(list_events))
        .routes(routes!(event_detail))
        .routes(routes!(calendar_list))
        .routes(routes!(calendar_list_update))
        .routes(routes!(calendar_cache))
        .routes(routes!(calendar_cache_update))
        .routes(routes!(user))
        .routes(routes!(link_shortener))
        .routes(routes!(build_calendar_event))
        .routes(routes!(create_calendar_event))
        .routes(routes!(edit_calendar))
        .with_state(app)
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Calendar Web App",
        description = "Web App to Display Calendar, Sync with GCal",
    ),
    components(schemas(LoggedUser, CalendarCacheWrapper, Pagination))
)]
pub struct ApiDoc;
