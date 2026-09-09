//! Rust-native `node:http` module implemented only through `jjs-module-api`.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleObjectKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
use jjs_module_node_buffer as buffer;
pub mod input;
use jjs_module_node_events as listeners;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HTTP_REQUEST_EVENT: u32 = 1;
pub const HTTP_RESPONSE_EVENT: u32 = 2;
pub const HTTP_DRAIN_EVENT: u32 = 3;
pub const HTTP_CLOSE_EVENT: u32 = 4;
pub const HTTP_STREAM_CONTRACT_VERSION: u16 = 2;
const CREATE_SERVER: ModuleFunctionKey = ModuleFunctionKey(1);
const SERVER_LISTEN: ModuleFunctionKey = ModuleFunctionKey(2);
const REQUEST_ON: ModuleFunctionKey = ModuleFunctionKey(3);
const RESPONSE_SET_HEADER: ModuleFunctionKey = ModuleFunctionKey(4);
const RESPONSE_END: ModuleFunctionKey = ModuleFunctionKey(5);
const RESPONSE_FLUSH_HEADERS: ModuleFunctionKey = ModuleFunctionKey(6);
const RESPONSE_WRITE: ModuleFunctionKey = ModuleFunctionKey(7);
const RESPONSE_ON: ModuleFunctionKey = ModuleFunctionKey(8);
const RESPONSE_WRITE_HEAD: ModuleFunctionKey = ModuleFunctionKey(9);
const REQUEST_SET_ENCODING: ModuleFunctionKey = ModuleFunctionKey(10);
const COMMITTED_STATUS: u32 = 16;
const TEXT_ENCODING: u32 = 17;
const DATA_STARTED: u32 = 18;
const SERVER: ModuleObjectKind = ModuleObjectKind(1);
const REQUEST: ModuleObjectKind = ModuleObjectKind(2);
const RESPONSE: ModuleObjectKind = ModuleObjectKind(3);
const LISTENING: u32 = 2;
const LIFECYCLE: u32 = 5;
const HEADERS_JSON: u32 = 6;
const BODY: u32 = 7;
const REQUEST_HANDLE: u32 = 10;
const CONNECTION_ID: u32 = 11;
const REQUEST_ID: u32 = 12;
const SEQUENCE: u32 = 13;
const CLOSE_DELIVERED: u32 = 14;
const BODY_BYTES: u32 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpResponseLifecycle {
    Buffered,
    Streaming,
    Ended,
    Closed,
}

impl HttpResponseLifecycle {
    fn as_private(self) -> &'static str {
        match self {
            Self::Buffered => "buffered",
            Self::Streaming => "streaming",
            Self::Ended => "ended",
            Self::Closed => "closed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpStreamEventV2 {
    Drain {
        version: u16,
        connection_id: String,
        request_id: String,
        sequence: u64,
    },
    Close {
        version: u16,
        connection_id: String,
        request_id: String,
        reason: String,
        sequence: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpStreamActionV2 {
    Start {
        version: u16,
        connection_id: String,
        request_id: String,
        status: u16,
        headers: Vec<(String, String)>,
        sequence: u64,
    },
    Write {
        version: u16,
        connection_id: String,
        request_id: String,
        bytes: Vec<u8>,
        sequence: u64,
    },
    End {
        version: u16,
        connection_id: String,
        request_id: String,
        optional_bytes: Option<Vec<u8>>,
        sequence: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpStreamContractError {
    HeadersCommitted,
    RepeatedStart,
    StreamClosed,
    DrainBeforeStreaming,
    UnknownConnection,
    UnknownRequest,
    UnsupportedVersion,
    OutOfOrder { expected: u64, actual: u64 },
}

/// Deterministic logical state for one host-owned streaming HTTP response.
/// It contains ids and ordered actions, never a socket or other host handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponseStreamV2 {
    connection_id: String,
    request_id: String,
    status: u16,
    headers: Vec<(String, String)>,
    lifecycle: HttpResponseLifecycle,
    next_sequence: u64,
    actions: Vec<HttpStreamActionV2>,
}

impl HttpResponseStreamV2 {
    pub fn new(connection_id: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            connection_id: connection_id.into(),
            request_id: request_id.into(),
            status: 200,
            headers: Vec::new(),
            lifecycle: HttpResponseLifecycle::Buffered,
            next_sequence: 1,
            actions: Vec::new(),
        }
    }

    pub fn lifecycle(&self) -> HttpResponseLifecycle {
        self.lifecycle
    }

    pub fn actions(&self) -> &[HttpStreamActionV2] {
        &self.actions
    }

    pub fn set_status(&mut self, status: u16) -> Result<(), HttpStreamContractError> {
        self.require_buffered()?;
        self.status = status;
        Ok(())
    }

    pub fn set_header(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), HttpStreamContractError> {
        self.require_buffered()?;
        let name = name.into().to_ascii_lowercase();
        self.headers.retain(|(n, _)| n != &name);
        self.headers.push((name, value.into()));
        Ok(())
    }

    pub fn flush_headers(&mut self) -> Result<(), HttpStreamContractError> {
        match self.lifecycle {
            HttpResponseLifecycle::Buffered => self.start(),
            HttpResponseLifecycle::Streaming => Err(HttpStreamContractError::RepeatedStart),
            HttpResponseLifecycle::Ended | HttpResponseLifecycle::Closed => {
                Err(HttpStreamContractError::StreamClosed)
            }
        }
    }

    pub fn write(&mut self, chunk: impl Into<String>) -> Result<(), HttpStreamContractError> {
        if self.lifecycle == HttpResponseLifecycle::Buffered {
            self.start()?;
        }
        if self.lifecycle != HttpResponseLifecycle::Streaming {
            return Err(HttpStreamContractError::StreamClosed);
        }
        let sequence = self.take_sequence();
        self.actions.push(HttpStreamActionV2::Write {
            version: HTTP_STREAM_CONTRACT_VERSION,
            connection_id: self.connection_id.clone(),
            request_id: self.request_id.clone(),
            bytes: chunk.into().into_bytes(),
            sequence,
        });
        Ok(())
    }

    /// End a streaming response. Buffered responses continue to use the
    /// existing `HttpResponse` encoding path and therefore emit no actions.
    pub fn end(&mut self, optional_chunk: Option<String>) -> Result<(), HttpStreamContractError> {
        match self.lifecycle {
            HttpResponseLifecycle::Buffered => {
                self.lifecycle = HttpResponseLifecycle::Ended;
                Ok(())
            }
            HttpResponseLifecycle::Streaming => {
                let sequence = self.take_sequence();
                self.actions.push(HttpStreamActionV2::End {
                    version: HTTP_STREAM_CONTRACT_VERSION,
                    connection_id: self.connection_id.clone(),
                    request_id: self.request_id.clone(),
                    optional_bytes: optional_chunk.map(String::into_bytes),
                    sequence,
                });
                self.lifecycle = HttpResponseLifecycle::Ended;
                Ok(())
            }
            HttpResponseLifecycle::Ended | HttpResponseLifecycle::Closed => {
                Err(HttpStreamContractError::StreamClosed)
            }
        }
    }

    pub fn deliver_event(
        &mut self,
        event: HttpStreamEventV2,
    ) -> Result<(), HttpStreamContractError> {
        let (version, connection_id, request_id, sequence, close) = match event {
            HttpStreamEventV2::Drain {
                version,
                connection_id,
                request_id,
                sequence,
            } => (version, connection_id, request_id, sequence, false),
            HttpStreamEventV2::Close {
                version,
                connection_id,
                request_id,
                sequence,
                ..
            } => (version, connection_id, request_id, sequence, true),
        };
        if version != HTTP_STREAM_CONTRACT_VERSION {
            return Err(HttpStreamContractError::UnsupportedVersion);
        }
        if connection_id != self.connection_id {
            return Err(HttpStreamContractError::UnknownConnection);
        }
        if request_id != self.request_id {
            return Err(HttpStreamContractError::UnknownRequest);
        }
        if sequence != self.next_sequence {
            return Err(HttpStreamContractError::OutOfOrder {
                expected: self.next_sequence,
                actual: sequence,
            });
        }
        if self.lifecycle == HttpResponseLifecycle::Ended
            || self.lifecycle == HttpResponseLifecycle::Closed
        {
            return Err(HttpStreamContractError::StreamClosed);
        }
        if !close && self.lifecycle != HttpResponseLifecycle::Streaming {
            return Err(HttpStreamContractError::DrainBeforeStreaming);
        }
        self.next_sequence += 1;
        if close {
            self.lifecycle = HttpResponseLifecycle::Closed;
        }
        Ok(())
    }

    fn require_buffered(&self) -> Result<(), HttpStreamContractError> {
        if self.lifecycle == HttpResponseLifecycle::Buffered {
            Ok(())
        } else if self.lifecycle == HttpResponseLifecycle::Streaming {
            Err(HttpStreamContractError::HeadersCommitted)
        } else {
            Err(HttpStreamContractError::StreamClosed)
        }
    }

    fn start(&mut self) -> Result<(), HttpStreamContractError> {
        self.require_buffered()?;
        let sequence = self.take_sequence();
        self.actions.push(HttpStreamActionV2::Start {
            version: HTTP_STREAM_CONTRACT_VERSION,
            connection_id: self.connection_id.clone(),
            request_id: self.request_id.clone(),
            status: self.status,
            headers: self.headers.clone(),
            sequence,
        });
        self.lifecycle = HttpResponseLifecycle::Streaming;
        Ok(())
    }

    fn take_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub client: HttpClient,
    pub received_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HttpClient {
    pub id: String,
    pub authenticated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_bytes: Option<Vec<u8>>,
}

pub fn decode_response(value: &str) -> Result<HttpResponse, serde_json::Error> {
    serde_json::from_str(value)
}

pub struct NodeHttpModule {
    manifest: ModuleManifest,
}

impl Default for NodeHttpModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-http".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-node-http-v4".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 5,
                imports: vec!["http".into(), "node:http".into()],
                capabilities: vec![
                    HostCapabilityDescriptor {
                        id: "jjs:net/listen".into(),
                        contract_version: 1,
                        completion: CompletionMode::Sync,
                        schema: "jjs.net.listen.v1".into(),
                    },
                    HostCapabilityDescriptor {
                        id: "jjs:http/stream".into(),
                        contract_version: 2,
                        completion: CompletionMode::Sync,
                        schema: "jjs.http.stream.v2".into(),
                    },
                ],
                dependencies: vec![jjs_module_api::ModuleDependency {
                    id: "org.jjs.node-buffer".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-buffer-v1".into(),
                }],
                function_keys: vec![
                    CREATE_SERVER.0,
                    SERVER_LISTEN.0,
                    REQUEST_ON.0,
                    RESPONSE_SET_HEADER.0,
                    RESPONSE_END.0,
                    RESPONSE_FLUSH_HEADERS.0,
                    RESPONSE_WRITE.0,
                    RESPONSE_ON.0,
                    RESPONSE_WRITE_HEAD.0,
                    REQUEST_SET_ENCODING.0,
                    11,
                    12,
                    13,
                    14,
                    15,
                    100,
                    101,
                    102,
                    103,
                    104,
                    105,
                    106,
                    107,
                ],
                object_kind_keys: vec![SERVER.0, REQUEST.0, RESPONSE.0],
                deterministic_resources: vec![],
            },
        }
    }
}

fn thrown(code: &str) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "TypeError".into(),
        message: code.into(),
    }
}

fn response_lifecycle(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<String, ModuleError> {
    let lifecycle = context.get_private(response, LIFECYCLE)?;
    context.as_string(lifecycle)
}

fn set_response_lifecycle(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    lifecycle: HttpResponseLifecycle,
) -> Result<(), ModuleError> {
    let lifecycle = context.string(lifecycle.as_private())?;
    context.set_private(response, LIFECYCLE, lifecycle)
}

fn stream_request(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    operation: &str,
    extra: Vec<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    let connection_id = context.get_private(response, CONNECTION_ID)?;
    let request_id = context.get_private(response, REQUEST_ID)?;
    if context.value_kind(connection_id)? != jjs_module_api::ModuleValueKind::String
        || context.value_kind(request_id)? != jjs_module_api::ModuleValueKind::String
    {
        return Ok(thrown("node_http_stream_ids_required"));
    }
    let sequence = context.get_private(response, SEQUENCE)?;
    let sequence_number = context.as_number(sequence)?;
    let version = context.number(HTTP_STREAM_CONTRACT_VERSION.into())?;
    let operation = context.string(operation)?;
    let mut arguments = vec![version, operation, connection_id, request_id, sequence];
    arguments.extend(extra);
    let result = context.request_host(
        HostRequestSpec {
            capability: "jjs:http/stream".into(),
            operation: "action".into(),
            arguments,
        },
        ModuleContinuation(2),
        vec![response],
        false,
    )?;
    if matches!(result, ModuleCallResult::Return(_)) {
        let next = context.number(sequence_number + 1.0)?;
        context.set_private(response, SEQUENCE, next)?;
    }
    Ok(result)
}

type Headers = Vec<(String, String)>;
fn invalid(s: &str) -> ModuleError {
    ModuleError::ContractViolation(s.into())
}
fn read_headers(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<Headers, ModuleError> {
    let v = c.get_private(r, HEADERS_JSON)?;
    serde_json::from_str(&c.as_string(v)?).map_err(|_| invalid("node_http_headers_invalid"))
}
fn validate_headers(headers: &Headers) -> Result<(), ModuleError> {
    if headers.len() > 256
        || headers
            .iter()
            .map(|(n, v)| n.len() + v.len())
            .sum::<usize>()
            > 65536
    {
        return Err(invalid("node_http_headers_limit"));
    }
    for (n, v) in headers {
        if n.is_empty()
            || n.len() > 256
            || !n
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        {
            return Err(invalid("node_http_header_name_invalid"));
        }
        if v.len() > 16384 || !v.bytes().all(|b| b == b'\t' || (32..=126).contains(&b)) {
            return Err(invalid("node_http_header_value_invalid"));
        }
    }
    Ok(())
}
fn save_headers(c: &mut dyn ModuleContext, r: ValueHandle, h: &Headers) -> Result<(), ModuleError> {
    validate_headers(h)?;
    let v =
        c.string(&serde_json::to_string(h).map_err(|_| invalid("node_http_headers_invalid"))?)?;
    c.set_private(r, HEADERS_JSON, v)
}
fn replace_header(
    c: &mut dyn ModuleContext,
    h: &mut Headers,
    n: &str,
    v: ValueHandle,
) -> Result<(), ModuleError> {
    let mut values = Vec::new();
    if c.value_kind(v)? == jjs_module_api::ModuleValueKind::Array {
        for i in 0..c.array_len(v)? {
            let v = c.array_get(v, i)?;
            values.push(
                c.as_string(v)
                    .map_err(|_| invalid("node_http_header_value_invalid"))?,
            );
        }
        if values.is_empty() {
            return Err(invalid("node_http_header_values_empty"));
        }
    } else {
        values.push(
            c.as_string(v)
                .map_err(|_| invalid("node_http_header_value_invalid"))?,
        );
    }
    h.retain(|(name, _)| !name.eq_ignore_ascii_case(n));
    h.extend(values.into_iter().map(|v| (n.to_ascii_lowercase(), v)));
    validate_headers(h)
}
fn committed(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<bool, ModuleError> {
    let v = c.get_private(r, COMMITTED_STATUS)?;
    Ok(c.value_kind(v)? != jjs_module_api::ModuleValueKind::Undefined)
}
fn commit(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<(), ModuleError> {
    if committed(c, r)? {
        return Ok(());
    }
    let status = c.get_property(r, "statusCode")?;
    let n = c.as_number(status)?;
    if !(200.0..=599.0).contains(&n) || n.fract() != 0.0 {
        return Err(invalid("node_http_final_status_unsupported"));
    }
    let mut h = read_headers(c, r)?;
    if n == 204.0 || n == 304.0 {
        h.retain(|(n, _)| n != "content-length" && n != "transfer-encoding");
    }
    save_headers(c, r, &h)?;
    c.set_private(r, COMMITTED_STATUS, status)?;
    let yes = c.bool(true)?;
    c.set_property(r, "headersSent", yes)
}
fn body_allowed(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<bool, ModuleError> {
    let request = c.get_private(r, REQUEST_HANDLE)?;
    let method = c.get_property(request, "method")?;
    let status = c.get_private(r, COMMITTED_STATUS)?;
    let status = c.as_number(status)?;
    Ok(c.as_string(method)? != "HEAD" && status != 204.0 && status != 304.0)
}
fn chunk_bytes(c: &mut dyn ModuleContext, v: ValueHandle) -> Result<Vec<u8>, ModuleError> {
    if c.is_bytes(v) {
        c.read_bytes(v)
    } else if c.value_kind(v)? == jjs_module_api::ModuleValueKind::String {
        Ok(c.as_string(v)?.into_bytes())
    } else {
        Err(invalid("node_http_chunk_requires_string_or_buffer"))
    }
}
fn stream_chunk(
    c: &mut dyn ModuleContext,
    r: ValueHandle,
    v: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let bytes = chunk_bytes(c, v)?;
    let bytes = if body_allowed(c, r)? {
        bytes.as_slice()
    } else {
        &[]
    };
    c.string(&buffer::decode(bytes, "base64")?)
}
fn request_headers(
    c: &mut dyn ModuleContext,
    r: ValueHandle,
    h: &Headers,
) -> Result<(), ModuleError> {
    validate_headers(h)?;
    let raw = c.array()?;
    let normalized = c.object()?;
    let distinct = c.object()?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (n, v) in h {
        let name = c.string(n)?;
        c.array_push(raw, name)?;
        let value = c.string(v)?;
        c.array_push(raw, value)?;
        groups
            .entry(n.to_ascii_lowercase())
            .or_default()
            .push(v.clone());
    }
    for (n, vs) in groups {
        let all = c.array()?;
        for v in &vs {
            let v = c.string(v)?;
            c.array_push(all, v)?;
        }
        c.set_property(distinct, &n, all)?;
        let value = if n == "set-cookie" {
            all
        } else {
            let singleton = matches!(
                n.as_str(),
                "age"
                    | "authorization"
                    | "content-length"
                    | "content-type"
                    | "etag"
                    | "expires"
                    | "from"
                    | "host"
                    | "if-modified-since"
                    | "if-unmodified-since"
                    | "last-modified"
                    | "location"
                    | "max-forwards"
                    | "proxy-authorization"
                    | "referer"
                    | "retry-after"
                    | "server"
                    | "user-agent"
            );
            c.string(&if singleton {
                vs[0].clone()
            } else {
                vs.join(if n == "cookie" { "; " } else { ", " })
            })?
        };
        c.set_property(normalized, &n, value)?;
    }
    c.set_property(r, "rawHeaders", raw)?;
    c.set_property(r, "headersDistinct", distinct)?;
    c.set_property(r, "headers", normalized)
}

fn start_stream(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    match response_lifecycle(context, response)?.as_str() {
        "buffered" => {}
        "streaming" => return Ok(thrown("node_http_response_already_streaming")),
        _ => return Ok(thrown("node_http_response_stream_closed")),
    }
    commit(context, response)?;
    let status = context.get_private(response, COMMITTED_STATUS)?;
    let headers = context.get_private(response, HEADERS_JSON)?;
    let result = stream_request(context, response, "start", vec![status, headers])?;
    if matches!(result, ModuleCallResult::Return(_)) {
        set_response_lifecycle(context, response, HttpResponseLifecycle::Streaming)?;
        let yes = context.bool(true)?;
        context.set_property(response, "headersSent", yes)?;
    }
    Ok(result)
}

fn validate_stream_event(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
    payload: ValueHandle,
) -> Result<(), ModuleError> {
    let version = context.get_property(payload, "version")?;
    if context.as_number(version)? != f64::from(HTTP_STREAM_CONTRACT_VERSION) {
        return Err(ModuleError::ContractViolation(
            "node_http_stream_event_version_invalid".into(),
        ));
    }
    let expected_connection = context.get_private(response, CONNECTION_ID)?;
    let actual_connection = context.get_property(payload, "connectionId")?;
    if context.as_string(expected_connection)? != context.as_string(actual_connection)? {
        return Err(ModuleError::ContractViolation(
            "node_http_stream_event_connection_unknown".into(),
        ));
    }
    let expected_request = context.get_private(response, REQUEST_ID)?;
    let actual_request = context.get_property(payload, "requestId")?;
    if context.as_string(expected_request)? != context.as_string(actual_request)? {
        return Err(ModuleError::ContractViolation(
            "node_http_stream_event_request_unknown".into(),
        ));
    }
    let expected_sequence = context.get_private(response, SEQUENCE)?;
    let expected_sequence = context.as_number(expected_sequence)?;
    let actual_sequence = context.get_property(payload, "sequence")?;
    let actual_sequence = context.as_number(actual_sequence)?;
    if actual_sequence != expected_sequence {
        return Err(ModuleError::ContractViolation(format!(
            "node_http_stream_event_out_of_order: expected {expected_sequence}, got {actual_sequence}"
        )));
    }
    let next = context.number(expected_sequence + 1.0)?;
    context.set_private(response, SEQUENCE, next)
}

fn encode_response(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let lifecycle = context.get_private(response, LIFECYCLE)?;
    if context.as_string(lifecycle)? != HttpResponseLifecycle::Ended.as_private() {
        return Err(ModuleError::ContractViolation(
            "node_http_response_not_ended".into(),
        ));
    }
    commit(context, response)?;
    let status = context.get_private(response, COMMITTED_STATUS)?;
    let status = context.as_number(status)? as u16;
    let headers = context.get_private(response, HEADERS_JSON)?;
    let headers = context.as_string(headers)?;
    let headers: Headers = serde_json::from_str(&headers)
        .map_err(|_| ModuleError::ContractViolation("node_http_response_headers_invalid".into()))?;
    let body = context.get_private(response, BODY)?;
    let body = context.as_string(body)?;
    let body_bytes = context.get_private(response, BODY_BYTES)?;
    let body_bytes =
        if context.value_kind(body_bytes)? == jjs_module_api::ModuleValueKind::Undefined {
            None
        } else {
            Some(context.read_bytes(body_bytes)?)
        };
    let (body, body_bytes) = if body_allowed(context, response)? {
        (body, body_bytes)
    } else {
        (String::new(), None)
    };
    let encoded = serde_json::to_string(&HttpResponse {
        status,
        headers,
        body,
        body_bytes,
    })
    .map_err(|_| ModuleError::ContractViolation("node_http_response_invalid".into()))?;
    let encoded = context.string(&encoded)?;
    Ok(ModuleCallResult::Return(encoded))
}

impl NativeModule for NodeHttpModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let create_server = context.function(CREATE_SERVER)?;
        context.set_property(exports, "createServer", create_server)?;
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if (11..=15).contains(&key.0) {
            return input::call(context, key, callee, receiver, args);
        }
        if listeners::FUNCTION_KEYS.contains(&key.0) {
            input::listener_added(context, receiver, key, args)?;
            let out = listeners::call(context, key, receiver, args)?;
            if matches!(key, listeners::ON | listeners::ONCE)
                && matches!(out, ModuleCallResult::Return(_))
                && args.len() == 2
                && context.as_string(args[0])? == "data"
                && input::enabled(context, receiver)?
            {
                let drained = input::drain(context, receiver)?;
                if !matches!(drained, ModuleCallResult::Return(_)) {
                    return Ok(drained);
                }
            }
            return Ok(out);
        }
        let result = (|| match key {
            CREATE_SERVER => {
                if args.len() > 1
                    || args
                        .first()
                        .is_some_and(|handler| !context.is_callable(*handler))
                {
                    return Ok(thrown("node_http_create_server_handler_not_callable"));
                }
                let server = context.module_object(SERVER)?;
                listeners::install(context, server, &["request", "listening", "error", "close"])?;
                if let Some(handler) = args.first() {
                    let event = context.string("request")?;
                    let registered =
                        listeners::call(context, listeners::ON, server, &[event, *handler])?;
                    if !matches!(registered, ModuleCallResult::Return(_)) {
                        return Ok(registered);
                    }
                }
                let not_listening = context.bool(false)?;
                context.set_private(server, LISTENING, not_listening)?;
                context.set_property(server, "listening", not_listening)?;
                let listen = context.function(SERVER_LISTEN)?;
                context.set_property(server, "listen", listen)?;
                Ok(ModuleCallResult::Return(server))
            }
            SERVER_LISTEN => {
                if args.is_empty() || args.len() > 2 {
                    return Ok(thrown("node_http_listen_arity_invalid"));
                }
                let port = match context.as_number(args[0]) {
                    Ok(value) if value >= 0.0 && value <= 65535.0 && value.fract() == 0.0 => value,
                    _ => return Ok(thrown("node_http_listen_port_invalid")),
                };
                if args.len() == 2 && !context.is_callable(args[1]) {
                    return Ok(thrown("node_http_listen_callback_not_callable"));
                }
                let listening = context.get_private(receiver, LISTENING)?;
                if context.as_bool(listening)? {
                    return Ok(thrown("node_http_server_already_listening"));
                }
                let port_handle = context.number(port)?;
                let request = HostRequestSpec {
                    capability: "jjs:net/listen".into(),
                    operation: "listen".into(),
                    arguments: vec![port_handle, receiver],
                };
                match context.request_host(request, ModuleContinuation(1), vec![receiver], false)? {
                    ModuleCallResult::Return(assigned_port) => {
                        let assigned_port = context.as_number(assigned_port).map_err(|_| {
                            ModuleError::ContractViolation(
                                "jjs:net/listen must return the assigned port".into(),
                            )
                        })?;
                        if assigned_port < 1.0
                            || assigned_port > 65535.0
                            || assigned_port.fract() != 0.0
                        {
                            return Err(ModuleError::ContractViolation(
                                "jjs:net/listen returned an invalid assigned port".into(),
                            ));
                        }
                        let assigned_port = context.number(assigned_port)?;
                        context.set_property(receiver, "port", assigned_port)?;
                        let listening = context.bool(true)?;
                        context.set_private(receiver, LISTENING, listening)?;
                        context.set_property(receiver, "listening", listening)?;
                        if args.len() == 2 {
                            let event = context.string("listening")?;
                            let registered = listeners::call(
                                context,
                                listeners::ONCE,
                                receiver,
                                &[event, args[1]],
                            )?;
                            if !matches!(registered, ModuleCallResult::Return(_)) {
                                return Ok(registered);
                            }
                        }
                        let emitted = listeners::emit(context, receiver, "listening", &[])?;
                        if !matches!(emitted, ModuleCallResult::Return(_)) {
                            return Ok(emitted);
                        }
                        Ok(ModuleCallResult::Return(receiver))
                    }
                    other => Ok(other),
                }
            }
            REQUEST_ON | RESPONSE_ON => listeners::call(context, listeners::ON, receiver, args),
            REQUEST_SET_ENCODING => {
                if args.len() != 1 {
                    return Ok(thrown("node_http_set_encoding_arity_invalid"));
                }
                let started = context.get_private(receiver, DATA_STARTED)?;
                if context.as_bool(started)? {
                    return Ok(thrown("node_http_set_encoding_after_data_unsupported"));
                }
                let enc = buffer::encoding(&context.as_string(args[0])?)?;
                if !matches!(enc, "utf8" | "latin1" | "ascii") {
                    return Ok(thrown("node_http_stream_encoding_unsupported"));
                }
                let enc = context.string(enc)?;
                context.set_private(receiver, TEXT_ENCODING, enc)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_SET_HEADER => {
                if args.len() != 2 {
                    return Ok(thrown("node_http_set_header_arity_invalid"));
                }
                if committed(context, receiver)? {
                    return Ok(thrown("node_http_response_headers_committed"));
                }
                let name = context
                    .as_string(args[0])
                    .map_err(|_| invalid("node_http_header_name_invalid"))?;
                let mut headers = read_headers(context, receiver)?;
                replace_header(context, &mut headers, &name, args[1])?;
                save_headers(context, receiver, &headers)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_WRITE_HEAD => {
                if args.is_empty() || args.len() > 3 {
                    return Ok(thrown("node_http_write_head_arity_invalid"));
                }
                if committed(context, receiver)? {
                    return Ok(thrown("node_http_response_headers_committed"));
                }
                let status = match context.as_number(args[0]) {
                    Ok(value) if (200.0..=599.0).contains(&value) && value.fract() == 0.0 => value,
                    _ => return Ok(thrown("node_http_write_head_status_invalid")),
                };
                let headers = match args.len() {
                    1 => None,
                    2 if context.value_kind(args[1])?
                        == jjs_module_api::ModuleValueKind::String =>
                    {
                        None
                    }
                    2 => Some(args[1]),
                    3 if context.value_kind(args[1])?
                        == jjs_module_api::ModuleValueKind::String =>
                    {
                        Some(args[2])
                    }
                    _ => return Ok(thrown("node_http_write_head_status_message_invalid")),
                };
                let encoded_headers = context.get_private(receiver, HEADERS_JSON)?;
                let encoded_headers = context.as_string(encoded_headers)?;
                let mut encoded_headers: Headers =
                    serde_json::from_str(&encoded_headers).map_err(|_| {
                        ModuleError::ContractViolation("node_http_response_headers_invalid".into())
                    })?;
                if let Some(headers) = headers {
                    let names = context.own_property_names(headers).map_err(|_| {
                        ModuleError::ContractViolation(
                            "node_http_write_head_headers_invalid".into(),
                        )
                    })?;
                    for name in names {
                        let value = context.get_property(headers, &name)?;
                        replace_header(context, &mut encoded_headers, &name, value)?;
                    }
                }
                let status = context.number(status)?;
                context.set_property(receiver, "statusCode", status)?;
                let encoded_headers = serde_json::to_string(&encoded_headers).map_err(|_| {
                    ModuleError::ContractViolation("node_http_response_headers_invalid".into())
                })?;
                let encoded_headers = context.string(&encoded_headers)?;
                context.set_private(receiver, HEADERS_JSON, encoded_headers)?;
                commit(context, receiver)?;
                if input::enabled(context, receiver)? {
                    return start_stream(context, receiver);
                }
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_END => {
                if args.len() > 1 {
                    return Ok(thrown("node_http_response_end_arity_invalid"));
                }
                if input::enabled(context, receiver)?
                    && response_lifecycle(context, receiver)? == "buffered"
                {
                    let result = start_stream(context, receiver)?;
                    if !matches!(result, ModuleCallResult::Return(_)) {
                        return Ok(result);
                    }
                }
                let bytes = if let Some(v) = args.first() {
                    Some(chunk_bytes(context, *v)?)
                } else {
                    None
                };
                match response_lifecycle(context, receiver)?.as_str() {
                    "buffered" => {
                        commit(context, receiver)?;
                        if let Some(bytes) = bytes {
                            let body = buffer::from_bytes(context, &bytes)?;
                            context.set_private(receiver, BODY_BYTES, body)?;
                        }
                        set_response_lifecycle(context, receiver, HttpResponseLifecycle::Ended)?;
                        Ok(ModuleCallResult::Return(receiver))
                    }
                    "streaming" => {
                        let chunk = if let Some(v) = args.first() {
                            stream_chunk(context, receiver, *v)?
                        } else {
                            context.undefined()
                        };
                        let result = stream_request(context, receiver, "end", vec![chunk])?;
                        if matches!(result, ModuleCallResult::Return(_)) {
                            set_response_lifecycle(
                                context,
                                receiver,
                                HttpResponseLifecycle::Ended,
                            )?;
                        }
                        Ok(result)
                    }
                    _ => Ok(thrown("node_http_response_already_ended")),
                }
            }
            RESPONSE_FLUSH_HEADERS => {
                if !args.is_empty() {
                    return Ok(thrown("node_http_flush_headers_arity_invalid"));
                }
                let result = start_stream(context, receiver)?;
                if matches!(result, ModuleCallResult::Return(_)) {
                    let undefined = context.undefined();
                    Ok(ModuleCallResult::Return(undefined))
                } else {
                    Ok(result)
                }
            }
            RESPONSE_WRITE => {
                if args.len() != 1 {
                    return Ok(thrown("node_http_write_arity_invalid"));
                }
                chunk_bytes(context, args[0])?;
                if response_lifecycle(context, receiver)? == "buffered" {
                    let started = start_stream(context, receiver)?;
                    if !matches!(started, ModuleCallResult::Return(_)) {
                        return Ok(started);
                    }
                }
                if response_lifecycle(context, receiver)? != "streaming" {
                    return Ok(thrown("node_http_response_stream_closed"));
                }
                let chunk = stream_chunk(context, receiver, args[0])?;
                stream_request(context, receiver, "write", vec![chunk])
            }
            _ => Err(ModuleError::ContractViolation(format!(
                "unknown node:http function key {}",
                key.0
            ))),
        })();
        match result {
            Err(ModuleError::ContractViolation(message))
                if message.starts_with("node_http_header_")
                    || message == "node_http_headers_limit"
                    || message == "node_http_final_status_unsupported"
                    || message == "node_http_chunk_requires_string_or_buffer"
                    || message.starts_with("buffer_encoding_") =>
            {
                Ok(thrown(&message))
            }
            other => other,
        }
    }

    fn resume(
        &self,
        _continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match completion {
            Ok(_) => state
                .first()
                .copied()
                .map(ModuleCallResult::Return)
                .ok_or_else(|| {
                    ModuleError::ContractViolation("node:http continuation state is missing".into())
                }),
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "Error".into(),
                message,
            }),
        }
    }

    fn event(
        &self,
        event: u32,
        target: ValueHandle,
        payload: ValueHandle,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if (input::DATA..=input::STATE).contains(&event) {
            return input::event(context, target, event, payload);
        }
        if event == HTTP_RESPONSE_EVENT {
            return encode_response(context, payload);
        }
        if event == HTTP_DRAIN_EVENT {
            if response_lifecycle(context, target)? != "streaming" {
                return Err(ModuleError::ContractViolation(
                    "node_http_drain_requires_streaming_response".into(),
                ));
            }
            validate_stream_event(context, target, payload)?;
            let emitted = input::emit(context, target, "drain", &[])?;
            if !matches!(emitted, ModuleCallResult::Return(_)) {
                return Ok(emitted);
            }
            return Ok(ModuleCallResult::Return(target));
        }
        if event == HTTP_CLOSE_EVENT {
            let delivered = context.get_private(target, CLOSE_DELIVERED)?;
            if context.as_bool(delivered)? {
                return Err(ModuleError::ContractViolation(
                    "node_http_close_already_delivered".into(),
                ));
            }
            validate_stream_event(context, target, payload)?;
            let yes = context.bool(true)?;
            context.set_private(target, CLOSE_DELIVERED, yes)?;
            set_response_lifecycle(context, target, HttpResponseLifecycle::Closed)?;
            let request = context.get_private(target, REQUEST_HANDLE)?;
            let emitted = input::close(context, request)?;
            if !matches!(emitted, ModuleCallResult::Return(_)) {
                return Ok(emitted);
            }
            let emitted = input::emit(context, request, "close", &[])?;
            if !matches!(emitted, ModuleCallResult::Return(_)) {
                return Ok(emitted);
            }
            let emitted = input::emit(context, target, "close", &[])?;
            if !matches!(emitted, ModuleCallResult::Return(_)) {
                return Ok(emitted);
            }
            return Ok(ModuleCallResult::Return(target));
        }
        if event != HTTP_REQUEST_EVENT && event != input::START {
            return Err(ModuleError::ContractViolation(
                "unsupported node:http event".into(),
            ));
        }
        let request = context.module_object(REQUEST)?;
        for name in ["method", "url", "client"] {
            let value = context.get_property(payload, name)?;
            context.set_property(request, name, value)?;
        }
        let raw = context.get_property(payload, "headersJson")?;
        let headers: Headers = serde_json::from_str(&context.as_string(raw)?)
            .map_err(|_| invalid("node_http_request_headers_invalid"))?;
        request_headers(context, request, &headers)?;
        let raw = context.get_property(payload, "bodyBase64")?;
        let bytes = buffer::encode(&context.as_string(raw)?, "base64")?;
        let body = buffer::from_bytes(context, &bytes)?;
        let body = if event == input::START {
            context.undefined()
        } else {
            body
        };
        context.set_property(request, "body", body)?;
        let encoding = context.undefined();
        context.set_private(request, TEXT_ENCODING, encoding)?;
        let started = context.bool(false)?;
        context.set_private(request, DATA_STARTED, started)?;
        let setter = context.function(REQUEST_SET_ENCODING)?;
        context.set_property(request, "setEncoding", setter)?;
        listeners::install(
            context,
            request,
            &["data", "readable", "end", "aborted", "close", "error"],
        )?;

        let response = context.module_object(RESPONSE)?;
        listeners::install(context, response, &["drain", "close", "error"])?;
        let status = context.number(200.0)?;
        context.set_property(response, "statusCode", status)?;
        let set_header = context.function(RESPONSE_SET_HEADER)?;
        let write_head = context.function(RESPONSE_WRITE_HEAD)?;
        let end = context.function(RESPONSE_END)?;
        context.set_property(response, "setHeader", set_header)?;
        context.set_property(response, "writeHead", write_head)?;
        context.set_property(response, "end", end)?;
        for (name, key) in [
            ("flushHeaders", RESPONSE_FLUSH_HEADERS),
            ("write", RESPONSE_WRITE),
        ] {
            let function = context.function(key)?;
            context.set_property(response, name, function)?;
        }
        let no = context.bool(false)?;
        context.set_property(response, "headersSent", no)?;
        let buffered = context.string(HttpResponseLifecycle::Buffered.as_private())?;
        context.set_private(response, LIFECYCLE, buffered)?;
        let empty_headers = context.string("[]")?;
        let uncommitted = context.undefined();
        context.set_private(response, COMMITTED_STATUS, uncommitted)?;
        let empty_body = context.string("")?;
        context.set_private(response, HEADERS_JSON, empty_headers)?;
        context.set_private(response, BODY, empty_body)?;
        let no_body_bytes = context.undefined();
        context.set_private(response, BODY_BYTES, no_body_bytes)?;
        context.set_private(response, REQUEST_HANDLE, request)?;
        let connection_id = context.get_property(payload, "connectionId")?;
        let request_id = context.get_property(payload, "requestId")?;
        context.set_private(response, CONNECTION_ID, connection_id)?;
        context.set_private(response, REQUEST_ID, request_id)?;
        let sequence = context.number(1.0)?;
        context.set_private(response, SEQUENCE, sequence)?;
        let close_delivered = context.bool(false)?;
        context.set_private(response, CLOSE_DELIVERED, close_delivered)?;

        input::install(context, request, response, event == input::START)?;
        let streamed = context.bool(event == input::START)?;
        context.set_property(request, "streamedInput", streamed)?;
        let emitted = input::server_emit(context, target, request, response)?;
        if !matches!(emitted, ModuleCallResult::Return(_)) {
            return Ok(emitted);
        }
        if event == input::START {
            return Ok(ModuleCallResult::Return(response));
        }
        let enc = context.get_private(request, TEXT_ENCODING)?;
        let encoding = if context.value_kind(enc)? == jjs_module_api::ModuleValueKind::Undefined {
            None
        } else {
            Some(context.as_string(enc)?)
        };
        let started = context.bool(true)?;
        context.set_private(request, DATA_STARTED, started)?;
        let mut pending = Vec::new();
        // Buffered input is sliced deterministically; transport streaming belongs to iteration 5.
        let count = bytes.len().div_ceil(16384);
        for (i, chunk) in bytes.chunks(16384).enumerate() {
            let value = if let Some(enc) = &encoding {
                let text = if enc == "utf8" {
                    buffer::decode_utf8(&mut pending, chunk, i + 1 == count)?
                } else {
                    buffer::decode(chunk, enc)?
                };
                if text.is_empty() {
                    continue;
                }
                context.string(&text)?
            } else {
                buffer::from_bytes(context, chunk)?
            };
            let emitted = listeners::emit(context, request, "data", &[value])?;
            if !matches!(emitted, ModuleCallResult::Return(_)) {
                return Ok(emitted);
            }
        }
        let emitted = listeners::emit(context, request, "end", &[])?;
        if !matches!(emitted, ModuleCallResult::Return(_)) {
            return Ok(emitted);
        }

        Ok(ModuleCallResult::Return(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_transitions_and_action_order_are_explicit() {
        let mut stream = HttpResponseStreamV2::new("connection-1", "request-1");
        stream.set_status(201).unwrap();
        stream
            .set_header("Content-Type", "text/event-stream")
            .unwrap();
        stream.flush_headers().unwrap();
        stream.write("data: one\n\n").unwrap();
        stream.write("data: two\n\n").unwrap();
        stream.end(None).unwrap();

        assert_eq!(stream.lifecycle(), HttpResponseLifecycle::Ended);
        assert!(matches!(
            stream.actions()[0],
            HttpStreamActionV2::Start { sequence: 1, .. }
        ));
        assert!(matches!(
            stream.actions()[1],
            HttpStreamActionV2::Write { sequence: 2, .. }
        ));
        assert!(matches!(
            stream.actions()[2],
            HttpStreamActionV2::Write { sequence: 3, .. }
        ));
        assert!(matches!(
            stream.actions()[3],
            HttpStreamActionV2::End { sequence: 4, .. }
        ));
    }

    #[test]
    fn lifecycle_rejects_every_required_invalid_transition() {
        let mut stream = HttpResponseStreamV2::new("connection-1", "request-1");
        assert_eq!(
            stream.deliver_event(HttpStreamEventV2::Drain {
                version: HTTP_STREAM_CONTRACT_VERSION,
                connection_id: "connection-1".into(),
                request_id: "request-1".into(),
                sequence: 1,
            }),
            Err(HttpStreamContractError::DrainBeforeStreaming)
        );
        stream.flush_headers().unwrap();
        assert_eq!(
            stream.flush_headers(),
            Err(HttpStreamContractError::RepeatedStart)
        );
        assert_eq!(
            stream.set_header("late", "value"),
            Err(HttpStreamContractError::HeadersCommitted)
        );
        assert_eq!(
            stream.deliver_event(HttpStreamEventV2::Drain {
                version: HTTP_STREAM_CONTRACT_VERSION,
                connection_id: "wrong".into(),
                request_id: "request-1".into(),
                sequence: 2,
            }),
            Err(HttpStreamContractError::UnknownConnection)
        );
        assert_eq!(
            stream.deliver_event(HttpStreamEventV2::Drain {
                version: HTTP_STREAM_CONTRACT_VERSION,
                connection_id: "connection-1".into(),
                request_id: "wrong".into(),
                sequence: 2,
            }),
            Err(HttpStreamContractError::UnknownRequest)
        );
        assert_eq!(
            stream.deliver_event(HttpStreamEventV2::Drain {
                version: HTTP_STREAM_CONTRACT_VERSION,
                connection_id: "connection-1".into(),
                request_id: "request-1".into(),
                sequence: 9,
            }),
            Err(HttpStreamContractError::OutOfOrder {
                expected: 2,
                actual: 9
            })
        );
        stream.end(None).unwrap();
        assert_eq!(
            stream.write("late"),
            Err(HttpStreamContractError::StreamClosed)
        );
        assert_eq!(stream.end(None), Err(HttpStreamContractError::StreamClosed));
    }

    #[test]
    fn close_is_ordered_and_terminal() {
        let mut stream = HttpResponseStreamV2::new("connection-1", "request-1");
        stream.flush_headers().unwrap();
        stream
            .deliver_event(HttpStreamEventV2::Close {
                version: HTTP_STREAM_CONTRACT_VERSION,
                connection_id: "connection-1".into(),
                request_id: "request-1".into(),
                reason: "client_disconnect".into(),
                sequence: 2,
            })
            .unwrap();
        assert_eq!(stream.lifecycle(), HttpResponseLifecycle::Closed);
        assert_eq!(
            stream.write("late"),
            Err(HttpStreamContractError::StreamClosed)
        );
    }

    #[test]
    fn buffered_end_emits_no_stream_actions() {
        let mut stream = HttpResponseStreamV2::new("connection-1", "request-1");
        stream.end(Some("ordinary response".into())).unwrap();
        assert_eq!(stream.lifecycle(), HttpResponseLifecycle::Ended);
        assert!(stream.actions().is_empty());
    }
}
