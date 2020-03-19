use actix_web::{
    http::StatusCode,
    web::{Data, Json, Path, Query},
    HttpResponse,
};
use chrono::NaiveDate;
use maplit::hashmap;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    sync::Arc,
};

use crate::app::AppState;
use crate::errors::ServiceError as Error;
use crate::logged_user::LoggedUser;

fn form_http_response(body: String) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(body))
}

pub async fn calendar_index(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}

pub async fn sync_calendars(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}

pub async fn sync_calendars_full(
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct DeleteEventPath {
    pub gcal_id: String,
    pub event_id: String,
}

pub async fn delete_event(
    query: Path<DeleteEventPath>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}

pub async fn list_calendars(_: LoggedUser, data: Data<AppState>) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct ListEventsRequest {
    pub gcal_id: String,
    pub min_time: Option<NaiveDate>,
    pub max_time: Option<NaiveDate>,
}

pub async fn list_events(
    query: Query<ListEventsRequest>,
    _: LoggedUser,
    data: Data<AppState>,
) -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}
