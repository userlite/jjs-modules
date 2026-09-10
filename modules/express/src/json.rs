//! The collector retains VM-owned byte chunks, charged by the existing byte allocator.
use super::*;

pub(super) fn resume_handlers(
    c: &mut dyn ModuleContext,
    frame: ValueHandle,
    error: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let container = c.get_property(frame, "container")?;
    let handlers = c.get_property(frame, "handlers")?;
    let request = c.get_property(frame, "request")?;
    let response = c.get_property(frame, "response")?;
    let layer = c.get_property(frame, "layer")?;
    let layer = c.as_number(layer)? as usize;
    let start = c.get_property(frame, "handler")?;
    let start = c.as_number(start)? as usize;
    let errors = c.get_property(frame, "errors")?;
    let error = if c.is_truthy(error)? {
        Some(error)
    } else {
        None
    };
    match call_handlers(
        c,
        handlers,
        request,
        response,
        error,
        Some(errors),
        Some((container, layer)),
        start,
    ) {
        Ok(HandlerFlow::Stop) => Ok(return_undefined(c)),
        Ok(HandlerFlow::Advance(error)) => {
            run_layers(c, container, request, response, layer, error)
        }
        Ok(HandlerFlow::Unsupported(value)) => {
            Ok(throw(format!("Express M1 unsupported: next({value})")))
        }
        Err(result) => Ok(result),
    }
}

pub(super) fn json_fail(
    c: &mut dyn ModuleContext,
    next: ValueHandle,
    name: &str,
    message: &str,
    status: u16,
) -> Result<ModuleCallResult, ModuleError> {
    let error = c.object()?;
    set_string(c, error, "name", name)?;
    set_string(c, error, "message", message)?;
    let status = c.number(status as f64)?;
    c.set_property(error, "status", status)?;
    c.set_property(error, "statusCode", status)?;
    let undefined = c.undefined();
    c.call(next, undefined, &[error])?;
    Ok(return_undefined(c))
}

pub(super) fn json_encoding_error(
    c: &mut dyn ModuleContext,
    headers: ValueHandle,
    content_type: &str,
) -> Result<Option<String>, ModuleError> {
    let encoding = c.get_property(headers, "content-encoding")?;
    if c.value_kind(encoding)? != ModuleValueKind::Undefined
        && !c
            .as_string(encoding)?
            .trim()
            .eq_ignore_ascii_case("identity")
    {
        return Ok(Some(
            "express.json supports only identity content encoding".into(),
        ));
    }
    for parameter in content_type.split(';').skip(1) {
        if let Some((name, value)) = parameter.trim().split_once('=') {
            if name.trim().eq_ignore_ascii_case("charset")
                && !value.trim().trim_matches('"').eq_ignore_ascii_case("utf-8")
            {
                return Ok(Some("express.json supports only UTF-8 charset".into()));
            }
        } else if parameter.trim().eq_ignore_ascii_case("charset") {
            return Ok(Some("express.json charset is missing".into()));
        }
    }
    Ok(None)
}

pub(super) fn json_parse_body(
    c: &mut dyn ModuleContext,
    request: ValueHandle,
    next: ValueHandle,
    bytes: &[u8],
    limit: usize,
    strict: bool,
    form: bool,
) -> Result<ModuleCallResult, ModuleError> {
    if bytes.len() > limit {
        return json_fail(
            c,
            next,
            "ExpressJsonLimitError",
            &format!("express.json body exceeds {limit} bytes"),
            413,
        );
    }
    if form {
        return super::form::parse_body(c, request, next, bytes);
    }
    let raw = match std::str::from_utf8(bytes) {
        Ok(raw) => raw,
        Err(_) => {
            return json_fail(
                c,
                next,
                "ExpressJsonSyntaxError",
                "express.json requires valid UTF-8",
                400,
            )
        }
    };
    let parsed = if raw.is_empty() {
        c.object()?
    } else {
        let encoded = c.string(raw)?;
        match c.json_parse(encoded) {
            Ok(parsed) => parsed,
            Err(ModuleError::ContractViolation(message))
                if message.starts_with("runtime error: json_parse_invalid") =>
            {
                return json_fail(
                    c,
                    next,
                    "ExpressJsonSyntaxError",
                    "express.json request body is malformed JSON",
                    400,
                )
            }
            Err(error) => return Err(error),
        }
    };
    if strict
        && !matches!(
            c.value_kind(parsed)?,
            ModuleValueKind::Object | ModuleValueKind::Array
        )
    {
        return json_fail(
            c,
            next,
            "ExpressJsonStrictError",
            "express.json strict mode accepts only objects or arrays",
            400,
        );
    }
    parsed_body(c, request, next, parsed)
}

pub(super) fn parsed_body(c: &mut dyn ModuleContext, request: ValueHandle, next: ValueHandle, parsed: ValueHandle) -> Result<ModuleCallResult, ModuleError> {
    c.set_property(request, "body", parsed)?;
    let yes = c.bool(true)?;
    let marker = c.function(JSON_EVENT)?;
    c.set_private(marker, JSON_DONE, yes)?;
    c.set_property(request, "_expressJsonState", marker)?;
    let undefined = c.undefined();
    c.call(next, undefined, &[])?;
    Ok(return_undefined(c))
}

const EVENTS: [&str; 6] = ["data", "end", "error", "aborted", "close", "responseClose"];

pub(super) fn json_collect(
    c: &mut dyn ModuleContext,
    request: ValueHandle,
    response: ValueHandle,
    next: ValueHandle,
    limit: usize,
    strict: bool,
    form: bool,
) -> Result<ModuleCallResult, ModuleError> {
    let status = c.get_property(request, "_bodyInputState")?;
    if c.value_kind(status)? != ModuleValueKind::String || c.as_string(status)? != "available" {
        return json_fail(
            c,
            next,
            "ExpressJsonRequestError",
            "express.json requires unconsumed raw request input",
            500,
        );
    }
    let state = c.get_property(request, "_expressJsonState")?;
    if c.value_kind(state)? != ModuleValueKind::Undefined {
        return json_fail(
            c,
            next,
            "ExpressJsonRequestError",
            "express.json collection already started",
            500,
        );
    }
    let headers = c.get_property(request, "headers")?;
    let length = c.get_property(headers, "content-length")?;
    if c.value_kind(length)? == ModuleValueKind::String {
        let length = c.as_string(length)?;
        if !length.is_empty() && length.bytes().all(|b| b.is_ascii_digit()) {
            if length
                .parse::<u64>()
                .map(|n| n > limit as u64)
                .unwrap_or(true)
            {
                let pause = c.get_property(request, "pause")?;
                c.call(pause, request, &[])?;
                return json_fail(
                    c,
                    next,
                    "ExpressJsonLimitError",
                    &format!("express.json body exceeds {limit} bytes"),
                    413,
                );
            }
        }
    }
    let pause = c.get_property(request, "pause")?;
    c.call(pause, request, &[])?;
    let state = c.object()?;
    let marker = c.function(JSON_EVENT)?;
    c.set_private(marker, JSON_STATE, state)?;
    c.set_property(request, "_expressJsonState", marker)?;
    for (name, value) in [("request", request), ("response", response), ("next", next)] {
        c.set_property(state, name, value)?;
    }
    let chunks = c.array()?;
    c.set_property(state, "chunks", chunks)?;
    for (name, n) in [("limit", limit), ("received", 0)] {
        let value = c.number(n as f64)?;
        c.set_property(state, name, value)?;
    }
    let form = c.bool(form)?;
    c.set_property(state, "form", form)?;
    let strict = c.bool(strict)?;
    c.set_property(state, "strict", strict)?;
    for name in EVENTS {
        let callback = c.function(JSON_EVENT)?;
        c.set_private(callback, JSON_STATE, state)?;
        let event = c.string(name)?;
        c.set_private(callback, JSON_EVENT_NAME, event)?;
        c.set_property(state, name, callback)?;
        let target = if name == "responseClose" {
            response
        } else {
            request
        };
        let on = c.get_property(
            target,
            if name == "responseClose" {
                "_expressRawOn"
            } else {
                "on"
            },
        )?;
        let event = c.string(if name == "responseClose" {
            "close"
        } else {
            name
        })?;
        c.call(on, target, &[event, callback])?;
    }
    let resume = c.get_property(request, "resume")?;
    c.call(resume, request, &[])?;
    Ok(return_undefined(c))
}

fn cleanup(c: &mut dyn ModuleContext, state: ValueHandle) -> Result<(), ModuleError> {
    let yes = c.bool(true)?;
    c.set_property(state, "done", yes)?;
    let request = c.get_property(state, "request")?;
    let response = c.get_property(state, "response")?;
    for name in EVENTS {
        let callback = c.get_property(state, name)?;
        let target = if name == "responseClose" {
            response
        } else {
            request
        };
        let remove = c.get_property(target, "removeListener")?;
        let event = c.string(if name == "responseClose" {
            "close"
        } else {
            name
        })?;
        c.call(remove, target, &[event, callback])?;
        let undefined = c.undefined();
        c.set_private(callback, JSON_STATE, undefined)?;
        c.set_property(state, name, undefined)?;
    }
    let undefined = c.undefined();
    let marker = c.get_property(request, "_expressJsonState")?;
    c.set_private(marker, JSON_STATE, undefined)?;
    c.set_property(request, "_expressJsonState", undefined)?;
    for name in ["chunks", "request", "response", "next"] {
        c.set_property(state, name, undefined)?;
    }
    Ok(())
}

pub(super) fn json_event(
    c: &mut dyn ModuleContext,
    callback: ValueHandle,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    let state = c.get_private(callback, JSON_STATE)?;
    if c.value_kind(state)? == ModuleValueKind::Undefined {
        return Ok(return_undefined(c));
    }
    let done = c.get_property(state, "done")?;
    if c.is_truthy(done)? {
        return Ok(return_undefined(c));
    }
    let name = c.get_private(callback, JSON_EVENT_NAME)?;
    let name = c.as_string(name)?;
    let request = c.get_property(state, "request")?;
    let next = c.get_property(state, "next")?;
    let limit = c.get_property(state, "limit")?;
    let limit = c.as_number(limit)? as usize;
    let received = c.get_property(state, "received")?;
    let received = c.as_number(received)? as usize;
    if name == "data" {
        if args.len() != 1 || !c.is_bytes(args[0]) {
            cleanup(c, state)?;
            return json_fail(
                c,
                next,
                "ExpressJsonRequestError",
                "express.json requires raw byte chunks",
                500,
            );
        }
        let bytes = c.read_bytes(args[0])?;
        if bytes.len() > limit - received {
            cleanup(c, state)?;
            let pause = c.get_property(request, "pause")?;
            c.call(pause, request, &[])?;
            return json_fail(
                c,
                next,
                "ExpressJsonLimitError",
                &format!("express.json body exceeds {limit} bytes"),
                413,
            );
        }
        let chunks = c.get_property(state, "chunks")?;
        c.array_push(chunks, args[0])?;
        let received = c.number((received + bytes.len()) as f64)?;
        c.set_property(state, "received", received)?;
        return Ok(return_undefined(c));
    }
    if name == "end" {
        let strict = c.get_property(state, "strict")?;
        let strict = c.as_bool(strict)?;
        let chunks = c.get_property(state, "chunks")?;
        let mut bytes = Vec::with_capacity(received);
        for i in 0..c.array_len(chunks)? {
            let chunk = c.array_get(chunks, i)?;
            bytes.extend_from_slice(&c.read_bytes(chunk)?);
        }
        let form = c.get_property(state, "form")?;
        let form = c.as_bool(form)?;
        cleanup(c, state)?;
        return json_parse_body(c, request, next, &bytes, limit, strict, form);
    }
    let response = c.get_property(state, "response")?;
    let ended = c.get_property(response, "_expressEnded")?;
    let input = c.get_property(request, "_bodyInputState")?;
    let writable = c.as_string(input)? != "terminal";
    cleanup(c, state)?;
    if name == "error" && writable && !c.is_truthy(ended)? {
        return json_fail(
            c,
            next,
            "ExpressJsonRequestError",
            "express.json input failed before completion",
            400,
        );
    }
    Ok(return_undefined(c))
}

// Native function records retain private handles; detach every saved continuation
// when it fires or its response closes, including middleware that never calls next.
pub(super) fn suspend_next(
    c: &mut dyn ModuleContext,
    next: ValueHandle,
    response: ValueHandle,
) -> Result<(), ModuleError> {
    let yes = c.bool(true)?;
    c.set_private(next, NEXT_SUSPENDED, yes)?;
    c.set_private(next, ASYNC_RESPONSE, response)?;
    let callback = c.function(NEXT_CLOSE)?;
    c.set_private(callback, NEXT_OWNER, next)?;
    c.set_private(next, NEXT_CLOSE_CALLBACK, callback)?;
    let on = c.get_property(response, "_expressRawOn")?;
    let close = c.string("close")?;
    c.call(on, response, &[close, callback])?;
    Ok(())
}
pub(super) fn retire_next(c: &mut dyn ModuleContext, next: ValueHandle) -> Result<(), ModuleError> {
    let callback = c.get_private(next, NEXT_CLOSE_CALLBACK)?;
    if c.value_kind(callback)? != ModuleValueKind::Undefined {
        let response = c.get_private(next, ASYNC_RESPONSE)?;
        let remove = c.get_property(response, "removeListener")?;
        let close = c.string("close")?;
        c.call(remove, response, &[close, callback])?;
        let undefined = c.undefined();
        c.set_private(callback, NEXT_OWNER, undefined)?;
    }
    let undefined = c.undefined();
    for field in [
        NEXT_FRAME,
        ASYNC_RESPONSE,
        NEXT_CLOSE_CALLBACK,
        NEXT_SUSPENDED,
    ] {
        c.set_private(next, field, undefined)?;
    }
    let yes = c.bool(true)?;
    c.set_private(next, NEXT_CALLED, yes)?;
    Ok(())
}
