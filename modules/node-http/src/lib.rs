//! Rust-native `node:http` module implemented only through `jjs-module-api`.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleObjectKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HTTP_REQUEST_EVENT: u32 = 1;
pub const HTTP_RESPONSE_EVENT: u32 = 2;
pub const HTTP_DRAIN_EVENT: u32 = 3;
pub const HTTP_CLOSE_EVENT: u32 = 4;
pub const HTTP_STREAM_CONTRACT_VERSION: u16 = 1;
const CREATE_SERVER: ModuleFunctionKey = ModuleFunctionKey(1);
const SERVER_LISTEN: ModuleFunctionKey = ModuleFunctionKey(2);
const REQUEST_ON: ModuleFunctionKey = ModuleFunctionKey(3);
const RESPONSE_SET_HEADER: ModuleFunctionKey = ModuleFunctionKey(4);
const RESPONSE_END: ModuleFunctionKey = ModuleFunctionKey(5);
const RESPONSE_FLUSH_HEADERS: ModuleFunctionKey = ModuleFunctionKey(6);
const RESPONSE_WRITE: ModuleFunctionKey = ModuleFunctionKey(7);
const RESPONSE_ON: ModuleFunctionKey = ModuleFunctionKey(8);
const SERVER: ModuleObjectKind = ModuleObjectKind(1);
const REQUEST: ModuleObjectKind = ModuleObjectKind(2);
const RESPONSE: ModuleObjectKind = ModuleObjectKind(3);
const HANDLER: u32 = 1;
const LISTENING: u32 = 2;
const DATA_HANDLER: u32 = 3;
const END_HANDLER: u32 = 4;
const LIFECYCLE: u32 = 5;
const HEADERS_JSON: u32 = 6;
const BODY: u32 = 7;
const CLOSE_HANDLER: u32 = 8;
const DRAIN_HANDLER: u32 = 9;
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
pub enum HttpStreamEventV1 {
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
pub enum HttpStreamActionV1 {
    Start {
        version: u16,
        connection_id: String,
        request_id: String,
        status: u16,
        headers: BTreeMap<String, String>,
        sequence: u64,
    },
    Write {
        version: u16,
        connection_id: String,
        request_id: String,
        utf8_chunk: String,
        sequence: u64,
    },
    End {
        version: u16,
        connection_id: String,
        request_id: String,
        optional_utf8_chunk: Option<String>,
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
pub struct HttpResponseStreamV1 {
    connection_id: String,
    request_id: String,
    status: u16,
    headers: BTreeMap<String, String>,
    lifecycle: HttpResponseLifecycle,
    next_sequence: u64,
    actions: Vec<HttpStreamActionV1>,
}

impl HttpResponseStreamV1 {
    pub fn new(connection_id: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            connection_id: connection_id.into(),
            request_id: request_id.into(),
            status: 200,
            headers: BTreeMap::new(),
            lifecycle: HttpResponseLifecycle::Buffered,
            next_sequence: 1,
            actions: Vec::new(),
        }
    }

    pub fn lifecycle(&self) -> HttpResponseLifecycle {
        self.lifecycle
    }

    pub fn actions(&self) -> &[HttpStreamActionV1] {
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
        self.headers
            .insert(name.into().to_ascii_lowercase(), value.into());
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
        self.actions.push(HttpStreamActionV1::Write {
            version: HTTP_STREAM_CONTRACT_VERSION,
            connection_id: self.connection_id.clone(),
            request_id: self.request_id.clone(),
            utf8_chunk: chunk.into(),
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
                self.actions.push(HttpStreamActionV1::End {
                    version: HTTP_STREAM_CONTRACT_VERSION,
                    connection_id: self.connection_id.clone(),
                    request_id: self.request_id.clone(),
                    optional_utf8_chunk: optional_chunk,
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
        event: HttpStreamEventV1,
    ) -> Result<(), HttpStreamContractError> {
        let (version, connection_id, request_id, sequence, close) = match event {
            HttpStreamEventV1::Drain {
                version,
                connection_id,
                request_id,
                sequence,
            } => (version, connection_id, request_id, sequence, false),
            HttpStreamEventV1::Close {
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
        self.actions.push(HttpStreamActionV1::Start {
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
    pub headers: BTreeMap<String, String>,
    pub body: String,
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
    pub headers: BTreeMap<String, String>,
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
                    implementation: "jjs-module-node-http-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 2,
                imports: vec!["node:http".into()],
                capabilities: vec![
                    HostCapabilityDescriptor {
                        id: "jjs:net/listen".into(),
                        contract_version: 1,
                        completion: CompletionMode::Sync,
                        schema: "jjs.net.listen.v1".into(),
                    },
                    HostCapabilityDescriptor {
                        id: "jjs:http/stream".into(),
                        contract_version: 1,
                        completion: CompletionMode::Sync,
                        schema: "jjs.http.stream.v1".into(),
                    },
                ],
                dependencies: vec![],
                function_keys: vec![
                    CREATE_SERVER.0,
                    SERVER_LISTEN.0,
                    REQUEST_ON.0,
                    RESPONSE_SET_HEADER.0,
                    RESPONSE_END.0,
                    RESPONSE_FLUSH_HEADERS.0,
                    RESPONSE_WRITE.0,
                    RESPONSE_ON.0,
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

fn start_stream(
    context: &mut dyn ModuleContext,
    response: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    match response_lifecycle(context, response)?.as_str() {
        "buffered" => {}
        "streaming" => return Ok(thrown("node_http_response_already_streaming")),
        _ => return Ok(thrown("node_http_response_stream_closed")),
    }
    let status = context.get_property(response, "statusCode")?;
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
    let status = context.get_property(response, "statusCode")?;
    let status = context.as_number(status)? as u16;
    let headers = context.get_private(response, HEADERS_JSON)?;
    let headers = context.as_string(headers)?;
    let headers: BTreeMap<String, String> = serde_json::from_str(&headers)
        .map_err(|_| ModuleError::ContractViolation("node_http_response_headers_invalid".into()))?;
    let body = context.get_private(response, BODY)?;
    let body = context.as_string(body)?;
    let body_bytes = context.get_private(response, BODY_BYTES)?;
    let body_bytes = match context.value_kind(body_bytes)? {
        jjs_module_api::ModuleValueKind::Undefined => None,
        jjs_module_api::ModuleValueKind::Array => {
            let length = context.array_len(body_bytes)?;
            let mut bytes = Vec::with_capacity(length);
            for index in 0..length {
                let value = context.array_get(body_bytes, index)?;
                let value = context.as_number(value)?;
                if !value.is_finite() || value.fract() != 0.0 || !(0.0..=255.0).contains(&value) {
                    return Err(ModuleError::ContractViolation(
                        "node_http_response_byte_invalid".into(),
                    ));
                }
                bytes.push(value as u8);
            }
            Some(bytes)
        }
        _ => {
            return Err(ModuleError::ContractViolation(
                "node_http_response_bytes_invalid".into(),
            ));
        }
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
        _callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            CREATE_SERVER => {
                let Some(handler) = args.first().copied() else {
                    return Ok(thrown("node_http_create_server_handler_not_callable"));
                };
                if !context.is_callable(handler) {
                    return Ok(thrown("node_http_create_server_handler_not_callable"));
                }
                let server = context.module_object(SERVER)?;
                context.set_private(server, HANDLER, handler)?;
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
                    Ok(value) if value >= 1.0 && value <= 65535.0 && value.fract() == 0.0 => value,
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
                    ModuleCallResult::Return(_) => {
                        let listening = context.bool(true)?;
                        context.set_private(receiver, LISTENING, listening)?;
                        context.set_property(receiver, "listening", listening)?;
                        if args.len() == 2 {
                            context.call(args[1], receiver, &[])?;
                        }
                        Ok(ModuleCallResult::Return(receiver))
                    }
                    other => Ok(other),
                }
            }
            REQUEST_ON => {
                if args.len() != 2 || !context.is_callable(args[1]) {
                    return Ok(thrown("node_http_request_on_invalid"));
                }
                match context.as_string(args[0])?.as_str() {
                    "data" => context.set_private(receiver, DATA_HANDLER, args[1])?,
                    "end" => context.set_private(receiver, END_HANDLER, args[1])?,
                    "close" => context.set_private(receiver, CLOSE_HANDLER, args[1])?,
                    _ => return Ok(thrown("node_http_request_event_unsupported")),
                }
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_SET_HEADER => {
                if args.len() != 2 {
                    return Ok(thrown("node_http_set_header_arity_invalid"));
                }
                let lifecycle = context.get_private(receiver, LIFECYCLE)?;
                if context.as_string(lifecycle)? != HttpResponseLifecycle::Buffered.as_private() {
                    return Ok(thrown("node_http_response_headers_committed"));
                }
                let name = context.as_string(args[0])?.to_ascii_lowercase();
                let value = context.as_string(args[1])?;
                let raw = context.get_private(receiver, HEADERS_JSON)?;
                let raw = context.as_string(raw)?;
                let mut headers: BTreeMap<String, String> =
                    serde_json::from_str(&raw).map_err(|_| {
                        ModuleError::ContractViolation("node_http_response_headers_invalid".into())
                    })?;
                headers.insert(name, value);
                let raw = serde_json::to_string(&headers).map_err(|_| {
                    ModuleError::ContractViolation("node_http_response_headers_invalid".into())
                })?;
                let raw = context.string(&raw)?;
                context.set_private(receiver, HEADERS_JSON, raw)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RESPONSE_END => {
                if args.len() > 1 {
                    return Ok(thrown("node_http_response_end_arity_invalid"));
                }
                match response_lifecycle(context, receiver)?.as_str() {
                    "buffered" => {
                        if let Some(value) = args.first() {
                            match context.value_kind(*value)? {
                                jjs_module_api::ModuleValueKind::String => {
                                    let body = context.as_string(*value)?;
                                    let body = context.string(&body)?;
                                    context.set_private(receiver, BODY, body)?;
                                }
                                jjs_module_api::ModuleValueKind::Array => {
                                    context.set_private(receiver, BODY_BYTES, *value)?;
                                }
                                _ => {
                                    return Ok(thrown(
                                        "node_http_response_chunk_not_string_or_bytes",
                                    ));
                                }
                            }
                        }
                        set_response_lifecycle(context, receiver, HttpResponseLifecycle::Ended)?;
                        let undefined = context.undefined();
                        Ok(ModuleCallResult::Return(undefined))
                    }
                    "streaming" => {
                        let chunk = if let Some(value) = args.first() {
                            if context.value_kind(*value)?
                                != jjs_module_api::ModuleValueKind::String
                            {
                                return Ok(thrown("node_http_response_chunk_not_string"));
                            }
                            *value
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
                            let undefined = context.undefined();
                            Ok(ModuleCallResult::Return(undefined))
                        } else {
                            Ok(result)
                        }
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
                if args.len() != 1
                    || context.value_kind(args[0])? != jjs_module_api::ModuleValueKind::String
                {
                    return Ok(thrown("node_http_response_chunk_not_string"));
                }
                if response_lifecycle(context, receiver)? == "buffered" {
                    let started = start_stream(context, receiver)?;
                    if !matches!(started, ModuleCallResult::Return(_)) {
                        return Ok(started);
                    }
                }
                if response_lifecycle(context, receiver)? != "streaming" {
                    return Ok(thrown("node_http_response_stream_closed"));
                }
                stream_request(context, receiver, "write", vec![args[0]])
            }
            RESPONSE_ON => {
                if args.len() != 2 || !context.is_callable(args[1]) {
                    return Ok(thrown("node_http_response_on_invalid"));
                }
                if context.as_string(args[0])? != "drain" {
                    return Ok(thrown("node_http_response_event_unsupported"));
                }
                context.set_private(receiver, DRAIN_HANDLER, args[1])?;
                Ok(ModuleCallResult::Return(receiver))
            }
            _ => Err(ModuleError::ContractViolation(format!(
                "unknown node:http function key {}",
                key.0
            ))),
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
            let handler = context.get_private(target, DRAIN_HANDLER)?;
            if context.is_callable(handler) {
                context.call(handler, target, &[])?;
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
            let handler = context.get_private(request, CLOSE_HANDLER)?;
            if context.is_callable(handler) {
                context.call(handler, request, &[])?;
            }
            let undefined = context.undefined();
            context.set_private(request, CLOSE_HANDLER, undefined)?;
            context.set_private(target, DRAIN_HANDLER, undefined)?;
            return Ok(ModuleCallResult::Return(target));
        }
        if event != HTTP_REQUEST_EVENT {
            return Err(ModuleError::ContractViolation(
                "unsupported node:http event".into(),
            ));
        }
        let handler = context.get_private(target, HANDLER)?;
        let request = context.module_object(REQUEST)?;
        for name in ["method", "url", "headers", "client", "body"] {
            let value = context.get_property(payload, name)?;
            context.set_property(request, name, value)?;
        }
        let on = context.function(REQUEST_ON)?;
        context.set_property(request, "on", on)?;

        let response = context.module_object(RESPONSE)?;
        let status = context.number(200.0)?;
        context.set_property(response, "statusCode", status)?;
        let set_header = context.function(RESPONSE_SET_HEADER)?;
        let end = context.function(RESPONSE_END)?;
        context.set_property(response, "setHeader", set_header)?;
        context.set_property(response, "end", end)?;
        for (name, key) in [
            ("flushHeaders", RESPONSE_FLUSH_HEADERS),
            ("write", RESPONSE_WRITE),
            ("on", RESPONSE_ON),
        ] {
            let function = context.function(key)?;
            context.set_property(response, name, function)?;
        }
        let no = context.bool(false)?;
        context.set_property(response, "headersSent", no)?;
        let buffered = context.string(HttpResponseLifecycle::Buffered.as_private())?;
        context.set_private(response, LIFECYCLE, buffered)?;
        let empty_headers = context.string("{}")?;
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

        context.call(handler, target, &[request, response])?;
        let data_handler = context.get_private(request, DATA_HANDLER)?;
        if context.is_callable(data_handler) {
            let body = context.get_property(payload, "body")?;
            context.call(data_handler, request, &[body])?;
        }
        let end_handler = context.get_private(request, END_HANDLER)?;
        if context.is_callable(end_handler) {
            context.call(end_handler, request, &[])?;
        }

        Ok(ModuleCallResult::Return(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_transitions_and_action_order_are_explicit() {
        let mut stream = HttpResponseStreamV1::new("connection-1", "request-1");
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
            HttpStreamActionV1::Start { sequence: 1, .. }
        ));
        assert!(matches!(
            stream.actions()[1],
            HttpStreamActionV1::Write { sequence: 2, .. }
        ));
        assert!(matches!(
            stream.actions()[2],
            HttpStreamActionV1::Write { sequence: 3, .. }
        ));
        assert!(matches!(
            stream.actions()[3],
            HttpStreamActionV1::End { sequence: 4, .. }
        ));
    }

    #[test]
    fn lifecycle_rejects_every_required_invalid_transition() {
        let mut stream = HttpResponseStreamV1::new("connection-1", "request-1");
        assert_eq!(
            stream.deliver_event(HttpStreamEventV1::Drain {
                version: 1,
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
            stream.deliver_event(HttpStreamEventV1::Drain {
                version: 1,
                connection_id: "wrong".into(),
                request_id: "request-1".into(),
                sequence: 2,
            }),
            Err(HttpStreamContractError::UnknownConnection)
        );
        assert_eq!(
            stream.deliver_event(HttpStreamEventV1::Drain {
                version: 1,
                connection_id: "connection-1".into(),
                request_id: "wrong".into(),
                sequence: 2,
            }),
            Err(HttpStreamContractError::UnknownRequest)
        );
        assert_eq!(
            stream.deliver_event(HttpStreamEventV1::Drain {
                version: 1,
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
        let mut stream = HttpResponseStreamV1::new("connection-1", "request-1");
        stream.flush_headers().unwrap();
        stream
            .deliver_event(HttpStreamEventV1::Close {
                version: 1,
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
        let mut stream = HttpResponseStreamV1::new("connection-1", "request-1");
        stream.end(Some("ordinary response".into())).unwrap();
        assert_eq!(stream.lifecycle(), HttpResponseLifecycle::Ended);
        assert!(stream.actions().is_empty());
    }
}
