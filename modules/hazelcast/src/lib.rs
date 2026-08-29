//! Rust-native Hazelcast-shaped module implemented only through `jjs-module-api`.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, MODULE_API_VERSION,
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError, ModuleFunctionKey,
    ModuleIdentity, ModuleManifest, ModuleObjectKind, NativeModule, ValueHandle,
};

const NEW_CLIENT: ModuleFunctionKey = ModuleFunctionKey(1);
const GET_MAP: ModuleFunctionKey = ModuleFunctionKey(2);
const GET_ATOMIC_LONG: ModuleFunctionKey = ModuleFunctionKey(3);
const GET_RINGBUFFER: ModuleFunctionKey = ModuleFunctionKey(4);
const GET_UNIQUE_THRESHOLD: ModuleFunctionKey = ModuleFunctionKey(5);
const GET_ORDERED_QUEUE: ModuleFunctionKey = ModuleFunctionKey(6);
const CLIENT_ATOMIC_BATCH: ModuleFunctionKey = ModuleFunctionKey(7);
const MAP_GET: ModuleFunctionKey = ModuleFunctionKey(10);
const MAP_PUT: ModuleFunctionKey = ModuleFunctionKey(11);
const MAP_SET: ModuleFunctionKey = ModuleFunctionKey(12);
const MAP_REMOVE: ModuleFunctionKey = ModuleFunctionKey(13);
const MAP_DELETE: ModuleFunctionKey = ModuleFunctionKey(14);
const MAP_CONTAINS_KEY: ModuleFunctionKey = ModuleFunctionKey(15);
const MAP_PUT_IF_ABSENT: ModuleFunctionKey = ModuleFunctionKey(16);
const MAP_REPLACE: ModuleFunctionKey = ModuleFunctionKey(17);
const MAP_GET_ALL: ModuleFunctionKey = ModuleFunctionKey(18);
const MAP_PUT_ALL: ModuleFunctionKey = ModuleFunctionKey(19);
const MAP_SIZE: ModuleFunctionKey = ModuleFunctionKey(20);
const MAP_CLEAR: ModuleFunctionKey = ModuleFunctionKey(21);
const MAP_SUBSCRIBE: ModuleFunctionKey = ModuleFunctionKey(22);
const MAP_ENTRY_SET: ModuleFunctionKey = ModuleFunctionKey(23);
const MAP_PUT_VERSIONED: ModuleFunctionKey = ModuleFunctionKey(24);
const MAP_REPLACE_VERSIONED: ModuleFunctionKey = ModuleFunctionKey(25);
const MAP_UPDATE_VERSIONED: ModuleFunctionKey = ModuleFunctionKey(26);
const ATOMIC_GET: ModuleFunctionKey = ModuleFunctionKey(30);
const ATOMIC_SET: ModuleFunctionKey = ModuleFunctionKey(31);
const ATOMIC_INCREMENT_AND_GET: ModuleFunctionKey = ModuleFunctionKey(32);
const ATOMIC_GET_AND_INCREMENT: ModuleFunctionKey = ModuleFunctionKey(33);
const ATOMIC_ADD_AND_GET: ModuleFunctionKey = ModuleFunctionKey(34);
const ATOMIC_COMPARE_AND_SET: ModuleFunctionKey = ModuleFunctionKey(35);
const SUBSCRIPTION_READ: ModuleFunctionKey = ModuleFunctionKey(40);
const SUBSCRIPTION_CLOSE: ModuleFunctionKey = ModuleFunctionKey(41);
const SUBSCRIPTION_READ_SSE: ModuleFunctionKey = ModuleFunctionKey(42);
const SUBSCRIPTION_KEEPALIVE: ModuleFunctionKey = ModuleFunctionKey(43);
const RINGBUFFER_ADD: ModuleFunctionKey = ModuleFunctionKey(50);
const RINGBUFFER_READ_AFTER: ModuleFunctionKey = ModuleFunctionKey(51);
const UNIQUE_THRESHOLD_GET_STATE: ModuleFunctionKey = ModuleFunctionKey(52);
const UNIQUE_THRESHOLD_CONTRIBUTE: ModuleFunctionKey = ModuleFunctionKey(53);
const ORDERED_QUEUE_GET_STATE: ModuleFunctionKey = ModuleFunctionKey(54);
const ORDERED_QUEUE_JOIN: ModuleFunctionKey = ModuleFunctionKey(55);
const ORDERED_QUEUE_LEAVE: ModuleFunctionKey = ModuleFunctionKey(56);
const ORDERED_QUEUE_RENEW: ModuleFunctionKey = ModuleFunctionKey(57);
const ORDERED_QUEUE_DISCONNECT: ModuleFunctionKey = ModuleFunctionKey(58);
const PIPE_MAP: ModuleFunctionKey = ModuleFunctionKey(60);
const PIPE_READABLE: ModuleFunctionKey = ModuleFunctionKey(61);
const PIPE_DRAIN: ModuleFunctionKey = ModuleFunctionKey(62);
const PIPE_HEARTBEAT: ModuleFunctionKey = ModuleFunctionKey(63);
const PIPE_CLOSE: ModuleFunctionKey = ModuleFunctionKey(64);
const PIPE_STATE: ModuleFunctionKey = ModuleFunctionKey(65);
const BROWSER_SOURCE: ModuleFunctionKey = ModuleFunctionKey(66);

const CLIENT: ModuleObjectKind = ModuleObjectKind(1);
const MAP: ModuleObjectKind = ModuleObjectKind(2);
const ATOMIC_LONG: ModuleObjectKind = ModuleObjectKind(3);
const SUBSCRIPTION: ModuleObjectKind = ModuleObjectKind(4);
const RINGBUFFER: ModuleObjectKind = ModuleObjectKind(5);
const PIPE: ModuleObjectKind = ModuleObjectKind(6);
const UNIQUE_THRESHOLD: ModuleObjectKind = ModuleObjectKind(7);
const ORDERED_QUEUE: ModuleObjectKind = ModuleObjectKind(8);
const RESOURCE_NAME: u32 = 1;
const READABLE_CALLBACK: u32 = 2;
const MAX_PENDING_BYTES: u32 = 3;
const PIPE_RESPONSE: u32 = 10;
const PIPE_SUBSCRIPTION: u32 = 11;
const PIPE_MAX_EVENTS: u32 = 12;
const PIPE_MAX_OUTPUT_BYTES: u32 = 13;
const PIPE_ACTIVE: u32 = 14;
const PIPE_PENDING: u32 = 15;
const PIPE_CLOSED: u32 = 16;
const PIPE_TIMER: u32 = 17;
const PIPE_HANDLE: u32 = 18;
const PIPE_ON_CLOSE: u32 = 19;
const PIPE_CLOSE_PAYLOAD: u32 = 20;
const PIPE_MIN_VERSION: u32 = 21;
const PIPE_STATE_MODE: u32 = 22;
const PIPE_CLIENT: u32 = 23;

const CLIENT_OPEN: &str = "jjs:state/client/open";
const MAP_OPEN: &str = "jjs:state/map/open";
const ATOMIC_OPEN: &str = "jjs:state/atomic/open";
const SUBSCRIPTION_OPEN: &str = "jjs:state/map/subscription/open";
const SUBSCRIPTION_READ_HOST: &str = "jjs:state/map/subscription/read";
const SUBSCRIPTION_CLOSE_HOST: &str = "jjs:state/map/subscription/close";
const RINGBUFFER_OPEN: &str = "jjs:state/ringbuffer/open";
const UNIQUE_THRESHOLD_OPEN: &str = "jjs:state/unique-threshold/open";
const UNIQUE_THRESHOLD_GET: &str = "jjs:state/unique-threshold/get";
const UNIQUE_THRESHOLD_CONTRIBUTE_HOST: &str = "jjs:state/unique-threshold/contribute";
const ORDERED_QUEUE_OPEN: &str = "jjs:state/ordered-queue/open";
const ORDERED_QUEUE_GET: &str = "jjs:state/ordered-queue/get";
const ORDERED_QUEUE_JOIN_HOST: &str = "jjs:state/ordered-queue/join";
const ORDERED_QUEUE_LEAVE_HOST: &str = "jjs:state/ordered-queue/leave";
const ORDERED_QUEUE_RENEW_HOST: &str = "jjs:state/ordered-queue/renew";
const ORDERED_QUEUE_DISCONNECT_HOST: &str = "jjs:state/ordered-queue/disconnect";
const ATOMIC_BATCH_HOST: &str = "jjs:state/atomicBatch";

const RETURN_SAVED: u32 = 1;
const RETURN_JSON: u32 = 2;
const RETURN_VALUE: u32 = 3;
const RETURN_UNDEFINED: u32 = 4;
const RETURN_SSE: u32 = 5;
const PIPE_READ_COMPLETE: u32 = 21;
const PIPE_CLOSED_COMPLETE: u32 = 22;
const PIPE_STATE_OPEN_COMPLETE: u32 = 23;

const SSE_EVENT_OVERHEAD_BYTES: f64 = 64.0;
const SSE_RESET_MAX_BYTES: f64 = 128.0;

pub struct HazelcastModule {
    manifest: ModuleManifest,
}

impl Default for HazelcastModule {
    fn default() -> Self {
        let capabilities = capability_ids()
            .into_iter()
            .map(|id| HostCapabilityDescriptor {
                id: id.into(),
                contract_version: 1,
                completion: if matches!(id, SUBSCRIPTION_OPEN | SUBSCRIPTION_CLOSE_HOST) {
                    CompletionMode::Sync
                } else {
                    CompletionMode::Yield
                },
                schema: "jjs.state.v1".into(),
            })
            .collect();
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.hazelcast-client".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-hazelcast-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 3,
                imports: vec!["hazelcast-client".into(), "jjs-sse".into()],
                capabilities,
                dependencies: vec![],
                function_keys: vec![
                    NEW_CLIENT.0,
                    GET_MAP.0,
                    GET_ATOMIC_LONG.0,
                    GET_RINGBUFFER.0,
                    GET_UNIQUE_THRESHOLD.0,
                    GET_ORDERED_QUEUE.0,
                    CLIENT_ATOMIC_BATCH.0,
                    MAP_GET.0,
                    MAP_PUT.0,
                    MAP_SET.0,
                    MAP_REMOVE.0,
                    MAP_DELETE.0,
                    MAP_CONTAINS_KEY.0,
                    MAP_PUT_IF_ABSENT.0,
                    MAP_REPLACE.0,
                    MAP_GET_ALL.0,
                    MAP_PUT_ALL.0,
                    MAP_SIZE.0,
                    MAP_CLEAR.0,
                    MAP_SUBSCRIBE.0,
                    MAP_ENTRY_SET.0,
                    MAP_PUT_VERSIONED.0,
                    MAP_REPLACE_VERSIONED.0,
                    MAP_UPDATE_VERSIONED.0,
                    ATOMIC_GET.0,
                    ATOMIC_SET.0,
                    ATOMIC_INCREMENT_AND_GET.0,
                    ATOMIC_GET_AND_INCREMENT.0,
                    ATOMIC_ADD_AND_GET.0,
                    ATOMIC_COMPARE_AND_SET.0,
                    SUBSCRIPTION_READ.0,
                    SUBSCRIPTION_CLOSE.0,
                    SUBSCRIPTION_READ_SSE.0,
                    SUBSCRIPTION_KEEPALIVE.0,
                    RINGBUFFER_ADD.0,
                    RINGBUFFER_READ_AFTER.0,
                    UNIQUE_THRESHOLD_GET_STATE.0,
                    UNIQUE_THRESHOLD_CONTRIBUTE.0,
                    ORDERED_QUEUE_GET_STATE.0,
                    ORDERED_QUEUE_JOIN.0,
                    ORDERED_QUEUE_LEAVE.0,
                    ORDERED_QUEUE_RENEW.0,
                    ORDERED_QUEUE_DISCONNECT.0,
                    PIPE_MAP.0,
                    PIPE_READABLE.0,
                    PIPE_DRAIN.0,
                    PIPE_HEARTBEAT.0,
                    PIPE_CLOSE.0,
                    PIPE_STATE.0,
                    BROWSER_SOURCE.0,
                ],
                object_kind_keys: vec![
                    CLIENT.0,
                    MAP.0,
                    ATOMIC_LONG.0,
                    SUBSCRIPTION.0,
                    RINGBUFFER.0,
                    PIPE.0,
                    UNIQUE_THRESHOLD.0,
                    ORDERED_QUEUE.0,
                ],
                deterministic_resources: vec![],
            },
        }
    }
}

pub fn capability_ids() -> Vec<&'static str> {
    vec![
        CLIENT_OPEN,
        MAP_OPEN,
        ATOMIC_OPEN,
        RINGBUFFER_OPEN,
        UNIQUE_THRESHOLD_OPEN,
        UNIQUE_THRESHOLD_GET,
        UNIQUE_THRESHOLD_CONTRIBUTE_HOST,
        ORDERED_QUEUE_OPEN,
        ORDERED_QUEUE_GET,
        ORDERED_QUEUE_JOIN_HOST,
        ORDERED_QUEUE_LEAVE_HOST,
        ORDERED_QUEUE_RENEW_HOST,
        ORDERED_QUEUE_DISCONNECT_HOST,
        ATOMIC_BATCH_HOST,
        "jjs:state/map/get",
        "jjs:state/map/put",
        "jjs:state/map/set",
        "jjs:state/map/remove",
        "jjs:state/map/delete",
        "jjs:state/map/containsKey",
        "jjs:state/map/putIfAbsent",
        "jjs:state/map/replace",
        "jjs:state/map/getAll",
        "jjs:state/map/putAll",
        "jjs:state/map/size",
        "jjs:state/map/clear",
        "jjs:state/map/entrySet",
        "jjs:state/map/putVersioned",
        "jjs:state/map/replaceVersioned",
        "jjs:state/map/updateVersioned",
        SUBSCRIPTION_OPEN,
        SUBSCRIPTION_READ_HOST,
        SUBSCRIPTION_CLOSE_HOST,
        "jjs:state/atomic/get",
        "jjs:state/atomic/set",
        "jjs:state/atomic/incrementAndGet",
        "jjs:state/atomic/getAndIncrement",
        "jjs:state/atomic/addAndGet",
        "jjs:state/atomic/compareAndSet",
        "jjs:state/ringbuffer/add",
        "jjs:state/ringbuffer/readAfter",
    ]
}

fn thrown(name: &str, message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: name.into(),
        message: message.into(),
    }
}

fn attach(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<(), ModuleError> {
    let function = context.function(key)?;
    context.set_property(object, name, function)
}

fn client(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(CLIENT)?;
    attach(context, value, "getMap", GET_MAP)?;
    attach(context, value, "getAtomicLong", GET_ATOMIC_LONG)?;
    attach(context, value, "getRingbuffer", GET_RINGBUFFER)?;
    attach(context, value, "getUniqueThreshold", GET_UNIQUE_THRESHOLD)?;
    attach(context, value, "getOrderedQueue", GET_ORDERED_QUEUE)?;
    attach(context, value, "atomicBatch", CLIENT_ATOMIC_BATCH)?;
    Ok(value)
}

fn named_proxy(
    context: &mut dyn ModuleContext,
    kind: ModuleObjectKind,
    name: &str,
) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(kind)?;
    let name = context.string(name)?;
    context.set_private(value, RESOURCE_NAME, name)?;
    Ok(value)
}

fn map(context: &mut dyn ModuleContext, name: &str) -> Result<ValueHandle, ModuleError> {
    let value = named_proxy(context, MAP, name)?;
    for (name, key) in [
        ("get", MAP_GET),
        ("put", MAP_PUT),
        ("set", MAP_SET),
        ("remove", MAP_REMOVE),
        ("delete", MAP_DELETE),
        ("containsKey", MAP_CONTAINS_KEY),
        ("putIfAbsent", MAP_PUT_IF_ABSENT),
        ("replace", MAP_REPLACE),
        ("getAll", MAP_GET_ALL),
        ("putAll", MAP_PUT_ALL),
        ("size", MAP_SIZE),
        ("clear", MAP_CLEAR),
        ("subscribe", MAP_SUBSCRIBE),
        ("entrySet", MAP_ENTRY_SET),
        ("putVersioned", MAP_PUT_VERSIONED),
        ("replaceVersioned", MAP_REPLACE_VERSIONED),
        ("updateVersioned", MAP_UPDATE_VERSIONED),
    ] {
        attach(context, value, name, key)?;
    }
    Ok(value)
}

fn positive_safe_integer(
    context: &dyn ModuleContext,
    value: ValueHandle,
    name: &str,
) -> Result<f64, ModuleCallResult> {
    let number = context.as_number(value).map_err(|_| {
        thrown(
            "TypeError",
            format!("Hazelcast subscription {name} must be a positive safe integer"),
        )
    })?;
    if !number.is_finite()
        || number.fract() != 0.0
        || !(1.0..=9_007_199_254_740_991.0).contains(&number)
    {
        return Err(thrown(
            "TypeError",
            format!("Hazelcast subscription {name} must be a positive safe integer"),
        ));
    }
    Ok(number)
}

fn map_limit(
    context: &dyn ModuleContext,
    value: ValueHandle,
    name: &str,
) -> Result<ValueHandle, ModuleCallResult> {
    let number = context.as_number(value).map_err(|_| {
        thrown(
            "TypeError",
            format!("Hazelcast map {name} must be a positive safe integer"),
        )
    })?;
    if !number.is_finite()
        || number.fract() != 0.0
        || !(1.0..=9_007_199_254_740_991.0).contains(&number)
    {
        return Err(thrown(
            "TypeError",
            format!("Hazelcast map {name} must be a positive safe integer"),
        ));
    }
    Ok(value)
}

fn subscription(
    context: &mut dyn ModuleContext,
    callback: ValueHandle,
    max_pending_bytes: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(SUBSCRIPTION)?;
    attach(context, value, "read", SUBSCRIPTION_READ)?;
    attach(context, value, "readSse", SUBSCRIPTION_READ_SSE)?;
    attach(context, value, "keepalive", SUBSCRIPTION_KEEPALIVE)?;
    attach(context, value, "close", SUBSCRIPTION_CLOSE)?;
    context.set_private(value, READABLE_CALLBACK, callback)?;
    context.set_private(value, MAX_PENDING_BYTES, max_pending_bytes)?;
    Ok(value)
}

fn pipe_bool(
    context: &mut dyn ModuleContext,
    pipe: ValueHandle,
    field: u32,
) -> Result<bool, ModuleError> {
    let value = context.get_private(pipe, field)?;
    context.as_bool(value)
}

fn set_pipe_bool(
    context: &mut dyn ModuleContext,
    pipe: ValueHandle,
    field: u32,
    value: bool,
) -> Result<(), ModuleError> {
    let value = context.bool(value)?;
    context.set_private(pipe, field, value)
}

fn pipe_callback(
    context: &mut dyn ModuleContext,
    key: ModuleFunctionKey,
    pipe: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let callback = context.function(key)?;
    context.set_private(callback, PIPE_HANDLE, pipe)?;
    Ok(callback)
}

fn call_method(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    args: &[ValueHandle],
) -> Result<ValueHandle, ModuleError> {
    let method = context.get_property(object, name)?;
    if !context.is_callable(method) {
        return Err(ModuleError::ContractViolation(format!(
            "jjs-sse requires callable {name}"
        )));
    }
    context.call(method, object, args)
}

fn start_pipe_read(
    context: &mut dyn ModuleContext,
    pipe: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    if pipe_bool(context, pipe, PIPE_CLOSED)? {
        return Ok(ModuleCallResult::Return(context.undefined()));
    }
    set_pipe_bool(context, pipe, PIPE_ACTIVE, true)?;
    set_pipe_bool(context, pipe, PIPE_PENDING, false)?;
    let subscription = context.get_private(pipe, PIPE_SUBSCRIPTION)?;
    let max_events = context.get_private(pipe, PIPE_MAX_EVENTS)?;
    request(
        context,
        SUBSCRIPTION_READ_HOST,
        vec![subscription, max_events],
        PIPE_READ_COMPLETE,
        vec![pipe],
    )
}

fn close_pipe(
    context: &mut dyn ModuleContext,
    pipe: ValueHandle,
    reason: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    if pipe_bool(context, pipe, PIPE_CLOSED)? {
        return Ok(ModuleCallResult::Return(context.undefined()));
    }
    set_pipe_bool(context, pipe, PIPE_CLOSED, true)?;
    let lifecycle = context.object()?;
    context.set_property(lifecycle, "reason", reason)?;
    let client = context.get_private(pipe, PIPE_CLIENT)?;
    context.set_property(lifecycle, "client", client)?;
    context.set_private(pipe, PIPE_CLOSE_PAYLOAD, lifecycle)?;
    let timer = context.get_private(pipe, PIPE_TIMER)?;
    if context.value_kind(timer)? != jjs_module_api::ModuleValueKind::Undefined {
        let clear = context.global("clearInterval")?;
        let undefined = context.undefined();
        context.call(clear, undefined, &[timer])?;
    }
    let subscription = context.get_private(pipe, PIPE_SUBSCRIPTION)?;
    if context.value_kind(subscription)? == jjs_module_api::ModuleValueKind::Undefined {
        let callback = context.get_private(pipe, PIPE_ON_CLOSE)?;
        if context.value_kind(callback)? == jjs_module_api::ModuleValueKind::Undefined {
            return Ok(ModuleCallResult::Return(context.undefined()));
        }
        let payload = context.get_private(pipe, PIPE_CLOSE_PAYLOAD)?;
        let undefined = context.undefined();
        let result = context.call(callback, undefined, &[payload])?;
        return Ok(ModuleCallResult::Return(result));
    }
    let closed = request(
        context,
        SUBSCRIPTION_CLOSE_HOST,
        vec![subscription],
        PIPE_CLOSED_COMPLETE,
        vec![pipe],
    )?;
    match closed {
        ModuleCallResult::Return(_) => {
            let callback = context.get_private(pipe, PIPE_ON_CLOSE)?;
            if context.value_kind(callback)? == jjs_module_api::ModuleValueKind::Undefined {
                return Ok(ModuleCallResult::Return(context.undefined()));
            }
            let payload = context.get_private(pipe, PIPE_CLOSE_PAYLOAD)?;
            let undefined = context.undefined();
            let result = context.call(callback, undefined, &[payload])?;
            Ok(ModuleCallResult::Return(result))
        }
        other => Ok(other),
    }
}

fn encode_sse_batch(
    encoded: &str,
    max_output_bytes: f64,
    after_version: Option<u64>,
) -> Result<String, String> {
    let batch: serde_json::Value = serde_json::from_str(encoded)
        .map_err(|error| format!("Hazelcast SSE batch is invalid: {error}"))?;
    let overflowed = batch
        .get("overflowed")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| "Hazelcast SSE batch overflow flag is missing".to_owned())?;
    let remaining_count = batch
        .get("remainingCount")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "Hazelcast SSE batch remaining count is missing".to_owned())?;
    let remaining_bytes = batch
        .get("remainingBytes")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "Hazelcast SSE batch remaining bytes are missing".to_owned())?;

    let chunk = if overflowed {
        let reset = serde_json::json!({
            "reason": "overflow",
            "firstRejectedSequence": batch.get("firstRejectedSequence").cloned().unwrap_or(serde_json::Value::Null),
        });
        format!("event: reset\ndata: {reset}\n\n")
    } else {
        let events = batch
            .get("events")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "Hazelcast SSE batch events are missing".to_owned())?;
        let events = events
            .iter()
            .filter(|event| {
                after_version.is_none_or(|version| {
                    event.get("sequence").and_then(serde_json::Value::as_u64) > Some(version)
                })
            })
            .collect::<Vec<_>>();
        if events.is_empty() {
            ": keepalive\n\n".to_owned()
        } else {
            let mut chunk = String::new();
            for event in events {
                let sequence = event
                    .get("sequence")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|sequence| *sequence <= 9_007_199_254_740_991)
                    .ok_or_else(|| "Hazelcast SSE event sequence is invalid".to_owned())?;
                chunk.push_str(&format!("id: {sequence}\nevent: state\ndata: {event}\n\n"));
            }
            chunk
        }
    };
    if chunk.len() as f64 > max_output_bytes {
        return Err("Hazelcast SSE output byte limit exceeded".to_owned());
    }
    serde_json::to_string(&serde_json::json!({
        "chunk": chunk,
        "reset": overflowed,
        "hasMore": remaining_count > 0,
        "remainingCount": remaining_count,
        "remainingBytes": remaining_bytes,
        "firstRejectedSequence": batch.get("firstRejectedSequence").cloned().unwrap_or(serde_json::Value::Null),
    }))
    .map_err(|error| format!("Hazelcast SSE result encoding failed: {error}"))
}

fn atomic_long(context: &mut dyn ModuleContext, name: &str) -> Result<ValueHandle, ModuleError> {
    let value = named_proxy(context, ATOMIC_LONG, name)?;
    for (name, key) in [
        ("get", ATOMIC_GET),
        ("set", ATOMIC_SET),
        ("incrementAndGet", ATOMIC_INCREMENT_AND_GET),
        ("getAndIncrement", ATOMIC_GET_AND_INCREMENT),
        ("addAndGet", ATOMIC_ADD_AND_GET),
        ("compareAndSet", ATOMIC_COMPARE_AND_SET),
    ] {
        attach(context, value, name, key)?;
    }
    Ok(value)
}

fn ringbuffer(context: &mut dyn ModuleContext, name: &str) -> Result<ValueHandle, ModuleError> {
    let value = named_proxy(context, RINGBUFFER, name)?;
    attach(context, value, "add", RINGBUFFER_ADD)?;
    attach(context, value, "readAfter", RINGBUFFER_READ_AFTER)?;
    Ok(value)
}

fn unique_threshold(
    context: &mut dyn ModuleContext,
    name: &str,
) -> Result<ValueHandle, ModuleError> {
    let value = named_proxy(context, UNIQUE_THRESHOLD, name)?;
    attach(context, value, "getState", UNIQUE_THRESHOLD_GET_STATE)?;
    attach(context, value, "contribute", UNIQUE_THRESHOLD_CONTRIBUTE)?;
    attach(context, value, "subscribe", MAP_SUBSCRIBE)?;
    Ok(value)
}

fn ordered_queue(context: &mut dyn ModuleContext, name: &str) -> Result<ValueHandle, ModuleError> {
    let value = named_proxy(context, ORDERED_QUEUE, name)?;
    for (name, key) in [
        ("getState", ORDERED_QUEUE_GET_STATE),
        ("join", ORDERED_QUEUE_JOIN),
        ("leave", ORDERED_QUEUE_LEAVE),
        ("renew", ORDERED_QUEUE_RENEW),
        ("disconnect", ORDERED_QUEUE_DISCONNECT),
        ("subscribe", MAP_SUBSCRIBE),
    ] {
        attach(context, value, name, key)?;
    }
    Ok(value)
}

fn request(
    context: &mut dyn ModuleContext,
    capability: &str,
    arguments: Vec<ValueHandle>,
    continuation: u32,
    state: Vec<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    context.request_host(
        HostRequestSpec {
            capability: capability.into(),
            operation: capability.into(),
            arguments,
        },
        ModuleContinuation(continuation),
        state,
        true,
    )
}

fn resource_name(
    context: &mut dyn ModuleContext,
    receiver: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    context.get_private(receiver, RESOURCE_NAME)
}

fn json_argument(
    context: &mut dyn ModuleContext,
    value: ValueHandle,
) -> Result<ValueHandle, ModuleCallResult> {
    context.json_stringify(value).map_err(|_| {
        thrown(
            "TypeError",
            "Hazelcast M1 value must be JSON-compatible and acyclic",
        )
    })
}

fn map_request(
    context: &mut dyn ModuleContext,
    receiver: ValueHandle,
    capability: &str,
    args: &[ValueHandle],
    arity: std::ops::RangeInclusive<usize>,
    json_args: bool,
    continuation: u32,
) -> Result<ModuleCallResult, ModuleError> {
    if !arity.contains(&args.len()) {
        return Ok(thrown(
            "TypeError",
            "Hazelcast M1 invalid map operation arguments",
        ));
    }
    let mut arguments = vec![resource_name(context, receiver)?];
    for value in args {
        if json_args {
            let value = match json_argument(context, *value) {
                Ok(value) => value,
                Err(error) => return Ok(error),
            };
            arguments.push(value);
        } else {
            arguments.push(*value);
        }
    }
    request(context, capability, arguments, continuation, vec![])
}

impl NativeModule for HazelcastModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let client = context.object()?;
        attach(context, client, "newHazelcastClient", NEW_CLIENT)?;
        context.set_property(exports, "Client", client)?;
        attach(context, exports, "pipeMap", PIPE_MAP)?;
        attach(context, exports, "pipeState", PIPE_STATE)?;
        attach(context, exports, "browserSource", BROWSER_SOURCE)?;
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
            BROWSER_SOURCE => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "TypeError",
                        "jjs-sse browserSource accepts no arguments",
                    ));
                }
                Ok(ModuleCallResult::Return(context.string(
                    r#"(function(g){'use strict';function connect(o){if(!o||typeof o.url!=='string'||typeof o.onBatch!=='function')throw new TypeError('JjsSse.connect requires url and onBatch');var delay=Number(o.batchMs);if(!Number.isFinite(delay)||delay<0)throw new TypeError('JjsSse.connect batchMs must be non-negative');var source=new EventSource(o.url);var queue=[];var timer=null;function flush(){timer=null;if(queue.length===0)return;var batch=queue;queue=[];o.onBatch(batch);}function enqueue(event){var value=JSON.parse(event.data);if(typeof o.project==='function')value=o.project(value);queue.push(value);if(timer===null)timer=setTimeout(flush,delay);}source.addEventListener('state',enqueue);source.addEventListener('reset',function(event){queue=[];if(timer!==null){clearTimeout(timer);timer=null;}if(typeof o.onReset==='function')o.onReset(JSON.parse(event.data));});return Object.freeze({close:function(){if(timer!==null)clearTimeout(timer);source.close();},source:source});}g.JjsSse=Object.freeze({connect:connect});})(globalThis);"#,
                )?))
            }
            NEW_CLIENT => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "Error",
                        "Hazelcast M1 unsupported: external cluster configuration",
                    ));
                }
                let client = client(context)?;
                request(context, CLIENT_OPEN, vec![], RETURN_SAVED, vec![client])
            }
            PIPE_MAP | PIPE_STATE => {
                let state_mode = key == PIPE_STATE;
                if args.len() != 1
                    || context.value_kind(args[0])? != jjs_module_api::ModuleValueKind::Object
                {
                    return Ok(thrown(
                        "TypeError",
                        "jjs-sse pipeMap requires one options object",
                    ));
                }
                let mut names = context.own_property_names(args[0])?;
                names.sort();
                let valid_names = if state_mode {
                    names
                        == [
                            "drain",
                            "heartbeatMs",
                            "identity",
                            "onClose",
                            "reconnect",
                            "req",
                            "res",
                            "state",
                            "subscription",
                        ]
                } else {
                    names
                        == [
                            "drain",
                            "heartbeatMs",
                            "map",
                            "reconnect",
                            "req",
                            "res",
                            "subscription",
                        ]
                        || names
                            == [
                                "drain",
                                "heartbeatMs",
                                "map",
                                "onClose",
                                "reconnect",
                                "req",
                                "res",
                                "subscription",
                            ]
                };
                if !valid_names {
                    return Ok(thrown(
                        "TypeError",
                        "jjs-sse pipeMap options must match the exact contract",
                    ));
                }
                let source_name = if state_mode { "state" } else { "map" };
                let source = context.get_property(args[0], source_name)?;
                let resource = match resource_name(context, source) {
                    Ok(value) => value,
                    Err(_) => {
                        return Ok(thrown("TypeError", "jjs-sse map must be a Hazelcast map"));
                    }
                };
                let request_value = context.get_property(args[0], "req")?;
                let response = context.get_property(args[0], "res")?;
                let client = context.get_property(request_value, "client")?;
                let identity = if state_mode {
                    let value = context.get_property(args[0], "identity")?;
                    match json_argument(context, value) {
                        Ok(value) => value,
                        Err(error) => return Ok(error),
                    }
                } else {
                    context.undefined()
                };
                let on_close = if names.iter().any(|name| name == "onClose") {
                    let callback = context.get_property(args[0], "onClose")?;
                    if !context.is_callable(callback) {
                        return Ok(thrown("TypeError", "jjs-sse onClose must be callable"));
                    }
                    callback
                } else {
                    context.undefined()
                };
                let subscription_options = context.get_property(args[0], "subscription")?;
                let drain = context.get_property(args[0], "drain")?;
                let mut subscription_names = context.own_property_names(subscription_options)?;
                subscription_names.sort();
                let mut drain_names = context.own_property_names(drain)?;
                drain_names.sort();
                if subscription_names != ["maxPendingBytes", "maxPendingEvents"]
                    || drain_names != ["maxEvents", "maxOutputBytes"]
                {
                    return Ok(thrown(
                        "TypeError",
                        "jjs-sse limits must match the exact contract",
                    ));
                }
                let max_pending_events =
                    context.get_property(subscription_options, "maxPendingEvents")?;
                let max_pending_bytes =
                    context.get_property(subscription_options, "maxPendingBytes")?;
                let max_events = context.get_property(drain, "maxEvents")?;
                let max_output_bytes = context.get_property(drain, "maxOutputBytes")?;
                for (value, label) in [
                    (max_pending_events, "maxPendingEvents"),
                    (max_pending_bytes, "maxPendingBytes"),
                    (max_events, "maxEvents"),
                    (max_output_bytes, "maxOutputBytes"),
                ] {
                    if let Err(error) = positive_safe_integer(context, value, label) {
                        return Ok(error);
                    }
                }
                let required_capacity = (context.as_number(max_pending_bytes)?
                    + context.as_number(max_events)? * SSE_EVENT_OVERHEAD_BYTES)
                    .max(SSE_RESET_MAX_BYTES);
                if context.as_number(max_output_bytes)? < required_capacity {
                    return Ok(thrown(
                        "RangeError",
                        format!(
                            "jjs-sse output limit must reserve at least {required_capacity} bytes"
                        ),
                    ));
                }
                let heartbeat = context.get_property(args[0], "heartbeatMs")?;
                if let Err(error) = positive_safe_integer(context, heartbeat, "heartbeatMs") {
                    return Ok(error);
                }
                let reconnect = context.get_property(args[0], "reconnect")?;
                if context.as_string(reconnect).ok().as_deref() != Some("reset") {
                    return Ok(thrown("TypeError", "jjs-sse reconnect must be reset"));
                }

                let pipe = context.module_object(PIPE)?;
                let undefined = context.undefined();
                context.set_private(pipe, PIPE_SUBSCRIPTION, undefined)?;
                context.set_private(pipe, PIPE_RESPONSE, response)?;
                context.set_private(pipe, PIPE_MAX_EVENTS, max_events)?;
                context.set_private(pipe, PIPE_MAX_OUTPUT_BYTES, max_output_bytes)?;
                context.set_private(pipe, PIPE_TIMER, undefined)?;
                context.set_private(pipe, PIPE_ON_CLOSE, on_close)?;
                context.set_private(pipe, PIPE_CLOSE_PAYLOAD, undefined)?;
                context.set_private(pipe, PIPE_MIN_VERSION, undefined)?;
                context.set_private(pipe, PIPE_CLIENT, client)?;
                set_pipe_bool(context, pipe, PIPE_STATE_MODE, state_mode)?;
                set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
                set_pipe_bool(context, pipe, PIPE_PENDING, false)?;
                set_pipe_bool(context, pipe, PIPE_CLOSED, false)?;

                let readable = pipe_callback(context, PIPE_READABLE, pipe)?;
                let drain_callback = pipe_callback(context, PIPE_DRAIN, pipe)?;
                let heartbeat_callback = pipe_callback(context, PIPE_HEARTBEAT, pipe)?;
                let close_callback = pipe_callback(context, PIPE_CLOSE, pipe)?;
                context.set_property(pipe, "close", close_callback)?;
                context.set_property(pipe, "pump", readable)?;
                context.set_property(pipe, "heartbeat", heartbeat_callback)?;

                let content_type = context.string("Content-Type")?;
                let content_value = context.string("text/event-stream")?;
                call_method(context, response, "set", &[content_type, content_value])?;
                let cache_name = context.string("Cache-Control")?;
                let cache_value = context.string("no-cache")?;
                call_method(context, response, "set", &[cache_name, cache_value])?;
                call_method(context, response, "flushHeaders", &[])?;
                let connected = context.string(": connected\n\n")?;
                call_method(context, response, "write", &[connected])?;

                let headers = context.get_property(request_value, "headers")?;
                let last_event_id = context.get_property(headers, "last-event-id")?;
                if context.value_kind(last_event_id)? == jjs_module_api::ModuleValueKind::String
                    && !context.as_string(last_event_id)?.is_empty()
                {
                    let reset =
                        context.string("event: reset\ndata: {\"reason\":\"reconnect\"}\n\n")?;
                    call_method(context, response, "write", &[reset])?;
                }

                let drain_name = context.string("drain")?;
                call_method(context, response, "on", &[drain_name, drain_callback])?;
                let close_name = context.string("close")?;
                call_method(context, request_value, "on", &[close_name, close_callback])?;
                let set_interval = context.global("setInterval")?;
                let timer_receiver = context.undefined();
                let timer = context.call(
                    set_interval,
                    timer_receiver,
                    &[heartbeat_callback, heartbeat],
                )?;
                context.set_private(pipe, PIPE_TIMER, timer)?;

                let subscription = subscription(context, readable, max_pending_bytes)?;
                let opened = context.request_host(
                    HostRequestSpec {
                        capability: SUBSCRIPTION_OPEN.into(),
                        operation: SUBSCRIPTION_OPEN.into(),
                        arguments: vec![
                            resource,
                            max_pending_events,
                            max_pending_bytes,
                            subscription,
                            readable,
                        ],
                    },
                    ModuleContinuation(RETURN_VALUE),
                    vec![],
                    false,
                )?;
                match opened {
                    ModuleCallResult::Return(_) => {
                        context.set_private(pipe, PIPE_SUBSCRIPTION, subscription)?;
                        if state_mode {
                            context.request_host(
                                HostRequestSpec {
                                    capability: ORDERED_QUEUE_GET.into(),
                                    operation: ORDERED_QUEUE_GET.into(),
                                    arguments: vec![resource, identity],
                                },
                                ModuleContinuation(PIPE_STATE_OPEN_COMPLETE),
                                vec![pipe],
                                true,
                            )
                        } else {
                            Ok(ModuleCallResult::Return(pipe))
                        }
                    }
                    other => Ok(other),
                }
            }
            GET_MAP => {
                if !(1..=2).contains(&args.len()) {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast getMap requires a name and optional capacity policy",
                    ));
                }
                let name = match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => name,
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast M1 resource name must be a non-empty string",
                        ));
                    }
                };
                let proxy = map(context, &name)?;
                let mut arguments = vec![args[0]];
                if args.len() == 2 {
                    let mut names = match context.own_property_names(args[1]) {
                        Ok(names) => names,
                        Err(_) => {
                            return Ok(thrown(
                                "TypeError",
                                "Hazelcast map capacity policy must be an object",
                            ));
                        }
                    };
                    names.sort();
                    if names != ["maxBytes", "maxEntries"] {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast map capacity policy requires exactly maxEntries and maxBytes",
                        ));
                    }
                    let max_entries = context.get_property(args[1], "maxEntries")?;
                    let max_bytes = context.get_property(args[1], "maxBytes")?;
                    let max_entries = match map_limit(context, max_entries, "maxEntries") {
                        Ok(value) => value,
                        Err(error) => return Ok(error),
                    };
                    let max_bytes = match map_limit(context, max_bytes, "maxBytes") {
                        Ok(value) => value,
                        Err(error) => return Ok(error),
                    };
                    arguments.extend([max_entries, max_bytes]);
                }
                request(context, MAP_OPEN, arguments, RETURN_SAVED, vec![proxy])
            }
            GET_ATOMIC_LONG => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast M1 resource name is required",
                    ));
                }
                let name = match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => name,
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast M1 resource name must be a non-empty string",
                        ));
                    }
                };
                let proxy = atomic_long(context, &name)?;
                request(
                    context,
                    ATOMIC_OPEN,
                    vec![args[0]],
                    RETURN_SAVED,
                    vec![proxy],
                )
            }
            GET_RINGBUFFER => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast getRingbuffer requires a name and retention limits",
                    ));
                }
                let name = match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => name,
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast ringbuffer name must be a non-empty string",
                        ));
                    }
                };
                let max_entries = context.get_property(args[1], "maxEntries")?;
                let max_bytes = context.get_property(args[1], "maxBytes")?;
                for (value, label) in [(max_entries, "maxEntries"), (max_bytes, "maxBytes")] {
                    let valid = context.as_number(value).is_ok_and(|number| {
                        number.is_finite()
                            && number.fract() == 0.0
                            && (1.0..=9_007_199_254_740_991.0).contains(&number)
                    });
                    if !valid {
                        return Ok(thrown(
                            "TypeError",
                            format!("Hazelcast ringbuffer {label} must be a positive safe integer"),
                        ));
                    }
                }
                let proxy = ringbuffer(context, &name)?;
                request(
                    context,
                    RINGBUFFER_OPEN,
                    vec![args[0], max_entries, max_bytes],
                    RETURN_SAVED,
                    vec![proxy],
                )
            }
            GET_UNIQUE_THRESHOLD => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast getUniqueThreshold requires a name and exact policy",
                    ));
                }
                let name = match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => name,
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast unique threshold name must be a non-empty string",
                        ));
                    }
                };
                let mut names = match context.own_property_names(args[1]) {
                    Ok(names) => names,
                    Err(_) => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast unique threshold policy must be an object",
                        ));
                    }
                };
                names.sort();
                if names != ["maxBytes", "maxContributors", "target"] {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast unique threshold policy requires exactly target, maxContributors, and maxBytes",
                    ));
                }
                let target = context.get_property(args[1], "target")?;
                let max_contributors = context.get_property(args[1], "maxContributors")?;
                let max_bytes = context.get_property(args[1], "maxBytes")?;
                for (value, label) in [
                    (target, "target"),
                    (max_contributors, "maxContributors"),
                    (max_bytes, "maxBytes"),
                ] {
                    if let Err(error) = positive_safe_integer(context, value, label) {
                        return Ok(error);
                    }
                }
                if context.as_number(target)? > context.as_number(max_contributors)? {
                    return Ok(thrown(
                        "RangeError",
                        "Hazelcast unique threshold target cannot exceed maxContributors",
                    ));
                }
                let proxy = unique_threshold(context, &name)?;
                request(
                    context,
                    UNIQUE_THRESHOLD_OPEN,
                    vec![args[0], target, max_contributors, max_bytes],
                    RETURN_SAVED,
                    vec![proxy],
                )
            }
            GET_ORDERED_QUEUE => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast getOrderedQueue requires a name and exact policy",
                    ));
                }
                let name = match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => name,
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast ordered queue name must be a non-empty string",
                        ));
                    }
                };
                let mut names = match context.own_property_names(args[1]) {
                    Ok(names) => names,
                    Err(_) => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast ordered queue policy must be an object",
                        ));
                    }
                };
                names.sort();
                if names
                    != [
                        "disconnectPolicy",
                        "identityMode",
                        "leaseMs",
                        "maxBytes",
                        "maxEntries",
                        "maxNameLength",
                    ]
                {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ordered queue policy requires exactly maxEntries, maxBytes, maxNameLength, disconnectPolicy, identityMode, and leaseMs",
                    ));
                }
                let max_entries = context.get_property(args[1], "maxEntries")?;
                let max_bytes = context.get_property(args[1], "maxBytes")?;
                let max_name_length = context.get_property(args[1], "maxNameLength")?;
                let lease_ms = context.get_property(args[1], "leaseMs")?;
                for (value, label) in [
                    (max_entries, "maxEntries"),
                    (max_bytes, "maxBytes"),
                    (max_name_length, "maxNameLength"),
                    (lease_ms, "leaseMs"),
                ] {
                    if let Err(error) = positive_safe_integer(context, value, label) {
                        return Ok(error);
                    }
                }
                let disconnect_policy = context.get_property(args[1], "disconnectPolicy")?;
                if context.as_string(disconnect_policy).ok().as_deref() != Some("lease") {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ordered queue disconnectPolicy must be lease",
                    ));
                }
                let identity_mode = context.get_property(args[1], "identityMode")?;
                if !matches!(
                    context.as_string(identity_mode).ok().as_deref(),
                    Some("network" | "cookie" | "cookie+tab")
                ) {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ordered queue identityMode must be network, cookie, or cookie+tab",
                    ));
                }
                let proxy = ordered_queue(context, &name)?;
                request(
                    context,
                    ORDERED_QUEUE_OPEN,
                    vec![
                        args[0],
                        max_entries,
                        max_bytes,
                        max_name_length,
                        disconnect_policy,
                        lease_ms,
                        identity_mode,
                    ],
                    RETURN_SAVED,
                    vec![proxy],
                )
            }
            CLIENT_ATOMIC_BATCH => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast atomicBatch requires exactly one operations array",
                    ));
                }
                let operations = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                request(
                    context,
                    ATOMIC_BATCH_HOST,
                    vec![operations],
                    RETURN_JSON,
                    vec![],
                )
            }
            MAP_GET => map_request(
                context,
                receiver,
                "jjs:state/map/get",
                args,
                1..=1,
                true,
                RETURN_JSON,
            ),
            MAP_PUT => map_request(
                context,
                receiver,
                "jjs:state/map/put",
                args,
                2..=2,
                true,
                RETURN_JSON,
            ),
            MAP_SET => map_request(
                context,
                receiver,
                "jjs:state/map/set",
                args,
                2..=2,
                true,
                RETURN_UNDEFINED,
            ),
            MAP_REMOVE => map_request(
                context,
                receiver,
                "jjs:state/map/remove",
                args,
                1..=2,
                true,
                if args.len() == 1 {
                    RETURN_JSON
                } else {
                    RETURN_VALUE
                },
            ),
            MAP_DELETE => map_request(
                context,
                receiver,
                "jjs:state/map/delete",
                args,
                1..=1,
                true,
                RETURN_UNDEFINED,
            ),
            MAP_CONTAINS_KEY => map_request(
                context,
                receiver,
                "jjs:state/map/containsKey",
                args,
                1..=1,
                true,
                RETURN_VALUE,
            ),
            MAP_PUT_IF_ABSENT => map_request(
                context,
                receiver,
                "jjs:state/map/putIfAbsent",
                args,
                2..=2,
                true,
                RETURN_JSON,
            ),
            MAP_REPLACE => map_request(
                context,
                receiver,
                "jjs:state/map/replace",
                args,
                3..=3,
                true,
                RETURN_VALUE,
            ),
            MAP_GET_ALL => map_request(
                context,
                receiver,
                "jjs:state/map/getAll",
                args,
                1..=1,
                true,
                RETURN_JSON,
            ),
            MAP_PUT_ALL => map_request(
                context,
                receiver,
                "jjs:state/map/putAll",
                args,
                1..=1,
                true,
                RETURN_UNDEFINED,
            ),
            MAP_SIZE => map_request(
                context,
                receiver,
                "jjs:state/map/size",
                args,
                0..=0,
                false,
                RETURN_VALUE,
            ),
            MAP_CLEAR => map_request(
                context,
                receiver,
                "jjs:state/map/clear",
                args,
                0..=0,
                false,
                RETURN_UNDEFINED,
            ),
            MAP_ENTRY_SET => {
                if args.len() != 2
                    || args.iter().any(|value| {
                        context.as_number(*value).map_or(true, |number| {
                            !number.is_finite()
                                || number.fract() != 0.0
                                || !(1.0..=9_007_199_254_740_991.0).contains(&number)
                        })
                    })
                {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast entrySet limits must be positive safe integers",
                    ));
                }
                map_request(
                    context,
                    receiver,
                    "jjs:state/map/entrySet",
                    args,
                    2..=2,
                    false,
                    RETURN_JSON,
                )
            }
            MAP_PUT_VERSIONED => {
                if args.len() != 3 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast putVersioned requires a key, value, and counter name",
                    ));
                }
                match context.as_string(args[2]) {
                    Ok(name) if !name.is_empty() => {}
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast putVersioned counter name must be a non-empty string",
                        ));
                    }
                }
                let key = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let value = match json_argument(context, args[1]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let name = resource_name(context, receiver)?;
                request(
                    context,
                    "jjs:state/map/putVersioned",
                    vec![name, key, value, args[2]],
                    RETURN_JSON,
                    vec![],
                )
            }
            MAP_REPLACE_VERSIONED => {
                if args.len() != 4 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast replaceVersioned requires a key, expected version, value, and counter name",
                    ));
                }
                if let Err(error) = positive_safe_integer(context, args[1], "expected version") {
                    return Ok(error);
                }
                match context.as_string(args[3]) {
                    Ok(name) if !name.is_empty() => {}
                    _ => {
                        return Ok(thrown(
                            "TypeError",
                            "Hazelcast replaceVersioned counter name must be a non-empty string",
                        ));
                    }
                }
                let key = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let value = match json_argument(context, args[2]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let name = resource_name(context, receiver)?;
                request(
                    context,
                    "jjs:state/map/replaceVersioned",
                    vec![name, key, args[1], value, args[3]],
                    RETURN_JSON,
                    vec![],
                )
            }
            MAP_UPDATE_VERSIONED => {
                if args.len() != 4 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast updateVersioned requires a key, expected version, value, and counter name",
                    ));
                }
                if context.as_number(args[1]).is_err() || context.as_string(args[3]).is_err() {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast updateVersioned expected version and counter name are invalid",
                    ));
                }
                let key = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let value = match json_argument(context, args[2]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let name = resource_name(context, receiver)?;
                request(
                    context,
                    "jjs:state/map/updateVersioned",
                    vec![name, key, args[1], value, args[3]],
                    RETURN_JSON,
                    vec![],
                )
            }
            MAP_SUBSCRIBE => {
                if args.len() != 2 || !context.is_callable(args[1]) {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast map.subscribe requires limits and a readable callback",
                    ));
                }
                let max_events = context.get_property(args[0], "maxPendingEvents")?;
                let max_bytes = context.get_property(args[0], "maxPendingBytes")?;
                if let Err(error) = positive_safe_integer(context, max_events, "maxPendingEvents") {
                    return Ok(error);
                }
                if let Err(error) = positive_safe_integer(context, max_bytes, "maxPendingBytes") {
                    return Ok(error);
                }
                let value = subscription(context, args[1], max_bytes)?;
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    SUBSCRIPTION_OPEN,
                    vec![resource, max_events, max_bytes, value, args[1]],
                    RETURN_VALUE,
                    vec![],
                )
            }
            SUBSCRIPTION_READ => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast subscription.read requires one maximum event count",
                    ));
                }
                if let Err(error) = positive_safe_integer(context, args[0], "read count") {
                    return Ok(error);
                }
                request(
                    context,
                    SUBSCRIPTION_READ_HOST,
                    vec![receiver, args[0]],
                    RETURN_JSON,
                    vec![],
                )
            }
            SUBSCRIPTION_READ_SSE => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast subscription.readSse requires event and output byte limits",
                    ));
                }
                let max_events = match positive_safe_integer(context, args[0], "SSE read count") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let max_output_bytes =
                    match positive_safe_integer(context, args[1], "SSE output byte limit") {
                        Ok(value) => value,
                        Err(error) => return Ok(error),
                    };
                let max_pending_bytes = context.get_private(receiver, MAX_PENDING_BYTES)?;
                let max_pending_bytes = context.as_number(max_pending_bytes)?;
                let required_capacity = (max_pending_bytes + max_events * SSE_EVENT_OVERHEAD_BYTES)
                    .max(SSE_RESET_MAX_BYTES);
                if required_capacity > max_output_bytes {
                    return Ok(thrown(
                        "RangeError",
                        format!(
                            "Hazelcast subscription.readSse output limit must reserve at least {required_capacity} bytes"
                        ),
                    ));
                }
                request(
                    context,
                    SUBSCRIPTION_READ_HOST,
                    vec![receiver, args[0]],
                    RETURN_SSE,
                    vec![args[1]],
                )
            }
            SUBSCRIPTION_KEEPALIVE => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast subscription.keepalive accepts no arguments",
                    ));
                }
                Ok(ModuleCallResult::Return(context.string(": keepalive\n\n")?))
            }
            SUBSCRIPTION_CLOSE => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast subscription.close accepts no arguments",
                    ));
                }
                request(
                    context,
                    SUBSCRIPTION_CLOSE_HOST,
                    vec![receiver],
                    RETURN_UNDEFINED,
                    vec![],
                )
            }
            PIPE_READABLE => {
                let pipe = context.get_private(_callee, PIPE_HANDLE)?;
                if pipe_bool(context, pipe, PIPE_CLOSED)? {
                    return Ok(ModuleCallResult::Return(context.undefined()));
                }
                if pipe_bool(context, pipe, PIPE_ACTIVE)? {
                    set_pipe_bool(context, pipe, PIPE_PENDING, true)?;
                    return Ok(ModuleCallResult::Return(context.undefined()));
                }
                start_pipe_read(context, pipe)
            }
            PIPE_DRAIN => {
                let pipe = context.get_private(_callee, PIPE_HANDLE)?;
                if pipe_bool(context, pipe, PIPE_PENDING)? {
                    start_pipe_read(context, pipe)
                } else {
                    Ok(ModuleCallResult::Return(context.undefined()))
                }
            }
            PIPE_HEARTBEAT => {
                let pipe = context.get_private(_callee, PIPE_HANDLE)?;
                if !pipe_bool(context, pipe, PIPE_CLOSED)?
                    && !pipe_bool(context, pipe, PIPE_ACTIVE)?
                {
                    let response = context.get_private(pipe, PIPE_RESPONSE)?;
                    let chunk = context.string(": keepalive\n\n")?;
                    call_method(context, response, "write", &[chunk])?;
                }
                Ok(ModuleCallResult::Return(context.undefined()))
            }
            PIPE_CLOSE => {
                let pipe = context.get_private(_callee, PIPE_HANDLE)?;
                let reason = args.first().copied().unwrap_or_else(|| context.undefined());
                close_pipe(context, pipe, reason)
            }
            RINGBUFFER_ADD => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ringbuffer.add requires one JSON value",
                    ));
                }
                let value = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let name = resource_name(context, receiver)?;
                request(
                    context,
                    "jjs:state/ringbuffer/add",
                    vec![name, value],
                    RETURN_VALUE,
                    vec![],
                )
            }
            RINGBUFFER_READ_AFTER => {
                if args.len() != 3 {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ringbuffer.readAfter requires a sequence and read limits",
                    ));
                }
                let sequence_valid = context.as_number(args[0]).is_ok_and(|number| {
                    number.is_finite()
                        && number.fract() == 0.0
                        && (-1.0..=9_007_199_254_740_991.0).contains(&number)
                });
                let limits_valid = args[1..].iter().all(|value| {
                    context.as_number(*value).is_ok_and(|number| {
                        number.is_finite()
                            && number.fract() == 0.0
                            && (1.0..=9_007_199_254_740_991.0).contains(&number)
                    })
                });
                if !sequence_valid || !limits_valid {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast ringbuffer.readAfter arguments must be safe integer bounds",
                    ));
                }
                map_request(
                    context,
                    receiver,
                    "jjs:state/ringbuffer/readAfter",
                    args,
                    3..=3,
                    false,
                    RETURN_JSON,
                )
            }
            UNIQUE_THRESHOLD_GET_STATE => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast unique threshold getState accepts no arguments",
                    ));
                }
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    UNIQUE_THRESHOLD_GET,
                    vec![resource],
                    RETURN_JSON,
                    vec![],
                )
            }
            UNIQUE_THRESHOLD_CONTRIBUTE => {
                if !args.is_empty() {
                    return Ok(thrown(
                        "TypeError",
                        "Hazelcast unique threshold contribute accepts no arguments",
                    ));
                }
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    UNIQUE_THRESHOLD_CONTRIBUTE_HOST,
                    vec![resource],
                    RETURN_JSON,
                    vec![],
                )
            }
            ORDERED_QUEUE_GET_STATE => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "OrderedQueue.getState requires one identity object or null",
                    ));
                }
                let identity = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    ORDERED_QUEUE_GET,
                    vec![resource, identity],
                    RETURN_JSON,
                    vec![],
                )
            }
            ORDERED_QUEUE_JOIN => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "OrderedQueue.join requires one entry object and one identity object",
                    ));
                }
                let entry = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let identity = match json_argument(context, args[1]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    ORDERED_QUEUE_JOIN_HOST,
                    vec![resource, entry, identity],
                    RETURN_JSON,
                    vec![],
                )
            }
            ORDERED_QUEUE_LEAVE | ORDERED_QUEUE_RENEW | ORDERED_QUEUE_DISCONNECT => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "OrderedQueue lifecycle methods require one identity object",
                    ));
                }
                let identity = match json_argument(context, args[0]) {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let operation = match key {
                    ORDERED_QUEUE_LEAVE => ORDERED_QUEUE_LEAVE_HOST,
                    ORDERED_QUEUE_RENEW => ORDERED_QUEUE_RENEW_HOST,
                    _ => ORDERED_QUEUE_DISCONNECT_HOST,
                };
                let resource = resource_name(context, receiver)?;
                request(
                    context,
                    operation,
                    vec![resource, identity],
                    RETURN_JSON,
                    vec![],
                )
            }
            ATOMIC_GET => map_request(
                context,
                receiver,
                "jjs:state/atomic/get",
                args,
                0..=0,
                false,
                RETURN_VALUE,
            ),
            ATOMIC_SET => map_request(
                context,
                receiver,
                "jjs:state/atomic/set",
                args,
                1..=1,
                false,
                RETURN_UNDEFINED,
            ),
            ATOMIC_INCREMENT_AND_GET => map_request(
                context,
                receiver,
                "jjs:state/atomic/incrementAndGet",
                args,
                0..=0,
                false,
                RETURN_VALUE,
            ),
            ATOMIC_GET_AND_INCREMENT => map_request(
                context,
                receiver,
                "jjs:state/atomic/getAndIncrement",
                args,
                0..=0,
                false,
                RETURN_VALUE,
            ),
            ATOMIC_ADD_AND_GET => map_request(
                context,
                receiver,
                "jjs:state/atomic/addAndGet",
                args,
                1..=1,
                false,
                RETURN_VALUE,
            ),
            ATOMIC_COMPARE_AND_SET => map_request(
                context,
                receiver,
                "jjs:state/atomic/compareAndSet",
                args,
                2..=2,
                false,
                RETURN_VALUE,
            ),
            _ => Err(ModuleError::ContractViolation(
                "unknown Hazelcast function key".into(),
            )),
        }
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if continuation.0 == PIPE_READ_COMPLETE {
            let pipe = state.first().copied().ok_or_else(|| {
                ModuleError::ContractViolation("jjs-sse read pipe state is missing".into())
            })?;
            let value = match completion {
                Ok(value) => value,
                Err(message) => {
                    set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
                    return Ok(thrown("SsePipeReadError", message));
                }
            };
            if pipe_bool(context, pipe, PIPE_CLOSED)? {
                set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
                return Ok(ModuleCallResult::Return(context.undefined()));
            }
            let max_output = context.get_private(pipe, PIPE_MAX_OUTPUT_BYTES)?;
            let encoded = context.as_string(value)?;
            let min_version = context.get_private(pipe, PIPE_MIN_VERSION)?;
            let after_version =
                if context.value_kind(min_version)? == jjs_module_api::ModuleValueKind::Undefined {
                    None
                } else {
                    Some(context.as_number(min_version)? as u64)
                };
            let result =
                match encode_sse_batch(&encoded, context.as_number(max_output)?, after_version) {
                    Ok(result) => result,
                    Err(message) => {
                        set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
                        return Ok(thrown("SsePipeOutputError", message));
                    }
                };
            let encoded_result = context.string(&result)?;
            let result = context.json_parse(encoded_result)?;
            let chunk = context.get_property(result, "chunk")?;
            let has_more = context.get_property(result, "hasMore")?;
            let reset = context.get_property(result, "reset")?;
            let response = context.get_private(pipe, PIPE_RESPONSE)?;
            let writable = call_method(context, response, "write", &[chunk])?;
            if context.as_bool(reset)? {
                set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
                set_pipe_bool(context, pipe, PIPE_PENDING, false)?;
                return Ok(ModuleCallResult::Return(context.undefined()));
            }
            if context.as_bool(has_more)? {
                if context.is_truthy(writable)? {
                    return start_pipe_read(context, pipe);
                }
                set_pipe_bool(context, pipe, PIPE_PENDING, true)?;
                return Ok(ModuleCallResult::Return(context.undefined()));
            }
            set_pipe_bool(context, pipe, PIPE_ACTIVE, false)?;
            if pipe_bool(context, pipe, PIPE_PENDING)? {
                return start_pipe_read(context, pipe);
            }
            return Ok(ModuleCallResult::Return(context.undefined()));
        }
        if continuation.0 == PIPE_CLOSED_COMPLETE {
            let pipe = state.first().copied().ok_or_else(|| {
                ModuleError::ContractViolation("jjs-sse close pipe state is missing".into())
            })?;
            let callback = context.get_private(pipe, PIPE_ON_CLOSE)?;
            let callback_result =
                if context.value_kind(callback)? == jjs_module_api::ModuleValueKind::Undefined {
                    context.undefined()
                } else {
                    let payload = context.get_private(pipe, PIPE_CLOSE_PAYLOAD)?;
                    let undefined = context.undefined();
                    context.call(callback, undefined, &[payload])?
                };
            return match completion {
                Ok(_) => Ok(ModuleCallResult::Return(callback_result)),
                Err(message) => Ok(thrown("SsePipeCloseError", message)),
            };
        }
        if continuation.0 == PIPE_STATE_OPEN_COMPLETE {
            let pipe = state.first().copied().ok_or_else(|| {
                ModuleError::ContractViolation("jjs-sse state pipe is missing".into())
            })?;
            let value = match completion {
                Ok(value) => value,
                Err(message) => return Ok(thrown("SsePipeSnapshotError", message)),
            };
            let encoded = context.as_string(value)?;
            let encoded_value = context.string(&encoded)?;
            let snapshot = context.json_parse(encoded_value)?;
            let version = context.get_property(snapshot, "version")?;
            let version_number = context.as_number(version)?;
            if !version_number.is_finite()
                || version_number.fract() != 0.0
                || !(0.0..=9_007_199_254_740_991.0).contains(&version_number)
            {
                return Ok(thrown("SsePipeSnapshotError", "state version is invalid"));
            }
            context.set_private(pipe, PIPE_MIN_VERSION, version)?;
            let response = context.get_private(pipe, PIPE_RESPONSE)?;
            let chunk = context.string(&format!(
                "id: {}\nevent: snapshot\ndata: {}\n\n",
                version_number as u64, encoded
            ))?;
            call_method(context, response, "write", &[chunk])?;
            return Ok(ModuleCallResult::Return(pipe));
        }
        let value = match completion {
            Ok(value) => value,
            Err(message) => return Ok(thrown("Error", message)),
        };
        Ok(match continuation.0 {
            RETURN_SAVED => ModuleCallResult::Return(state.first().copied().ok_or_else(|| {
                ModuleError::ContractViolation("Hazelcast continuation state is missing".into())
            })?),
            RETURN_JSON => ModuleCallResult::Return(context.json_parse(value)?),
            RETURN_VALUE => ModuleCallResult::Return(value),
            RETURN_UNDEFINED => ModuleCallResult::Return(context.undefined()),
            RETURN_SSE => {
                let max_output_bytes = state
                    .first()
                    .copied()
                    .ok_or_else(|| {
                        ModuleError::ContractViolation(
                            "Hazelcast SSE continuation byte limit is missing".into(),
                        )
                    })
                    .and_then(|value| context.as_number(value))?;
                let encoded = context.as_string(value)?;
                let result = match encode_sse_batch(&encoded, max_output_bytes, None) {
                    Ok(result) => result,
                    Err(message) => return Ok(thrown("Error", message)),
                };
                let result = context.string(&result)?;
                ModuleCallResult::Return(context.json_parse(result)?)
            }
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown Hazelcast continuation".into(),
                ));
            }
        })
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "Hazelcast has no synchronous guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_contract_is_declared_without_implicit_limits() {
        let module = HazelcastModule::default();
        assert!(module.manifest.function_keys.contains(&MAP_SUBSCRIBE.0));
        assert!(module.manifest.function_keys.contains(&SUBSCRIPTION_READ.0));
        assert!(
            module
                .manifest
                .function_keys
                .contains(&SUBSCRIPTION_CLOSE.0)
        );
        assert!(module.manifest.object_kind_keys.contains(&SUBSCRIPTION.0));
        assert!(capability_ids().contains(&SUBSCRIPTION_OPEN));
        assert!(capability_ids().contains(&SUBSCRIPTION_READ_HOST));
        assert!(capability_ids().contains(&SUBSCRIPTION_CLOSE_HOST));
        assert!(module.manifest.function_keys.contains(&MAP_ENTRY_SET.0));
        assert!(capability_ids().contains(&"jjs:state/map/entrySet"));
        assert!(module.manifest.function_keys.contains(&MAP_PUT_VERSIONED.0));
        assert!(capability_ids().contains(&"jjs:state/map/putVersioned"));
        assert!(module.manifest.function_keys.contains(&GET_RINGBUFFER.0));
        assert!(module.manifest.function_keys.contains(&RINGBUFFER_ADD.0));
        assert!(
            module
                .manifest
                .function_keys
                .contains(&SUBSCRIPTION_READ_SSE.0)
        );
        assert!(
            module
                .manifest
                .function_keys
                .contains(&SUBSCRIPTION_KEEPALIVE.0)
        );
        assert!(
            module
                .manifest
                .function_keys
                .contains(&RINGBUFFER_READ_AFTER.0)
        );
        assert!(capability_ids().contains(&RINGBUFFER_OPEN));
        assert!(capability_ids().contains(&"jjs:state/ringbuffer/add"));
        assert!(capability_ids().contains(&"jjs:state/ringbuffer/readAfter"));
        assert!(
            module
                .manifest
                .function_keys
                .contains(&GET_UNIQUE_THRESHOLD.0)
        );
        assert!(
            module
                .manifest
                .object_kind_keys
                .contains(&UNIQUE_THRESHOLD.0)
        );
        assert!(capability_ids().contains(&UNIQUE_THRESHOLD_OPEN));
        assert!(capability_ids().contains(&UNIQUE_THRESHOLD_GET));
        assert!(capability_ids().contains(&UNIQUE_THRESHOLD_CONTRIBUTE_HOST));
        assert!(
            module
                .manifest
                .function_keys
                .contains(&CLIENT_ATOMIC_BATCH.0)
        );
        assert!(capability_ids().contains(&ATOMIC_BATCH_HOST));
    }
}
