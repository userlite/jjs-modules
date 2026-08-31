//! Rust-native Express M1 module implemented only through `jjs-module-api`.

use jjs_module_api::{
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleDependency, ModuleError,
    ModuleFunctionKey, ModuleIdentity, ModuleManifest, ModuleValueKind, NativeModule, ValueHandle,
    MODULE_API_VERSION,
};

const EXPRESS: ModuleFunctionKey = ModuleFunctionKey(1);
const ROUTER: ModuleFunctionKey = ModuleFunctionKey(2);
const CONTAINER: ModuleFunctionKey = ModuleFunctionKey(3);
const USE: ModuleFunctionKey = ModuleFunctionKey(4);
const GET: ModuleFunctionKey = ModuleFunctionKey(5);
const POST: ModuleFunctionKey = ModuleFunctionKey(6);
const PUT: ModuleFunctionKey = ModuleFunctionKey(7);
const PATCH: ModuleFunctionKey = ModuleFunctionKey(8);
const DELETE: ModuleFunctionKey = ModuleFunctionKey(9);
const HEAD: ModuleFunctionKey = ModuleFunctionKey(10);
const OPTIONS: ModuleFunctionKey = ModuleFunctionKey(11);
const ALL: ModuleFunctionKey = ModuleFunctionKey(12);
const LISTEN: ModuleFunctionKey = ModuleFunctionKey(13);
const UNSUPPORTED: ModuleFunctionKey = ModuleFunctionKey(14);
const SETTING: ModuleFunctionKey = ModuleFunctionKey(15);
const REQUEST_GET: ModuleFunctionKey = ModuleFunctionKey(16);
const RESPONSE_END: ModuleFunctionKey = ModuleFunctionKey(17);
const RESPONSE_STATUS: ModuleFunctionKey = ModuleFunctionKey(18);
const RESPONSE_SET: ModuleFunctionKey = ModuleFunctionKey(19);
const RESPONSE_GET: ModuleFunctionKey = ModuleFunctionKey(20);
const RESPONSE_TYPE: ModuleFunctionKey = ModuleFunctionKey(21);
const RESPONSE_SEND: ModuleFunctionKey = ModuleFunctionKey(22);
const RESPONSE_JSON: ModuleFunctionKey = ModuleFunctionKey(23);
const RESPONSE_REDIRECT: ModuleFunctionKey = ModuleFunctionKey(24);
const NEXT: ModuleFunctionKey = ModuleFunctionKey(25);
const ASYNC_REJECT: ModuleFunctionKey = ModuleFunctionKey(26);
const RESPONSE_FLUSH_HEADERS: ModuleFunctionKey = ModuleFunctionKey(27);
const RESPONSE_WRITE: ModuleFunctionKey = ModuleFunctionKey(28);
const RESPONSE_ON: ModuleFunctionKey = ModuleFunctionKey(29);
const EXPRESS_JSON: ModuleFunctionKey = ModuleFunctionKey(30);
const JSON_MIDDLEWARE: ModuleFunctionKey = ModuleFunctionKey(31);
const EXPRESS_ASSETS: ModuleFunctionKey = ModuleFunctionKey(32);
const ASSET_MIDDLEWARE: ModuleFunctionKey = ModuleFunctionKey(33);

const LAYERS: u32 = 1;
const UNSUPPORTED_MESSAGE: u32 = 2;
const NEXT_CALLED: u32 = 3;
const NEXT_ERROR: u32 = 4;
const ASYNC_RESPONSE: u32 = 5;
const JSON_LIMIT: u32 = 6;
const ASSET_MANIFEST: u32 = 7;
const ASYNC_REQUEST: u32 = 8;
const ASYNC_ERROR_HANDLERS: u32 = 9;

pub struct ExpressModule {
    manifest: ModuleManifest,
}

impl Default for ExpressModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.express".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-express-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 5,
                imports: vec!["express".into()],
                capabilities: vec![],
                dependencies: vec![ModuleDependency {
                    id: "org.jjs.node-http".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-http-v1".into(),
                }],
                function_keys: (1..=33).collect(),
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}

fn throw(message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "Error".into(),
        message: message.into(),
    }
}

fn type_throw(message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "TypeError".into(),
        message: message.into(),
    }
}

fn named_throw(name: &str, message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: name.into(),
        message: message.into(),
    }
}

fn return_undefined(context: &mut dyn ModuleContext) -> ModuleCallResult {
    ModuleCallResult::Return(context.undefined())
}

fn property_string(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
) -> Result<String, ModuleError> {
    let value = context.get_property(object, name)?;
    context.as_string(value)
}

fn set_string(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    value: &str,
) -> Result<(), ModuleError> {
    let value = context.string(value)?;
    context.set_property(object, name, value)
}

fn unsupported_function(
    context: &mut dyn ModuleContext,
    message: &str,
) -> Result<ValueHandle, ModuleError> {
    let function = context.function(UNSUPPORTED)?;
    let message = context.string(message)?;
    context.set_private(function, UNSUPPORTED_MESSAGE, message)?;
    Ok(function)
}

fn attach_function(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<(), ModuleError> {
    let function = context.function(key)?;
    context.set_property(object, name, function)
}

fn create_container(
    context: &mut dyn ModuleContext,
    router: bool,
) -> Result<ValueHandle, ModuleError> {
    let container = context.function(CONTAINER)?;
    let layers = context.array()?;
    context.set_private(container, LAYERS, layers)?;
    set_string(
        context,
        container,
        "_expressKind",
        if router { "router" } else { "app" },
    )?;
    attach_function(context, container, "use", USE)?;
    for (name, key) in [
        ("get", GET),
        ("post", POST),
        ("put", PUT),
        ("patch", PATCH),
        ("delete", DELETE),
        ("head", HEAD),
        ("options", OPTIONS),
        ("all", ALL),
    ] {
        attach_function(context, container, name, key)?;
    }
    if !router {
        attach_function(context, container, "listen", LISTEN)?;
    }
    let route = unsupported_function(context, "Express M1 unsupported: route")?;
    context.set_property(container, "route", route)?;
    let param = unsupported_function(context, "Express M1 unsupported: param")?;
    context.set_property(container, "param", param)?;
    let render = unsupported_function(context, "Express M1 unsupported: views")?;
    context.set_property(container, "render", render)?;
    attach_function(context, container, "set", SETTING)?;
    let enable = unsupported_function(context, "Express M1 unsupported: settings")?;
    context.set_property(container, "enable", enable)?;
    let disable = unsupported_function(context, "Express M1 unsupported: settings")?;
    context.set_property(container, "disable", disable)?;
    Ok(container)
}

fn validate_path(
    context: &dyn ModuleContext,
    value: ValueHandle,
) -> Result<String, ModuleCallResult> {
    if context.value_kind(value).ok() != Some(ModuleValueKind::String) {
        return Err(throw("Express M1 unsupported: route path shape"));
    }
    let path = context
        .as_string(value)
        .map_err(|_| throw("Express M1 unsupported: route path shape"))?;
    if path.contains('*') {
        return Err(throw("Express M1 unsupported: route path shape"));
    }
    Ok(path)
}

fn handlers_array(
    context: &mut dyn ModuleContext,
    args: &[ValueHandle],
    start: usize,
) -> Result<ValueHandle, ModuleCallResult> {
    if start >= args.len() {
        return Err(type_throw("express_m1_handler_not_callable"));
    }
    let handlers = context.array().map_err(|error| throw(error.to_string()))?;
    for handler in &args[start..] {
        if !context.is_callable(*handler) {
            return Err(type_throw("express_m1_handler_not_callable"));
        }
        context
            .array_push(handlers, *handler)
            .map_err(|error| throw(error.to_string()))?;
    }
    Ok(handlers)
}

fn add_layer(
    context: &mut dyn ModuleContext,
    container: ValueHandle,
    kind: &str,
    path: &str,
    method: Option<&str>,
    handlers: ValueHandle,
    router: Option<ValueHandle>,
) -> Result<(), ModuleError> {
    let layer = context.object()?;
    set_string(context, layer, "kind", kind)?;
    set_string(context, layer, "path", path)?;
    if let Some(method) = method {
        set_string(context, layer, "method", method)?;
    }
    context.set_property(layer, "handlers", handlers)?;
    if let Some(router) = router {
        context.set_property(layer, "router", router)?;
    }
    let layers = context.get_private(container, LAYERS)?;
    context.array_push(layers, layer)
}

fn segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|v| !v.is_empty())
        .collect()
}

fn percent_decode(value: &str, plus: bool) -> Result<String, ()> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' if plus => {
                out.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(());
                }
                let digit = |value: u8| match value {
                    b'0'..=b'9' => Some(value - b'0'),
                    b'a'..=b'f' => Some(value - b'a' + 10),
                    b'A'..=b'F' => Some(value - b'A' + 10),
                    _ => None,
                };
                let high = digit(bytes[index + 1]).ok_or(())?;
                let low = digit(bytes[index + 2]).ok_or(())?;
                out.push((high << 4) | low);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn match_path(
    pattern: &str,
    path: &str,
    prefix: bool,
) -> Result<Option<(String, Vec<(String, String)>)>, ModuleCallResult> {
    let wanted = segments(pattern);
    let actual = segments(path);
    if wanted.len() > actual.len() || (!prefix && wanted.len() != actual.len()) {
        return Ok(None);
    }
    let mut params = Vec::new();
    for (wanted, actual) in wanted.iter().zip(actual.iter()) {
        if let Some(name) = wanted.strip_prefix(':') {
            let value = percent_decode(actual, false)
                .map_err(|_| throw("Express M1 request error: malformed URI escape"))?;
            params.push((name.to_owned(), value));
        } else if !wanted.eq_ignore_ascii_case(actual) {
            return Ok(None);
        }
    }
    let prefix_value = if wanted.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", actual[..wanted.len()].join("/"))
    };
    Ok(Some((prefix_value, params)))
}

fn merge_params(
    context: &mut dyn ModuleContext,
    request: ValueHandle,
    params: &[(String, String)],
) -> Result<(), ModuleError> {
    let previous = context.get_property(request, "params")?;
    let merged = context.object()?;
    if context.value_kind(previous)? == ModuleValueKind::Object {
        for name in context.own_property_names(previous)? {
            let value = context.get_property(previous, &name)?;
            context.set_property(merged, &name, value)?;
        }
    }
    for (name, value) in params {
        let current = context.get_property(merged, name)?;
        if context.value_kind(current)? == ModuleValueKind::Undefined {
            let value = context.string(value)?;
            context.set_property(merged, name, value)?;
        }
    }
    context.set_property(request, "params", merged)
}

fn query_object(
    context: &mut dyn ModuleContext,
    url: &str,
) -> Result<ValueHandle, ModuleCallResult> {
    let result = context.object().map_err(|error| throw(error.to_string()))?;
    let Some((_, query)) = url.split_once('?') else {
        return Ok(result);
    };
    if query.is_empty() {
        return Ok(result);
    }
    for pair in query.split('&') {
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(raw_key, true)
            .map_err(|_| throw("Express M1 request error: malformed URI escape"))?;
        let value = percent_decode(raw_value, true)
            .map_err(|_| throw("Express M1 request error: malformed URI escape"))?;
        if matches!(key.as_str(), "__proto__" | "prototype" | "constructor") {
            return Err(throw("Express M1 request error: dangerous query key"));
        }
        let existing = context
            .get_property(result, &key)
            .map_err(|error| throw(error.to_string()))?;
        let value = context
            .string(&value)
            .map_err(|error| throw(error.to_string()))?;
        match context
            .value_kind(existing)
            .map_err(|error| throw(error.to_string()))?
        {
            ModuleValueKind::Undefined => context
                .set_property(result, &key, value)
                .map_err(|error| throw(error.to_string()))?,
            ModuleValueKind::Array => context
                .array_push(existing, value)
                .map_err(|error| throw(error.to_string()))?,
            _ => {
                let values = context.array().map_err(|error| throw(error.to_string()))?;
                context
                    .array_push(values, existing)
                    .map_err(|error| throw(error.to_string()))?;
                context
                    .array_push(values, value)
                    .map_err(|error| throw(error.to_string()))?;
                context
                    .set_property(result, &key, values)
                    .map_err(|error| throw(error.to_string()))?;
            }
        }
    }
    Ok(result)
}

fn decorate_request(
    context: &mut dyn ModuleContext,
    request: ValueHandle,
    app: ValueHandle,
) -> Result<(), ModuleCallResult> {
    let decorated = context
        .get_property(request, "_expressDecorated")
        .map_err(|e| throw(e.to_string()))?;
    if context
        .is_truthy(decorated)
        .map_err(|e| throw(e.to_string()))?
    {
        return Ok(());
    }
    let yes = context.bool(true).map_err(|e| throw(e.to_string()))?;
    context
        .set_property(request, "_expressDecorated", yes)
        .map_err(|e| throw(e.to_string()))?;
    let url_value = context
        .get_property(request, "url")
        .map_err(|e| throw(e.to_string()))?;
    let url = if context
        .value_kind(url_value)
        .map_err(|e| throw(e.to_string()))?
        == ModuleValueKind::String
    {
        context
            .as_string(url_value)
            .map_err(|e| throw(e.to_string()))?
    } else {
        "/".to_owned()
    };
    set_string(context, request, "url", &url).map_err(|e| throw(e.to_string()))?;
    set_string(context, request, "originalUrl", &url).map_err(|e| throw(e.to_string()))?;
    set_string(context, request, "baseUrl", "").map_err(|e| throw(e.to_string()))?;
    let path = url.split('?').next().unwrap_or(&url);
    set_string(context, request, "path", path).map_err(|e| throw(e.to_string()))?;
    let params = context.object().map_err(|e| throw(e.to_string()))?;
    context
        .set_property(request, "params", params)
        .map_err(|e| throw(e.to_string()))?;
    let query = query_object(context, &url)?;
    context
        .set_property(request, "query", query)
        .map_err(|e| throw(e.to_string()))?;
    context
        .set_property(request, "app", app)
        .map_err(|e| throw(e.to_string()))?;
    let headers_source = context
        .get_property(request, "headers")
        .map_err(|e| throw(e.to_string()))?;
    let headers = context.object().map_err(|e| throw(e.to_string()))?;
    if context
        .value_kind(headers_source)
        .map_err(|e| throw(e.to_string()))?
        == ModuleValueKind::Object
    {
        for name in context
            .own_property_names(headers_source)
            .map_err(|e| throw(e.to_string()))?
        {
            let value = context
                .get_property(headers_source, &name)
                .map_err(|e| throw(e.to_string()))?;
            context
                .set_property(headers, &name.to_ascii_lowercase(), value)
                .map_err(|e| throw(e.to_string()))?;
        }
    }
    context
        .set_property(request, "headers", headers)
        .map_err(|e| throw(e.to_string()))?;
    attach_function(context, request, "get", REQUEST_GET).map_err(|e| throw(e.to_string()))?;
    let get = context
        .get_property(request, "get")
        .map_err(|e| throw(e.to_string()))?;
    context
        .set_property(request, "header", get)
        .map_err(|e| throw(e.to_string()))?;
    let accepts = unsupported_function(context, "Express M1 unsupported: content negotiation")
        .map_err(|e| throw(e.to_string()))?;
    context
        .set_property(request, "accepts", accepts)
        .map_err(|e| throw(e.to_string()))?;
    Ok(())
}

fn decorate_response(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<(), ModuleError> {
    let decorated = context.get_property(response, "_expressDecorated")?;
    if context.is_truthy(decorated)? {
        return Ok(());
    }
    let yes = context.bool(true)?;
    context.set_property(response, "_expressDecorated", yes)?;
    let raw_end = context.get_property(response, "end")?;
    let raw_set = context.get_property(response, "setHeader")?;
    let raw_flush = context.get_property(response, "flushHeaders")?;
    let raw_write = context.get_property(response, "write")?;
    let raw_on = context.get_property(response, "on")?;
    context.set_property(response, "_expressRawEnd", raw_end)?;
    context.set_property(response, "_expressRawSetHeader", raw_set)?;
    context.set_property(response, "_expressRawFlushHeaders", raw_flush)?;
    context.set_property(response, "_expressRawWrite", raw_write)?;
    context.set_property(response, "_expressRawOn", raw_on)?;
    let no = context.bool(false)?;
    context.set_property(response, "_expressEnded", no)?;
    context.set_property(response, "_expressStreaming", no)?;
    let headers = context.object()?;
    context.set_property(response, "_expressHeaders", headers)?;
    for (name, key) in [
        ("end", RESPONSE_END),
        ("status", RESPONSE_STATUS),
        ("set", RESPONSE_SET),
        ("get", RESPONSE_GET),
        ("type", RESPONSE_TYPE),
        ("send", RESPONSE_SEND),
        ("json", RESPONSE_JSON),
        ("redirect", RESPONSE_REDIRECT),
        ("flushHeaders", RESPONSE_FLUSH_HEADERS),
        ("write", RESPONSE_WRITE),
        ("on", RESPONSE_ON),
    ] {
        attach_function(context, response, name, key)?;
    }
    let location = unsupported_function(context, "Express M1 unsupported: res.location")?;
    context.set_property(response, "location", location)?;
    let cookie = unsupported_function(context, "Express M1 unsupported: cookies and sessions")?;
    context.set_property(response, "cookie", cookie)?;
    let send_file = unsupported_function(
        context,
        "Express M1 unsupported: streams, ranges, and compression",
    )?;
    context.set_property(response, "sendFile", send_file)?;
    Ok(())
}

fn response_set(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    name: &str,
    value: &str,
) -> Result<(), ModuleError> {
    let streaming = context.get_property(response, "_expressStreaming")?;
    if context.is_truthy(streaming)? {
        return Err(ModuleError::ContractViolation(
            "Express response error: headers already committed".into(),
        ));
    }
    let raw = context.get_property(response, "_expressRawSetHeader")?;
    let name_handle = context.string(name)?;
    let value_handle = context.string(value)?;
    context.call(raw, response, &[name_handle, value_handle])?;
    let headers = context.get_property(response, "_expressHeaders")?;
    context.set_property(headers, &name.to_ascii_lowercase(), value_handle)?;
    Ok(())
}

fn response_end(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    body: Option<&str>,
) -> Result<ModuleCallResult, ModuleError> {
    let ended = context.get_property(response, "_expressEnded")?;
    if context.is_truthy(ended)? {
        return Ok(throw("Express M1 response error: already ended"));
    }
    let raw = context.get_property(response, "_expressRawEnd")?;
    let head = context.get_property(response, "_expressHead")?;
    if context.is_truthy(head)? || body.is_none() {
        context.call(raw, response, &[])?;
    } else {
        let body = context.string(body.unwrap())?;
        context.call(raw, response, &[body])?;
    }
    let yes = context.bool(true)?;
    context.set_property(response, "_expressEnded", yes)?;
    Ok(return_undefined(context))
}

fn response_flush_headers(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let ended = context.get_property(response, "_expressEnded")?;
    if context.is_truthy(ended)? {
        return Ok(throw("Express response error: stream closed"));
    }
    let raw = context.get_property(response, "_expressRawFlushHeaders")?;
    context.call(raw, response, &[])?;
    let yes = context.bool(true)?;
    context.set_property(response, "_expressStreaming", yes)?;
    Ok(return_undefined(context))
}

fn response_write(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
        return Ok(type_throw(
            "Express response error: write requires a string",
        ));
    }
    let ended = context.get_property(response, "_expressEnded")?;
    if context.is_truthy(ended)? {
        return Ok(throw("Express response error: stream closed"));
    }
    let raw = context.get_property(response, "_expressRawWrite")?;
    let accepted = context.call(raw, response, args)?;
    let yes = context.bool(true)?;
    context.set_property(response, "_expressStreaming", yes)?;
    Ok(ModuleCallResult::Return(accepted))
}

fn response_type(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    name: &str,
) -> Result<(), ModuleError> {
    let value = match name {
        "html" => "text/html; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "text" => "text/plain; charset=utf-8",
        other => other,
    };
    response_set(context, response, "Content-Type", value)
}

fn response_send(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    body: Option<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    let body = body.unwrap_or_else(|| context.undefined());
    if context.value_kind(body)? == ModuleValueKind::Object {
        let encoded = context.json_stringify(body)?;
        response_type(context, response, "json")?;
        let encoded = context.as_string(encoded)?;
        return response_end(context, response, Some(&encoded));
    }
    let headers = context.get_property(response, "_expressHeaders")?;
    let content_type = context.get_property(headers, "content-type")?;
    if context.value_kind(content_type)? == ModuleValueKind::Undefined
        && context.value_kind(body)? == ModuleValueKind::String
    {
        response_type(context, response, "html")?;
    }
    let text = if context.value_kind(body)? == ModuleValueKind::Undefined {
        String::new()
    } else {
        context.to_string(body)?
    };
    response_end(context, response, Some(&text))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn method_name(key: ModuleFunctionKey) -> Option<&'static str> {
    Some(match key {
        GET => "GET",
        POST => "POST",
        PUT => "PUT",
        PATCH => "PATCH",
        DELETE => "DELETE",
        HEAD => "HEAD",
        OPTIONS => "OPTIONS",
        ALL => "ALL",
        _ => return None,
    })
}

fn has_explicit(
    context: &mut dyn ModuleContext,
    container: ValueHandle,
    path: &str,
    method: &str,
) -> Result<bool, ModuleCallResult> {
    let layers = context
        .get_private(container, LAYERS)
        .map_err(|e| throw(e.to_string()))?;
    for index in 0..context
        .array_len(layers)
        .map_err(|e| throw(e.to_string()))?
    {
        let layer = context
            .array_get(layers, index)
            .map_err(|e| throw(e.to_string()))?;
        if property_string(context, layer, "kind").map_err(|e| throw(e.to_string()))? != "route" {
            continue;
        }
        let layer_method =
            property_string(context, layer, "method").map_err(|e| throw(e.to_string()))?;
        if layer_method != method && layer_method != "ALL" {
            continue;
        }
        let layer_path =
            property_string(context, layer, "path").map_err(|e| throw(e.to_string()))?;
        if match_path(&layer_path, path, false)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

enum HandlerFlow {
    Stop,
    Advance(Option<ValueHandle>),
    Unsupported(String),
}

fn later_error_handlers(
    context: &mut dyn ModuleContext,
    layers: ValueHandle,
    start: usize,
    path: &str,
) -> Result<ValueHandle, ModuleCallResult> {
    let result = context.array().map_err(|error| throw(error.to_string()))?;
    let length = context
        .array_len(layers)
        .map_err(|error| throw(error.to_string()))?;
    for index in start..length {
        let layer = context
            .array_get(layers, index)
            .map_err(|error| throw(error.to_string()))?;
        if property_string(context, layer, "kind").map_err(|error| throw(error.to_string()))?
            != "middleware"
        {
            continue;
        }
        let layer_path =
            property_string(context, layer, "path").map_err(|error| throw(error.to_string()))?;
        if match_path(&layer_path, path, true)?.is_none() {
            continue;
        }
        let router = context
            .get_property(layer, "router")
            .map_err(|error| throw(error.to_string()))?;
        if context.is_callable(router) {
            continue;
        }
        let handlers = context
            .get_property(layer, "handlers")
            .map_err(|error| throw(error.to_string()))?;
        for handler_index in 0..context
            .array_len(handlers)
            .map_err(|error| throw(error.to_string()))?
        {
            let handler = context
                .array_get(handlers, handler_index)
                .map_err(|error| throw(error.to_string()))?;
            if context
                .function_arity(handler)
                .map_err(|error| throw(error.to_string()))?
                == Some(4)
            {
                context
                    .array_push(result, handler)
                    .map_err(|error| throw(error.to_string()))?;
            }
        }
    }
    Ok(result)
}

fn call_handlers(
    context: &mut dyn ModuleContext,
    handlers: ValueHandle,
    request: ValueHandle,
    response: ValueHandle,
    mut error: Option<ValueHandle>,
    async_error_handlers: Option<ValueHandle>,
) -> Result<HandlerFlow, ModuleCallResult> {
    for index in 0..context
        .array_len(handlers)
        .map_err(|e| throw(e.to_string()))?
    {
        let handler = context
            .array_get(handlers, index)
            .map_err(|e| throw(e.to_string()))?;
        let arity = context
            .function_arity(handler)
            .map_err(|e| throw(e.to_string()))?;
        if (error.is_some() && arity != Some(4)) || (error.is_none() && arity == Some(4)) {
            continue;
        }
        let next = context.function(NEXT).map_err(|e| throw(e.to_string()))?;
        let no = context.bool(false).map_err(|e| throw(e.to_string()))?;
        context
            .set_private(next, NEXT_CALLED, no)
            .map_err(|e| throw(e.to_string()))?;
        let undefined = context.undefined();
        context
            .set_private(next, NEXT_ERROR, undefined)
            .map_err(|e| throw(e.to_string()))?;
        let args = if let Some(current) = error {
            vec![current, request, response, next]
        } else {
            vec![request, response, next]
        };
        match context
            .try_call(handler, undefined, &args)
            .map_err(|e| throw(e.to_string()))?
        {
            Ok(result) => {
                if matches!(
                    context
                        .value_kind(result)
                        .map_err(|e| throw(e.to_string()))?,
                    ModuleValueKind::Object | ModuleValueKind::Function
                ) {
                    let then = context
                        .get_property(result, "then")
                        .map_err(|e| throw(e.to_string()))?;
                    if context.is_callable(then) {
                        let reject = context
                            .function(ASYNC_REJECT)
                            .map_err(|e| throw(e.to_string()))?;
                        context
                            .set_private(reject, ASYNC_RESPONSE, response)
                            .map_err(|e| throw(e.to_string()))?;
                        context
                            .set_private(reject, ASYNC_REQUEST, request)
                            .map_err(|e| throw(e.to_string()))?;
                        if let Some(handlers) = async_error_handlers {
                            context
                                .set_private(reject, ASYNC_ERROR_HANDLERS, handlers)
                                .map_err(|e| throw(e.to_string()))?;
                        }
                        let undefined = context.undefined();
                        context
                            .call(then, result, &[undefined, reject])
                            .map_err(|e| throw(e.to_string()))?;
                        return Ok(HandlerFlow::Stop);
                    }
                }
            }
            Err(thrown) => {
                error = Some(thrown);
                continue;
            }
        }
        let called = context
            .get_private(next, NEXT_CALLED)
            .map_err(|e| throw(e.to_string()))?;
        if !context.as_bool(called).map_err(|e| throw(e.to_string()))? {
            return Ok(HandlerFlow::Stop);
        }
        let next_error = context
            .get_private(next, NEXT_ERROR)
            .map_err(|e| throw(e.to_string()))?;
        if context
            .value_kind(next_error)
            .map_err(|e| throw(e.to_string()))?
            == ModuleValueKind::String
        {
            let value = context
                .as_string(next_error)
                .map_err(|e| throw(e.to_string()))?;
            if value == "route" || value == "router" {
                return Ok(HandlerFlow::Unsupported(value));
            }
        }
        error = if context
            .is_truthy(next_error)
            .map_err(|e| throw(e.to_string()))?
        {
            Some(next_error)
        } else {
            None
        };
    }
    Ok(HandlerFlow::Advance(error))
}

fn express_error_response(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    error: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let error_kind = context.value_kind(error)?;
    let is_object = matches!(error_kind, ModuleValueKind::Object | ModuleValueKind::Function);
    let property = |context: &mut dyn ModuleContext, name: &str| {
        if is_object {
            context.get_property(error, name)
        } else {
            Ok(context.undefined())
        }
    };

    let mut name = property(context, "name")?;
    if context.value_kind(name)? != ModuleValueKind::String {
        name = context.string("Error")?;
    }
    let mut message = property(context, "message")?;
    if context.value_kind(message)? != ModuleValueKind::String {
        message = context.string(&context.to_string(error)?)?;
    }
    let code = property(context, "code")?;
    let stack = property(context, "stack")?;

    let details = context.object()?;
    context.set_property(details, "name", name)?;
    context.set_property(details, "message", message)?;
    if matches!(
        context.value_kind(code)?,
        ModuleValueKind::String | ModuleValueKind::Number
    ) {
        context.set_property(details, "code", code)?;
    }
    if context.value_kind(stack)? == ModuleValueKind::String {
        context.set_property(details, "stack", stack)?;
    }
    let payload = context.object()?;
    context.set_property(payload, "error", details)?;
    let encoded = context.json_stringify(payload)?;
    let encoded = context.as_string(encoded)?;

    let status = context.number(500.0)?;
    context.set_property(response, "statusCode", status)?;
    response_type(context, response, "json")?;
    response_end(context, response, Some(&encoded))
}

fn finish_response(
    context: &mut dyn ModuleContext,
    container: ValueHandle,
    request: ValueHandle,
    response: ValueHandle,
    error: Option<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    let streaming = context.get_property(response, "_expressStreaming")?;
    if context.is_truthy(streaming)? && error.is_none() {
        return Ok(return_undefined(context));
    }
    if let Some(error) = error {
        let ended = context.get_property(response, "_expressEnded")?;
        if !context.is_truthy(ended)? {
            return express_error_response(context, response, error);
        }
        return Ok(return_undefined(context));
    }
    let method = property_string(context, request, "method")?;
    let path = property_string(context, request, "path")?;
    if method == "OPTIONS"
        && !has_explicit(context, container, &path, "OPTIONS").map_err(|result| match result {
            ModuleCallResult::Throw { message, .. } => ModuleError::ContractViolation(message),
            _ => ModuleError::ContractViolation("options matching failed".into()),
        })?
    {
        let layers = context.get_private(container, LAYERS)?;
        let mut methods: Vec<String> = Vec::new();
        for index in 0..context.array_len(layers)? {
            let layer = context.array_get(layers, index)?;
            if property_string(context, layer, "kind")? != "route" {
                continue;
            }
            let layer_path = property_string(context, layer, "path")?;
            let Some(_) = match_path(&layer_path, &path, false)
                .map_err(|_| ModuleError::ContractViolation("options path decode failed".into()))?
            else {
                continue;
            };
            let method = property_string(context, layer, "method")?;
            let candidates: Vec<&str> = if method == "ALL" {
                vec!["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]
            } else if method == "GET" {
                vec!["GET", "HEAD"]
            } else {
                vec![method.as_str()]
            };
            for candidate in candidates {
                if !methods.iter().any(|value| value == candidate) {
                    methods.push(candidate.to_owned());
                }
            }
        }
        if !methods.is_empty() {
            let status = context.number(200.0)?;
            context.set_property(response, "statusCode", status)?;
            response_set(context, response, "Allow", &methods.join(", "))?;
            response_type(context, response, "html")?;
            return response_end(context, response, Some(&methods.join(",")));
        }
    }
    let ended = context.get_property(response, "_expressEnded")?;
    if !context.is_truthy(ended)? {
        let status = context.number(404.0)?;
        context.set_property(response, "statusCode", status)?;
        response_type(context, response, "text")?;
        return response_end(context, response, Some("Not Found"));
    }
    Ok(return_undefined(context))
}

fn run_container(
    context: &mut dyn ModuleContext,
    container: ValueHandle,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    if args.len() < 2 {
        return Ok(type_throw("express_m1_request_response_required"));
    }
    let request = args[0];
    let response = args[1];
    if let Err(result) = decorate_request(context, request, container) {
        return Ok(result);
    }
    decorate_response(context, response)?;
    let method = property_string(context, request, "method")?;
    let path = property_string(context, request, "path")?;
    if method == "HEAD" {
        let yes = context.bool(true)?;
        context.set_property(response, "_expressHead", yes)?;
    }
    let layers = context.get_private(container, LAYERS)?;
    let mut error = None;
    for index in 0..context.array_len(layers)? {
        let layer = context.array_get(layers, index)?;
        let kind = property_string(context, layer, "kind")?;
        let layer_path = property_string(context, layer, "path")?;
        if kind == "middleware" {
            let Some((prefix, params)) = (match match_path(&layer_path, &path, true) {
                Ok(value) => value,
                Err(result) => return Ok(result),
            }) else {
                continue;
            };
            merge_params(context, request, &params)?;
            let router = context.get_property(layer, "router")?;
            if context.is_callable(router) {
                let base = property_string(context, request, "baseUrl")?;
                set_string(
                    context,
                    request,
                    "baseUrl",
                    &format!("{}{}", base, if prefix == "/" { "" } else { &prefix }),
                )?;
                let mut rest = path[prefix.len().min(path.len())..].to_owned();
                if rest.is_empty() {
                    rest.push('/');
                } else if !rest.starts_with('/') {
                    rest.insert(0, '/');
                }
                let original_url = property_string(context, request, "url")?;
                let query = original_url
                    .split_once('?')
                    .map(|(_, q)| format!("?{q}"))
                    .unwrap_or_default();
                set_string(context, request, "url", &format!("{rest}{query}"))?;
                set_string(context, request, "path", &rest)?;
                let undefined = context.undefined();
                match context.try_call(router, undefined, &[request, response])? {
                    Ok(_) => return Ok(return_undefined(context)),
                    Err(thrown) => error = Some(thrown),
                }
                continue;
            }
            let handlers = context.get_property(layer, "handlers")?;
            let async_handlers = match later_error_handlers(context, layers, index + 1, &path) {
                Ok(value) => value,
                Err(result) => return Ok(result),
            };
            match call_handlers(
                context,
                handlers,
                request,
                response,
                error,
                Some(async_handlers),
            ) {
                Ok(HandlerFlow::Stop) => return Ok(return_undefined(context)),
                Ok(HandlerFlow::Advance(next)) => error = next,
                Ok(HandlerFlow::Unsupported(value)) => {
                    return Ok(throw(format!("Express M1 unsupported: next({value})")))
                }
                Err(result) => return Ok(result),
            }
            continue;
        }
        if error.is_some() {
            continue;
        }
        let layer_method = property_string(context, layer, "method")?;
        let allowed = if method == "HEAD" {
            let explicit = match has_explicit(context, container, &path, "HEAD") {
                Ok(value) => value,
                Err(result) => return Ok(result),
            };
            if explicit {
                layer_method == "HEAD" || layer_method == "ALL"
            } else {
                layer_method == "GET" || layer_method == "ALL"
            }
        } else if method == "OPTIONS" {
            let explicit = match has_explicit(context, container, &path, "OPTIONS") {
                Ok(value) => value,
                Err(result) => return Ok(result),
            };
            explicit && (layer_method == "OPTIONS" || layer_method == "ALL")
        } else {
            layer_method == method || layer_method == "ALL"
        };
        if !allowed {
            continue;
        }
        let Some((_, params)) = (match match_path(&layer_path, &path, false) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        }) else {
            continue;
        };
        merge_params(context, request, &params)?;
        let handlers = context.get_property(layer, "handlers")?;
        let async_handlers = match later_error_handlers(context, layers, index + 1, &path) {
            Ok(value) => value,
            Err(result) => return Ok(result),
        };
        match call_handlers(
            context,
            handlers,
            request,
            response,
            None,
            Some(async_handlers),
        ) {
            Ok(HandlerFlow::Stop) => return Ok(return_undefined(context)),
            Ok(HandlerFlow::Advance(next)) => error = next,
            Ok(HandlerFlow::Unsupported(value)) => {
                return Ok(throw(format!("Express M1 unsupported: next({value})")))
            }
            Err(result) => return Ok(result),
        }
    }
    finish_response(context, container, request, response, error)
}

impl NativeModule for ExpressModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let express = context.function(EXPRESS)?;
        attach_function(context, express, "Router", ROUTER)?;
        attach_function(context, express, "json", EXPRESS_JSON)?;
        attach_function(context, express, "assets", EXPRESS_ASSETS)?;
        for (name, message) in [
            (
                "urlencoded",
                "Express M1 unsupported: express.urlencoded data/end surface",
            ),
            ("static", "Express M1 unsupported: static files"),
        ] {
            let function = unsupported_function(context, message)?;
            context.set_property(express, name, function)?;
        }
        Ok(ModuleCallResult::Return(express))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            EXPRESS => Ok(ModuleCallResult::Return(create_container(context, false)?)),
            ROUTER => {
                if !args.is_empty() {
                    Ok(throw("Express M1 unsupported: router options"))
                } else {
                    Ok(ModuleCallResult::Return(create_container(context, true)?))
                }
            }
            EXPRESS_JSON => {
                if args.len() != 1 {
                    return Ok(type_throw(
                        "express.json requires exactly one options object",
                    ));
                }
                let mut names = match context.own_property_names(args[0]) {
                    Ok(names) => names,
                    Err(_) => return Ok(type_throw("express.json options must be an object")),
                };
                names.sort();
                if names != ["limit", "strict"] {
                    return Ok(type_throw(
                        "express.json options require exactly limit and strict",
                    ));
                }
                let limit = context.get_property(args[0], "limit")?;
                let number = match context.as_number(limit) {
                    Ok(number)
                        if number.is_finite()
                            && number.fract() == 0.0
                            && (1.0..=9_007_199_254_740_991.0).contains(&number) =>
                    {
                        number
                    }
                    _ => {
                        return Ok(type_throw(
                            "express.json limit must be a positive safe integer",
                        ));
                    }
                };
                let strict = context.get_property(args[0], "strict")?;
                if context.as_bool(strict).ok() != Some(true) {
                    return Ok(type_throw("express.json strict must be true"));
                }
                let middleware = context.function(JSON_MIDDLEWARE)?;
                let limit = context.number(number)?;
                context.set_private(middleware, JSON_LIMIT, limit)?;
                Ok(ModuleCallResult::Return(middleware))
            }
            EXPRESS_ASSETS => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
                    return Ok(type_throw(
                        "express.assets requires exactly one manifest object",
                    ));
                }
                let routes = context.own_property_names(args[0])?;
                if routes.is_empty() {
                    return Ok(type_throw("express.assets manifest cannot be empty"));
                }
                for route in routes {
                    if !route.starts_with('/') || route.contains('?') {
                        return Ok(type_throw(
                            "express.assets routes must be absolute paths without queries",
                        ));
                    }
                    let descriptor = context.get_property(args[0], &route)?;
                    if context.value_kind(descriptor)? != ModuleValueKind::Object {
                        return Ok(type_throw("express.assets descriptors must be objects"));
                    }
                    let mut names = context.own_property_names(descriptor)?;
                    names.sort();
                    if names != ["body", "cacheControl", "contentType"] {
                        return Ok(type_throw(
                            "express.assets descriptors require exactly body, cacheControl, and contentType",
                        ));
                    }
                    for name in names {
                        let value = context.get_property(descriptor, &name)?;
                        if context.value_kind(value)? != ModuleValueKind::String {
                            return Ok(type_throw(
                                "express.assets descriptor values must be strings",
                            ));
                        }
                    }
                }
                let middleware = context.function(ASSET_MIDDLEWARE)?;
                context.set_private(middleware, ASSET_MANIFEST, args[0])?;
                Ok(ModuleCallResult::Return(middleware))
            }
            ASSET_MIDDLEWARE => {
                if args.len() != 3 || !context.is_callable(args[2]) {
                    return Ok(type_throw(
                        "express.assets middleware requires req, res, and next",
                    ));
                }
                let method = property_string(context, args[0], "method")?;
                if method != "GET" && method != "HEAD" {
                    let undefined = context.undefined();
                    context.call(args[2], undefined, &[])?;
                    return Ok(return_undefined(context));
                }
                let path = property_string(context, args[0], "path")?;
                let manifest = context.get_private(callee, ASSET_MANIFEST)?;
                let descriptor = context.get_property(manifest, &path)?;
                if context.value_kind(descriptor)? == ModuleValueKind::Undefined {
                    let undefined = context.undefined();
                    context.call(args[2], undefined, &[])?;
                    return Ok(return_undefined(context));
                }
                let content_type = context.get_property(descriptor, "contentType")?;
                let cache_control = context.get_property(descriptor, "cacheControl")?;
                let body = context.get_property(descriptor, "body")?;
                let set = context.get_property(args[1], "set")?;
                let content_type_name = context.string("Content-Type")?;
                context.call(set, args[1], &[content_type_name, content_type])?;
                let set = context.get_property(args[1], "set")?;
                let cache_name = context.string("Cache-Control")?;
                context.call(set, args[1], &[cache_name, cache_control])?;
                let send = context.get_property(args[1], "send")?;
                context.call(send, args[1], &[body])?;
                Ok(return_undefined(context))
            }
            JSON_MIDDLEWARE => {
                if args.len() != 3 {
                    return Ok(type_throw(
                        "express.json middleware requires req, res, and next",
                    ));
                }
                let next = args[2];
                if !context.is_callable(next) {
                    return Ok(type_throw("express.json next must be callable"));
                }
                let request = args[0];
                let headers = context.get_property(request, "headers")?;
                if context.value_kind(headers)? != ModuleValueKind::Object {
                    return Ok(named_throw(
                        "ExpressJsonRequestError",
                        "express.json request headers are missing",
                    ));
                }
                let content_type = context.get_property(headers, "content-type")?;
                let content_type = context.as_string(content_type).unwrap_or_default();
                let media_type = content_type
                    .split(';')
                    .next()
                    .map(str::trim)
                    .unwrap_or_default();
                if !media_type.eq_ignore_ascii_case("application/json") {
                    let receiver = context.undefined();
                    context.call(next, receiver, &[])?;
                    return Ok(return_undefined(context));
                }
                let raw = context.get_property(request, "body")?;
                let raw = match context.as_string(raw) {
                    Ok(raw) => raw,
                    Err(_) => {
                        return Ok(named_throw(
                            "ExpressJsonBodyError",
                            "express.json request body must be raw text",
                        ));
                    }
                };
                if raw.is_empty() {
                    return Ok(named_throw(
                        "ExpressJsonEmptyBodyError",
                        "express.json request body is empty",
                    ));
                }
                let limit_value = context.get_private(callee, JSON_LIMIT)?;
                let limit = context.as_number(limit_value)? as usize;
                if raw.len() > limit {
                    return Ok(named_throw(
                        "ExpressJsonLimitError",
                        format!("express.json body exceeds {limit} bytes"),
                    ));
                }
                let encoded = context.string(&raw)?;
                let parsed = match context.json_parse(encoded) {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        return Ok(named_throw(
                            "ExpressJsonSyntaxError",
                            "express.json request body is malformed JSON",
                        ));
                    }
                };
                if !matches!(
                    context.value_kind(parsed)?,
                    ModuleValueKind::Object | ModuleValueKind::Array
                ) {
                    return Ok(named_throw(
                        "ExpressJsonStrictError",
                        "express.json strict mode accepts only objects or arrays",
                    ));
                }
                context.set_property(request, "body", parsed)?;
                let receiver = context.undefined();
                context.call(next, receiver, &[])?;
                Ok(return_undefined(context))
            }
            CONTAINER => run_container(context, callee, args),
            USE => {
                if args.is_empty() {
                    return Ok(type_throw("express_m1_handler_not_callable"));
                }
                let (path, start) = if context.value_kind(args[0])? == ModuleValueKind::Function {
                    ("/".to_owned(), 0)
                } else {
                    (
                        match validate_path(context, args[0]) {
                            Ok(path) => path,
                            Err(result) => return Ok(result),
                        },
                        1,
                    )
                };
                let Some(first) = args.get(start).copied() else {
                    return Ok(type_throw("express_m1_handler_not_callable"));
                };
                if context.is_callable(first) {
                    let kind = context.get_property(first, "_expressKind")?;
                    if context.value_kind(kind)? == ModuleValueKind::String {
                        match context.as_string(kind)?.as_str() {
                            "app" => return Ok(throw("Express M1 unsupported: sub-app mount")),
                            "router" => {
                                if args.len() != start + 1 {
                                    return Ok(type_throw("express_m1_handler_not_callable"));
                                }
                                let handlers = context.array()?;
                                add_layer(
                                    context,
                                    receiver,
                                    "middleware",
                                    &path,
                                    None,
                                    handlers,
                                    Some(first),
                                )?;
                                return Ok(ModuleCallResult::Return(receiver));
                            }
                            _ => {}
                        }
                    }
                }
                let handlers = match handlers_array(context, args, start) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                add_layer(context, receiver, "middleware", &path, None, handlers, None)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            key if method_name(key).is_some() => {
                if args.is_empty() {
                    return Ok(throw("Express M1 unsupported: route path shape"));
                }
                let path = match validate_path(context, args[0]) {
                    Ok(path) => path,
                    Err(result) => return Ok(result),
                };
                let handlers = match handlers_array(context, args, 1) {
                    Ok(value) => value,
                    Err(result) => return Ok(result),
                };
                add_layer(
                    context,
                    receiver,
                    "route",
                    &path,
                    method_name(key),
                    handlers,
                    None,
                )?;
                Ok(ModuleCallResult::Return(receiver))
            }
            LISTEN => {
                if args.is_empty() || context.value_kind(args[0])? != ModuleValueKind::Number {
                    return Ok(throw("Express M1 unsupported: listen port"));
                }
                if args.len() > 2 {
                    return Ok(throw("Express M1 unsupported: listen extra arguments"));
                }
                if args.len() == 2 && !context.is_callable(args[1]) {
                    return Ok(throw("Express M1 unsupported: listen callback/options"));
                }
                let http = context.import("node:http")?;
                let create_server = context.get_property(http, "createServer")?;
                let undefined = context.undefined();
                let server = context.call(create_server, undefined, &[receiver])?;
                let listen = context.get_property(server, "listen")?;
                let result = context.call(listen, server, args)?;
                Ok(ModuleCallResult::Return(result))
            }
            UNSUPPORTED => {
                let message = context.get_private(callee, UNSUPPORTED_MESSAGE)?;
                Ok(throw(context.as_string(message)?))
            }
            SETTING => {
                if args.first().is_some_and(|value| {
                    context.value_kind(*value).ok() == Some(ModuleValueKind::String)
                        && context.as_string(*value).ok().as_deref() == Some("trust proxy")
                }) {
                    Ok(throw("Express M1 unsupported: trust proxy"))
                } else {
                    Ok(throw("Express M1 unsupported: settings"))
                }
            }
            REQUEST_GET => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(throw("Express M1 response error: invalid header"));
                }
                let headers = context.get_property(receiver, "headers")?;
                let name = context.as_string(args[0])?.to_ascii_lowercase();
                Ok(ModuleCallResult::Return(
                    context.get_property(headers, &name)?,
                ))
            }
            RESPONSE_END => {
                if args.len() > 1 {
                    return Ok(throw("Express M1 response error: already ended"));
                }
                let body = args
                    .first()
                    .map(|value| context.to_string(*value))
                    .transpose()?;
                response_end(context, receiver, body.as_deref())
            }
            RESPONSE_STATUS => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Number {
                    return Ok(throw("Express M1 response error: invalid status"));
                }
                let code = context.as_number(args[0])?;
                if !(100.0..=999.0).contains(&code) {
                    return Ok(throw("Express M1 response error: invalid status"));
                }
                let streaming = context.get_property(receiver, "_expressStreaming")?;
                let ended = context.get_property(receiver, "_expressEnded")?;
                if context.is_truthy(streaming)? || context.is_truthy(ended)? {
                    return Ok(throw("Express response error: headers already committed"));
                }
                context.set_property(receiver, "statusCode", args[0])?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_SET => {
                if args.len() != 2
                    || context.value_kind(args[0])? != ModuleValueKind::String
                    || context.value_kind(args[1])? == ModuleValueKind::Undefined
                {
                    return Ok(throw("Express M1 response error: invalid header"));
                }
                let streaming = context.get_property(receiver, "_expressStreaming")?;
                if context.is_truthy(streaming)? {
                    return Ok(throw("Express response error: headers already committed"));
                }
                let name = context.as_string(args[0])?;
                let value = context.to_string(args[1])?;
                response_set(context, receiver, &name, &value)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_GET => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(throw("Express M1 response error: invalid header"));
                }
                let headers = context.get_property(receiver, "_expressHeaders")?;
                let name = context.as_string(args[0])?.to_ascii_lowercase();
                Ok(ModuleCallResult::Return(
                    context.get_property(headers, &name)?,
                ))
            }
            RESPONSE_TYPE => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(throw("Express M1 response error: invalid type"));
                }
                let name = context.as_string(args[0])?;
                response_type(context, receiver, &name)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_SEND => response_send(context, receiver, args.first().copied()),
            RESPONSE_JSON => {
                let body = args.first().copied().unwrap_or_else(|| context.undefined());
                let encoded = context.json_stringify(body)?;
                response_type(context, receiver, "json")?;
                let encoded = context.as_string(encoded)?;
                response_end(context, receiver, Some(&encoded))
            }
            RESPONSE_REDIRECT => {
                let (code, target) = match args {
                    [target] => (302.0, *target),
                    [code, target] => {
                        if context.value_kind(*code)? != ModuleValueKind::Number {
                            return Ok(throw("Express M1 response error: invalid status"));
                        }
                        (context.as_number(*code)?, *target)
                    }
                    _ => return Ok(throw("Express M1 unsupported: redirect location")),
                };
                if context.value_kind(target)? != ModuleValueKind::String {
                    return Ok(throw("Express M1 unsupported: redirect location"));
                }
                let target = context.as_string(target)?;
                if target == "back" {
                    return Ok(throw("Express M1 unsupported: redirect back"));
                }
                if !(300.0..=399.0).contains(&code) {
                    return Ok(throw("Express M1 response error: invalid status"));
                }
                let status = context.number(code)?;
                context.set_property(receiver, "statusCode", status)?;
                response_set(context, receiver, "Location", &target)?;
                response_type(context, receiver, "html")?;
                response_end(
                    context,
                    receiver,
                    Some(&format!("<p>{}</p>", escape_html(&target))),
                )
            }
            RESPONSE_FLUSH_HEADERS => {
                if !args.is_empty() {
                    return Ok(throw(
                        "Express response error: flushHeaders takes no arguments",
                    ));
                }
                response_flush_headers(context, receiver)
            }
            RESPONSE_WRITE => response_write(context, receiver, args),
            RESPONSE_ON => {
                if args.len() != 2
                    || context.value_kind(args[0])? != ModuleValueKind::String
                    || !context.is_callable(args[1])
                    || context.as_string(args[0])? != "drain"
                {
                    return Ok(throw("Express response error: unsupported response event"));
                }
                let raw = context.get_property(receiver, "_expressRawOn")?;
                context.call(raw, receiver, args)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            NEXT => {
                let yes = context.bool(true)?;
                context.set_private(callee, NEXT_CALLED, yes)?;
                let error = args.first().copied().unwrap_or_else(|| context.undefined());
                context.set_private(callee, NEXT_ERROR, error)?;
                Ok(return_undefined(context))
            }
            ASYNC_REJECT => {
                let response = context.get_private(callee, ASYNC_RESPONSE)?;
                let ended = context.get_property(response, "_expressEnded")?;
                if !context.is_truthy(ended)? {
                    let error = args.first().copied().unwrap_or_else(|| context.undefined());
                    let handlers = context.get_private(callee, ASYNC_ERROR_HANDLERS)?;
                    if context.value_kind(handlers)? == ModuleValueKind::Array
                        && context.array_len(handlers)? > 0
                    {
                        let request = context.get_private(callee, ASYNC_REQUEST)?;
                        match call_handlers(
                            context,
                            handlers,
                            request,
                            response,
                            Some(error),
                            None,
                        ) {
                            Ok(HandlerFlow::Stop) => return Ok(return_undefined(context)),
                            Ok(HandlerFlow::Advance(Some(error))) => {
                                return express_error_response(context, response, error);
                            }
                            Ok(HandlerFlow::Advance(None)) => return Ok(return_undefined(context)),
                            Ok(HandlerFlow::Unsupported(value)) => {
                                return Ok(throw(format!(
                                    "Express M1 unsupported: next({value})"
                                )));
                            }
                            Err(result) => return Ok(result),
                        }
                    }
                    return express_error_response(context, response, error);
                }
                Ok(return_undefined(context))
            }
            _ => Err(ModuleError::ContractViolation(format!(
                "unknown Express function key {}",
                key.0
            ))),
        }
    }

    fn resume(
        &self,
        _continuation: ModuleContinuation,
        _state: &[ValueHandle],
        _completion: Result<ValueHandle, String>,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "Express does not own host continuations".into(),
        ))
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "Express does not receive host events directly".into(),
        ))
    }
}
