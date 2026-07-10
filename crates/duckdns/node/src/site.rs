//! Static DuckFS site serving over one already-authenticated web-plane stream.
//! Reads reuse noded's `ActorNodeApi` and the existing `duckfs_client::NodeApi`
//! contract; no `/v1/*` route is dialed or exposed.

use std::convert::Infallible;
use std::error::Error as StdError;

use bytes::Bytes;
use duckdns_client::DuckFsSite;
use duckfs_client::api::{ApiError, NodeApi};
use duckfs_core::{EntryInfo, EntryKindWire, MAX_READ_BYTES};
use futures::stream;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt as _, Empty, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH, LOCATION,
    X_CONTENT_TYPE_OPTIONS,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use percent_encoding::percent_decode_str;
use tokio::io::{AsyncRead, AsyncWrite};

type BodyError = Box<dyn StdError + Send + Sync>;
type Body = UnsyncBoxBody<Bytes, BodyError>;

pub async fn serve<S>(stream: S, api: noded::ActorNodeApi, site: DuckFsSite) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    http1::Builder::new()
        .keep_alive(true)
        .serve_connection(
            TokioIo::new(stream),
            service_fn(move |request| handle(request, api.clone(), site.clone())),
        )
        .await
        .map_err(|error| format!("serve DuckFS site: {error}"))
}

async fn handle(
    request: Request<Incoming>,
    api: noded::ActorNodeApi,
    site: DuckFsSite,
) -> Result<Response<Body>, Infallible> {
    let response = match *request.method() {
        Method::GET | Method::HEAD => serve_request(request, api, site).await,
        _ => response(StatusCode::METHOD_NOT_ALLOWED, "method not allowed\n"),
    };
    Ok(response)
}

async fn serve_request(
    request: Request<Incoming>,
    api: noded::ActorNodeApi,
    site: DuckFsSite,
) -> Response<Body> {
    let requested_path = match site_path(&site, request.uri().path()) {
        Ok(path) => path,
        Err(_) => return response(StatusCode::BAD_REQUEST, "invalid site path\n"),
    };
    // A "follow head" site follows it between requests, not between chunks of
    // one response. Pin the committed head once so stat, index lookup, and all
    // body pages see the same immutable tree and Content-Length/ETag stay true
    // even if another DuckFS commit lands while the response is streaming.
    let snapshot = match site.snapshot.clone() {
        Some(snapshot) => Some(snapshot),
        None => {
            let refs_api = api.clone();
            match tokio::task::spawn_blocking(move || refs_api.refs()).await {
                Ok(Ok(refs)) => match refs.head {
                    Some(head) => Some(head),
                    None => return response(StatusCode::NOT_FOUND, "site has no files\n"),
                },
                Ok(Err(_)) | Err(_) => {
                    return response(StatusCode::SERVICE_UNAVAILABLE, "DuckFS site unavailable\n");
                }
            }
        }
    };
    let lookup_api = api.clone();
    let lookup_path = requested_path.clone();
    let lookup_snapshot = snapshot.clone();
    let entry = match tokio::task::spawn_blocking(move || {
        lookup_api.stat(&lookup_path, lookup_snapshot.as_deref())
    })
    .await
    {
        Ok(Ok(Some(entry))) => entry,
        Ok(Ok(None)) | Ok(Err(ApiError::NotFound)) => {
            return response(StatusCode::NOT_FOUND, "site file not found\n");
        }
        Ok(Err(_)) | Err(_) => {
            return response(StatusCode::SERVICE_UNAVAILABLE, "DuckFS site unavailable\n");
        }
    };

    let (path, entry) = if entry.kind == EntryKindWire::Dir {
        if !request.uri().path().ends_with('/') {
            let mut location = request.uri().path().to_owned();
            location.push('/');
            if let Some(query) = request.uri().query() {
                location.push('?');
                location.push_str(query);
            }
            return Response::builder()
                .status(StatusCode::PERMANENT_REDIRECT)
                .header(LOCATION, location)
                .body(empty_body())
                .expect("static redirect response");
        }
        let index_path = format!("{}/{}", requested_path.trim_end_matches('/'), site.index);
        let index_api = api.clone();
        let index_snapshot = snapshot.clone();
        let index_lookup = index_path.clone();
        match tokio::task::spawn_blocking(move || {
            index_api.stat(&index_lookup, index_snapshot.as_deref())
        })
        .await
        {
            Ok(Ok(Some(index))) if index.kind == EntryKindWire::File => (index_path, index),
            Ok(Ok(_)) | Ok(Err(ApiError::NotFound)) => {
                return response(StatusCode::NOT_FOUND, "site index not found\n");
            }
            Ok(Err(_)) | Err(_) => {
                return response(StatusCode::SERVICE_UNAVAILABLE, "DuckFS site unavailable\n");
            }
        }
    } else if entry.kind == EntryKindWire::File {
        (requested_path, entry)
    } else {
        return response(StatusCode::NOT_FOUND, "site file not found\n");
    };

    let etag = format!("\"{}\"", entry.object);
    if request
        .headers()
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
    {
        return Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(ETAG, etag)
            .header(CACHE_CONTROL, cache_control(&site))
            .body(empty_body())
            .expect("static not-modified response");
    }

    let content_type = content_type(&entry, &path);
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, entry.size)
        .header(ETAG, etag)
        .header(CACHE_CONTROL, cache_control(&site))
        .header(X_CONTENT_TYPE_OPTIONS, "nosniff");
    if request.method() == Method::HEAD {
        return builder.body(empty_body()).expect("static HEAD response");
    }

    let body = file_body(api, path, snapshot);
    builder.body(body).expect("static file response")
}

fn site_path(site: &DuckFsSite, uri_path: &str) -> Result<String, String> {
    let decoded = percent_decode_str(uri_path)
        .decode_utf8()
        .map_err(|_| "site path is not UTF-8".to_string())?;
    if !decoded.starts_with('/') || decoded.contains('\\') {
        return Err("site path is not canonical".into());
    }
    let joined = if decoded == "/" {
        site.prefix.clone()
    } else {
        format!("{}{}", site.prefix.trim_end_matches('/'), decoded)
    };
    duckfs_core::paths::canonical(&joined)?;
    Ok(joined)
}

fn content_type(entry: &EntryInfo, path: &str) -> String {
    entry
        .meta
        .get("mime")
        .filter(|value| hyper::header::HeaderValue::from_str(value).is_ok())
        .cloned()
        .unwrap_or_else(|| {
            mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string()
        })
}

fn cache_control(site: &DuckFsSite) -> &'static str {
    if site.snapshot.is_some() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn file_body(api: noded::ActorNodeApi, path: String, snapshot: Option<String>) -> Body {
    struct ReadState {
        api: noded::ActorNodeApi,
        path: String,
        snapshot: Option<String>,
        offset: u64,
        done: bool,
    }

    let stream = stream::unfold(
        ReadState {
            api,
            path,
            snapshot,
            offset: 0,
            done: false,
        },
        |mut state| async move {
            if state.done {
                return None;
            }
            let api = state.api.clone();
            let path = state.path.clone();
            let snapshot = state.snapshot.clone();
            let offset = state.offset;
            let read = tokio::task::spawn_blocking(move || {
                api.read(&path, snapshot.as_deref(), offset, MAX_READ_BYTES)
            })
            .await;
            match read {
                Ok(Ok((bytes, eof))) if !bytes.is_empty() || eof => {
                    state.offset = state.offset.saturating_add(bytes.len() as u64);
                    state.done = eof;
                    Some((Ok(Frame::data(Bytes::from(bytes))), state))
                }
                Ok(Ok(_)) => Some((Err(body_error("DuckFS read made no progress")), state)),
                Ok(Err(error)) => Some((Err(Box::new(error) as BodyError), state)),
                Err(error) => Some((Err(Box::new(error) as BodyError), state)),
            }
        },
    );
    StreamBody::new(stream).boxed_unsync()
}

fn response(status: StatusCode, message: &'static str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(CONTENT_LENGTH, message.len())
        .body(full_body(message))
        .expect("static error response")
}

fn full_body(value: &'static str) -> Body {
    Full::new(Bytes::from_static(value.as_bytes()))
        .map_err(|never| match never {})
        .boxed_unsync()
}

fn empty_body() -> Body {
    Empty::new().map_err(|never| match never {}).boxed_unsync()
}

fn body_error(message: &'static str) -> BodyError {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> DuckFsSite {
        DuckFsSite {
            prefix: "/shared/sites/docs".into(),
            snapshot: None,
            index: "index.html".into(),
        }
    }

    #[test]
    fn maps_url_paths_under_the_declared_duckfs_prefix() {
        assert_eq!(
            site_path(&site(), "/assets/app.css").unwrap(),
            "/shared/sites/docs/assets/app.css"
        );
        assert_eq!(site_path(&site(), "/").unwrap(), "/shared/sites/docs");
        assert!(site_path(&site(), "/../secrets").is_err());
        assert!(site_path(&site(), "/%2e%2e/secrets").is_err());
        assert!(site_path(&site(), "/assets%2f..%2fsecrets").is_err());
    }
}
