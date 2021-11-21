FROM rust:alpine3.14 as buildstage

RUN apk add musl-dev && mkdir -p /build/calendar_app_rust

ADD Cargo.toml /build/calendar_app_rust/
COPY src /build/calendar_app_rust/src
COPY calendar_app_http /build/calendar_app_rust/calendar_app_http
COPY calendar_app_bot /build/calendar_app_rust/calendar_app_bot
COPY calendar_app_lib /build/calendar_app_rust/calendar_app_lib
COPY gcal_lib /build/calendar_app_rust/gcal_lib
COPY migrations /build/calendar_app_rust/migrations
COPY templates /build/calendar_app_rust/templates

RUN mkdir -p /calendar_app_rust && \
    cd /build/calendar_app_rust && \
    cargo install --path=. --root=/calendar_app_rust

FROM alpine:3.14

COPY --from=buildstage /calendar_app_rust/bin/* /usr/bin/

STOPSIGNAL SIGTERM

EXPOSE 3096

CMD ["/usr/bin/aws-app-http"]
