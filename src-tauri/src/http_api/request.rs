//! Reading one HTTP/1.1 request and writing its response. Pure: the guards (only local tools, the
//! token), the body's limits and the parameters.

use serde_json::{Value, json};

use crate::automation::{AutomationError, Outcome, Params};

const MAX_BODY_BYTES: usize = 16 * 1024;
pub const MAX_HEAD_BYTES: usize = 8 * 1024;

/// A request line and its headers (names lowercased).
#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Chunked bodies are refused: a body comes with its length.
    pub fn content_length(&self) -> Result<usize, AutomationError> {
        if self
            .header("transfer-encoding")
            .is_some_and(|v| !v.eq_ignore_ascii_case("identity"))
        {
            return Err(AutomationError::new(
                411,
                "Send the body with a Content-Length.",
            ));
        }
        let length = match self.header("content-length") {
            None => 0,
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| AutomationError::bad("Content-Length isn't a number."))?,
        };
        if length > MAX_BODY_BYTES {
            return Err(AutomationError::new(413, "The request body is too large."));
        }
        Ok(length)
    }
}

/// Where the head ends (after the first blank line); `None` while it is incomplete.
pub fn head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

pub fn parse_head(raw: &[u8]) -> Result<Head, AutomationError> {
    let text =
        std::str::from_utf8(raw).map_err(|_| AutomationError::bad("The request isn't UTF-8."))?;
    let mut lines = text.split("\r\n");
    let mut line = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target)) = (line.next(), line.next()) else {
        return Err(AutomationError::bad("Malformed request line."));
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Ok(Head {
        method: method.into(),
        path: path.into(),
        query: query.into(),
        headers,
    })
}

/// Timing-safe, so the token can't be guessed byte by byte; an empty token never matches.
pub fn token_matches(given: Option<&str>, expected: &str) -> bool {
    let Some(given) = given else { return false };
    if expected.is_empty() || given.len() != expected.len() {
        return false;
    }
    given
        .bytes()
        .zip(expected.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Only `127.0.0.1` or `localhost`, on our port when one is given.
fn local_host(host: &str, port: u16) -> bool {
    let (name, given) = match host.rsplit_once(':') {
        Some((name, given)) => (name, Some(given)),
        None => (host, None),
    };
    (name == "127.0.0.1" || name == "localhost")
        && given.is_none_or(|given| given.parse::<u16>().is_ok_and(|given| given == port))
}

fn given_token(head: &Head) -> Option<String> {
    let bearer = head.header("authorization").and_then(|v| {
        let (scheme, value) = v.split_once(' ')?;
        scheme
            .eq_ignore_ascii_case("bearer")
            .then(|| value.trim().to_string())
    });
    bearer
        .or_else(|| head.header("x-daybook-token").map(String::from))
        .or_else(|| head.header("x-companion-token").map(String::from))
        .or_else(|| {
            url::form_urlencoded::parse(head.query.as_bytes())
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.into_owned())
        })
}

/// The action, once the request passed the checks that need no body. A browser page (Origin) or a
/// foreign Host (DNS rebinding) is refused before anything else.
pub fn check(head: &Head, token: &str, port: u16) -> Result<String, AutomationError> {
    let host = head.header("host").unwrap_or_default();
    if head.header("origin").is_some() || !local_host(host, port) {
        return Err(AutomationError::new(
            403,
            "Only local tools may use this API.",
        ));
    }
    if head.method != "GET" && head.method != "POST" {
        return Err(AutomationError::new(405, "Use GET or POST."));
    }
    let action = head
        .path
        .strip_prefix("/v1/")
        .map(|rest| rest.strip_suffix('/').unwrap_or(rest))
        .filter(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_lowercase()))
        .ok_or_else(|| AutomationError::new(404, "Use /v1/<action>, e.g. /v1/state."))?;
    if !token_matches(given_token(head).as_deref(), token) {
        return Err(AutomationError::new(401, "Wrong or missing token."));
    }
    Ok(action.to_string())
}

/// The query, then a POST body: a JSON object of primitives (other values are ignored) or a form;
/// without the token, which may come as `?token=`.
pub fn params(head: &Head, body: &[u8]) -> Result<Params, AutomationError> {
    let mut params: Params = url::form_urlencoded::parse(head.query.as_bytes())
        .into_owned()
        .collect();
    params.extend(body_params(head, body)?);
    params.remove("token");
    Ok(params)
}

/// A POST body's parameters: JSON when it says so or looks like it, else a form.
fn body_params(head: &Head, body: &[u8]) -> Result<Params, AutomationError> {
    let body = std::str::from_utf8(body)
        .map_err(|_| AutomationError::bad("The body isn't UTF-8."))?
        .trim();
    if head.method != "POST" || body.is_empty() {
        return Ok(Params::new());
    }
    let json = head
        .header("content-type")
        .is_some_and(|t| t.contains("application/json"))
        || body.starts_with('{');
    if json {
        json_params(body)
    } else {
        Ok(form_params(body))
    }
}

/// A JSON object's strings, numbers and booleans as text; other values are ignored.
fn json_params(body: &str) -> Result<Params, AutomationError> {
    let Value::Object(object) = serde_json::from_str(body)
        .map_err(|_| AutomationError::bad("The body isn't valid JSON."))?
    else {
        return Err(AutomationError::bad("The body must be a JSON object."));
    };
    Ok(object
        .into_iter()
        .filter_map(|(key, value)| match value {
            Value::String(s) => Some((key, s)),
            Value::Number(n) => Some((key, n.to_string())),
            Value::Bool(b) => Some((key, b.to_string())),
            _ => None,
        })
        .collect())
}

fn form_params(body: &str) -> Params {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        422 => "Unprocessable Content",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

/// `{ ok: true, ...outcome }` or `{ ok: false, error }` with the error's status.
pub fn response(result: Result<Outcome, AutomationError>) -> Vec<u8> {
    let (status, body) = match result {
        Ok(outcome) => {
            let mut body = Outcome::new();
            body.insert("ok".into(), json!(true));
            body.extend(outcome);
            (200, Value::Object(body))
        }
        Err(e) => (e.status, json!({ "ok": false, "error": e.message })),
    };
    let body = format!("{body}\n");
    format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        reason(status),
        body.len()
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "s3cret";
    const PORT: u16 = 27180;

    fn head(method: &str, target: &str, headers: &[&str]) -> Head {
        let mut raw = format!("{method} {target} HTTP/1.1\r\n");
        for header in headers {
            raw.push_str(header);
            raw.push_str("\r\n");
        }
        raw.push_str("\r\n");
        parse_head(raw.as_bytes()).unwrap()
    }

    fn local(method: &str, target: &str, extra: &[&str]) -> Head {
        let mut headers = vec!["Host: 127.0.0.1:27180"];
        headers.extend_from_slice(extra);
        head(method, target, &headers)
    }

    fn rejected(head: &Head) -> (u16, String) {
        let e = check(head, TOKEN, PORT).unwrap_err();
        (e.status, e.message)
    }

    fn params_of(head: &Head, body: &str) -> Result<Params, AutomationError> {
        params(head, body.as_bytes())
    }

    fn pairs(items: &[(&str, &str)]) -> Params {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn answer(bytes: Vec<u8>) -> (String, serde_json::Value) {
        let text = String::from_utf8(bytes).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        (head.to_string(), serde_json::from_str(body).unwrap())
    }

    #[test]
    fn a_request_with_the_bearer_token_yields_its_action() {
        let head = local("GET", "/v1/state", &["Authorization: Bearer s3cret"]);

        assert_eq!(check(&head, TOKEN, PORT), Ok("state".to_string()));
    }

    #[test]
    fn the_token_is_read_from_every_supported_place() {
        for (extra, target) in [
            ("Authorization: bearer s3cret", "/v1/state"),
            ("X-Daybook-Token: s3cret", "/v1/state"),
            ("X-Companion-Token: s3cret", "/v1/state"),
            ("Accept: */*", "/v1/state?token=s3cret"),
        ] {
            let head = local("GET", target, &[extra]);

            assert_eq!(
                check(&head, TOKEN, PORT),
                Ok("state".to_string()),
                "{extra} {target}"
            );
        }
    }

    #[test]
    fn a_missing_or_wrong_token_is_unauthorized() {
        for (extra, target) in [
            ("Accept: */*", "/v1/state"),
            ("Authorization: Bearer nope", "/v1/state"),
            ("Authorization: Basic s3cret", "/v1/state"),
            ("X-Daybook-Token: s3cre", "/v1/state"),
            ("Accept: */*", "/v1/state?token=nope"),
        ] {
            let head = local("GET", target, &[extra]);

            assert_eq!(
                rejected(&head),
                (401, "Wrong or missing token.".to_string()),
                "{extra} {target}"
            );
        }
    }

    #[test]
    fn an_empty_configured_token_never_matches() {
        let head = local("GET", "/v1/state?token=", &["X-Daybook-Token: "]);

        assert_eq!(check(&head, "", PORT).unwrap_err().status, 401);
        assert!(!token_matches(Some(""), ""));
        assert!(!token_matches(None, "s3cret"));
        assert!(token_matches(Some("s3cret"), "s3cret"));
        assert!(!token_matches(Some("s3creT"), "s3cret"));
    }

    #[test]
    fn a_request_with_an_origin_is_forbidden_even_with_the_token() {
        let head = local(
            "GET",
            "/v1/state",
            &[
                "Authorization: Bearer s3cret",
                "Origin: https://evil.example",
            ],
        );

        assert_eq!(
            rejected(&head),
            (403, "Only local tools may use this API.".to_string())
        );
    }

    #[test]
    fn only_local_hosts_on_our_port_are_accepted() {
        for (host, accepted) in [
            ("127.0.0.1:27180", true),
            ("localhost:27180", true),
            ("127.0.0.1", true),
            ("localhost", true),
            ("127.0.0.1:27181", false),
            ("localhost:abc", false),
            ("evil.example:27180", false),
            ("evil.example", false),
            ("[::1]:27180", false),
            ("[::1]", false),
            ("", false),
        ] {
            let host_header = format!("Host: {host}");
            let head = head(
                "GET",
                "/v1/state",
                &[host_header.as_str(), "Authorization: Bearer s3cret"],
            );

            let outcome = check(&head, TOKEN, PORT);

            if accepted {
                assert_eq!(outcome, Ok("state".to_string()), "{host}");
            } else {
                assert_eq!(outcome.unwrap_err().status, 403, "{host}");
            }
        }
    }

    #[test]
    fn a_request_without_a_host_is_forbidden() {
        let head = head("GET", "/v1/state", &["Authorization: Bearer s3cret"]);

        assert_eq!(rejected(&head).0, 403);
    }

    #[test]
    fn only_get_and_post_are_allowed() {
        for method in ["GET", "POST"] {
            let head = local(method, "/v1/state", &["Authorization: Bearer s3cret"]);

            assert!(check(&head, TOKEN, PORT).is_ok(), "{method}");
        }
        for method in ["PUT", "DELETE", "OPTIONS", "HEAD"] {
            let head = local(method, "/v1/state", &["Authorization: Bearer s3cret"]);

            assert_eq!(
                rejected(&head),
                (405, "Use GET or POST.".to_string()),
                "{method}"
            );
        }
    }

    #[test]
    fn a_path_outside_v1_actions_is_not_found() {
        for target in [
            "/",
            "/v1",
            "/v1/",
            "/v2/state",
            "/v1/State",
            "/v1/state/extra",
            "/v1/sta-te",
            "/state",
        ] {
            let head = local("GET", target, &["Authorization: Bearer s3cret"]);

            assert_eq!(
                rejected(&head),
                (404, "Use /v1/<action>, e.g. /v1/state.".to_string()),
                "{target}"
            );
        }
    }

    #[test]
    fn a_trailing_slash_is_allowed() {
        let head = local("GET", "/v1/status/", &["Authorization: Bearer s3cret"]);

        assert_eq!(check(&head, TOKEN, PORT), Ok("status".to_string()));
    }

    #[test]
    fn the_origin_is_checked_before_the_method_and_the_path() {
        let head = local("PUT", "/nope", &["Origin: http://localhost"]);

        assert_eq!(rejected(&head).0, 403);
    }

    #[test]
    fn the_path_is_checked_before_the_token() {
        let head = local("GET", "/nope", &[]);

        assert_eq!(rejected(&head).0, 404);
    }

    #[test]
    fn header_names_are_read_without_regard_to_case() {
        let head = head(
            "GET",
            "/v1/state",
            &["HOST: 127.0.0.1:27180", "x-daybook-token: s3cret"],
        );

        assert_eq!(check(&head, TOKEN, PORT), Ok("state".to_string()));
    }

    #[test]
    fn the_head_splits_the_target_into_path_and_query() {
        let head = head("POST", "/v1/status?status=break&note=a+b", &["Host: x"]);

        assert_eq!(
            head,
            Head {
                method: "POST".into(),
                path: "/v1/status".into(),
                query: "status=break&note=a+b".into(),
                headers: vec![("host".into(), "x".into())],
            }
        );
    }

    #[test]
    fn a_request_line_without_a_target_is_malformed() {
        let e = parse_head(b"GET\r\n\r\n").unwrap_err();

        assert_eq!(e, AutomationError::bad("Malformed request line."));
    }

    #[test]
    fn a_head_that_is_not_utf8_is_a_bad_request() {
        let e = parse_head(b"GET /\xff HTTP/1.1\r\n\r\n").unwrap_err();

        assert_eq!(e, AutomationError::bad("The request isn't UTF-8."));
    }

    #[test]
    fn the_head_ends_after_the_first_blank_line() {
        assert_eq!(head_end(b"GET / HTTP/1.1\r\nA: b\r\n"), None);
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\nbody"), Some(18));
    }

    #[test]
    fn a_request_without_a_body_has_length_zero() {
        assert_eq!(local("GET", "/v1/state", &[]).content_length(), Ok(0));
    }

    #[test]
    fn the_content_length_is_read() {
        let head = local("POST", "/v1/status", &["Content-Length: 12"]);

        assert_eq!(head.content_length(), Ok(12));
    }

    #[test]
    fn a_body_of_exactly_the_limit_is_accepted_and_one_more_byte_is_too_large() {
        let at = local("POST", "/v1/x", &["Content-Length: 16384"]);
        let over = local("POST", "/v1/x", &["Content-Length: 16385"]);

        assert_eq!(at.content_length(), Ok(MAX_BODY_BYTES));
        assert_eq!(
            over.content_length(),
            Err(AutomationError::new(413, "The request body is too large."))
        );
    }

    #[test]
    fn a_chunked_body_needs_a_length_instead() {
        let head = local("POST", "/v1/x", &["Transfer-Encoding: chunked"]);

        assert_eq!(
            head.content_length(),
            Err(AutomationError::new(
                411,
                "Send the body with a Content-Length."
            ))
        );
    }

    #[test]
    fn the_identity_transfer_encoding_is_not_chunked() {
        let head = local(
            "POST",
            "/v1/x",
            &["Transfer-Encoding: Identity", "Content-Length: 3"],
        );

        assert_eq!(head.content_length(), Ok(3));
    }

    #[test]
    fn a_content_length_that_is_not_a_number_is_a_bad_request() {
        for value in ["abc", "-1", "1.5", ""] {
            let length = format!("Content-Length: {value}");
            let head = local("POST", "/v1/x", &[length.as_str()]);

            assert_eq!(
                head.content_length(),
                Err(AutomationError::bad("Content-Length isn't a number.")),
                "{value:?}"
            );
        }
    }

    #[test]
    fn the_query_is_decoded_into_params() {
        let head = local("GET", "/v1/status?status=break&note=out%20for+lunch", &[]);

        assert_eq!(
            params_of(&head, ""),
            Ok(pairs(&[("status", "break"), ("note", "out for lunch")]))
        );
    }

    #[test]
    fn a_form_body_adds_to_the_query() {
        let head = local(
            "POST",
            "/v1/status?status=off",
            &["Content-Type: application/x-www-form-urlencoded"],
        );

        assert_eq!(
            params_of(&head, "note=Lunch+time&status=break\n"),
            Ok(pairs(&[("status", "break"), ("note", "Lunch time")]))
        );
    }

    #[test]
    fn a_json_body_keeps_strings_numbers_and_booleans_as_text() {
        let head = local("POST", "/v1/x", &["Content-Type: application/json"]);

        assert_eq!(
            params_of(
                &head,
                r#"{"a":"text","n":5,"f":1.5,"yes":true,"no":false,"nothing":null,"list":[1],"obj":{"k":1}}"#
            ),
            Ok(pairs(&[
                ("a", "text"),
                ("n", "5"),
                ("f", "1.5"),
                ("yes", "true"),
                ("no", "false"),
            ]))
        );
    }

    #[test]
    fn a_body_starting_with_a_brace_is_json_without_a_content_type() {
        let head = local("POST", "/v1/x", &[]);

        assert_eq!(
            params_of(&head, r#"  {"id":"journal:open"} "#),
            Ok(pairs(&[("id", "journal:open")]))
        );
    }

    #[test]
    fn a_json_body_overrides_the_query() {
        let head = local("POST", "/v1/status?status=off", &[]);

        assert_eq!(
            params_of(&head, r#"{"status":"break"}"#),
            Ok(pairs(&[("status", "break")]))
        );
    }

    #[test]
    fn invalid_json_is_a_bad_request() {
        let head = local("POST", "/v1/x", &["Content-Type: application/json"]);

        for body in ["{nope", "not json at all"] {
            assert_eq!(
                params_of(&head, body),
                Err(AutomationError::bad("The body isn't valid JSON.")),
                "{body}"
            );
        }
    }

    #[test]
    fn json_that_is_not_an_object_is_a_bad_request() {
        let head = local("POST", "/v1/x", &["Content-Type: application/json"]);

        for body in ["[1,2]", "\"text\"", "5", "null"] {
            assert_eq!(
                params_of(&head, body),
                Err(AutomationError::bad("The body must be a JSON object.")),
                "{body}"
            );
        }
    }

    #[test]
    fn a_get_request_ignores_its_body() {
        let head = local("GET", "/v1/x?a=1", &[]);

        assert_eq!(params_of(&head, r#"{"b":"2"}"#), Ok(pairs(&[("a", "1")])));
    }

    #[test]
    fn a_body_that_is_not_utf8_is_a_bad_request() {
        let head = local("POST", "/v1/x", &[]);

        assert_eq!(
            params(&head, &[0xff, 0xfe]),
            Err(AutomationError::bad("The body isn't UTF-8."))
        );
    }

    #[test]
    fn the_token_never_reaches_the_params() {
        let query = local("GET", "/v1/x?token=s3cret&a=1", &[]);
        let json = local("POST", "/v1/x", &[]);
        let form = local("POST", "/v1/x?a=1", &[]);

        assert_eq!(params_of(&query, ""), Ok(pairs(&[("a", "1")])));
        assert_eq!(
            params_of(&json, r#"{"token":"s3cret","a":"1"}"#),
            Ok(pairs(&[("a", "1")]))
        );
        assert_eq!(params_of(&form, "token=s3cret"), Ok(pairs(&[("a", "1")])));
    }

    #[test]
    fn a_success_answers_ok_true_with_the_outcome() {
        let mut outcome = Outcome::new();
        outcome.insert("result".into(), json!("set"));

        let (head, body) = answer(response(Ok(outcome)));

        assert_eq!(
            head,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: 27\r\nConnection: close"
        );
        assert_eq!(body, json!({ "ok": true, "result": "set" }));
    }

    #[test]
    fn the_content_length_counts_the_body_with_its_newline() {
        let bytes = response(Ok(Outcome::new()));
        let text = String::from_utf8(bytes).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();

        assert_eq!(body, "{\"ok\":true}\n");
        assert!(
            head.contains(&format!("Content-Length: {}", body.len())),
            "{head}"
        );
    }

    #[test]
    fn a_failure_answers_ok_false_with_the_errors_status() {
        for (status, line) in [
            (400, "400 Bad Request"),
            (401, "401 Unauthorized"),
            (403, "403 Forbidden"),
            (404, "404 Not Found"),
            (405, "405 Method Not Allowed"),
            (409, "409 Conflict"),
            (411, "411 Length Required"),
            (413, "413 Content Too Large"),
            (422, "422 Unprocessable Content"),
            (503, "503 Service Unavailable"),
            (500, "500 Internal Server Error"),
        ] {
            let (head, body) = answer(response(Err(AutomationError::new(status, "Nope."))));

            assert!(
                head.starts_with(&format!("HTTP/1.1 {line}\r\n")),
                "{status}: {head}"
            );
            assert_eq!(body, json!({ "ok": false, "error": "Nope." }), "{status}");
        }
    }
}
