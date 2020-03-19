use actix_web::{
    http::StatusCode,
    web::{Data, Json, Query},
    HttpResponse,
};
use maplit::hashmap;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

use crate::errors::ServiceError as Error;

fn form_http_response(body: String) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(body))
}

pub async fn calendar_index() -> Result<HttpResponse, Error> {
    form_http_response("nothing".to_string())
}
