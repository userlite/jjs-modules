//! One credited input chunk at a time; all decoder/readable state is in the VM.
use super::*;
use jjs_module_api::ModuleValueKind as Kind;
pub const START: u32 = 5;
pub const DATA: u32 = 6;
pub const END: u32 = 7;
pub const ABORT: u32 = 8;
pub const ERROR: u32 = 9;
pub const STATE: u32 = 10;
pub const MODE: u32 = 20;
const PAUSED: u32 = 30;
const FLOWING: u32 = 31;
const QUEUE: u32 = 32;
const EOF: u32 = 33;
const ENDED: u32 = 34;
const NEXT: u32 = 35;
const PENDING: u32 = 36;
const RESPONSE_HANDLE: u32 = 37;
const FAILED: u32 = 38;
const HOOK: u32 = 39;
const TERMINAL: u32 = 40;
const CONSUMED: u32 = 41;
pub const BODY_STATE: ModuleFunctionKey = ModuleFunctionKey(16);
pub const PAUSE: ModuleFunctionKey = ModuleFunctionKey(11);
pub const RESUME: ModuleFunctionKey = ModuleFunctionKey(12);
pub const READ: ModuleFunctionKey = ModuleFunctionKey(13);
pub const REJECT: ModuleFunctionKey = ModuleFunctionKey(14);
pub const SOCKET: ModuleFunctionKey = ModuleFunctionKey(15);
pub const MAX_CHUNK: usize = 16384;
fn flag(c: &mut dyn ModuleContext, r: ValueHandle, k: u32) -> Result<bool, ModuleError> {
    let v = c.get_private(r, k)?;
    c.as_bool(v)
}
fn set_flag(c: &mut dyn ModuleContext, r: ValueHandle, k: u32, v: bool) -> Result<(), ModuleError> {
    let v = c.bool(v)?;
    c.set_private(r, k, v)
}
pub fn enabled(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<bool, ModuleError> {
    let v = c.get_private(r, MODE)?;
    Ok(c.value_kind(v)? == Kind::Bool && c.as_bool(v)?)
}
pub fn install(
    c: &mut dyn ModuleContext,
    r: ValueHandle,
    response: ValueHandle,
    streaming: bool,
) -> Result<(), ModuleError> {
    set_flag(c, response, MODE, streaming)?;
    set_flag(c, r, MODE, streaming)?;
    for k in [PAUSED, FLOWING, EOF, ENDED, TERMINAL, CONSUMED] {
        set_flag(c, r, k, false)?;
    }
    let empty = c.undefined();
    c.set_private(r, QUEUE, empty)?;
    c.set_private(response, FAILED, empty)?;
    let pending = c.string("")?;
    c.set_private(r, PENDING, pending)?;
    let next = c.number(1.0)?;
    c.set_private(r, NEXT, next)?;
    c.set_private(r, RESPONSE_HANDLE, response)?;
    for (name, key) in [("pause", PAUSE), ("resume", RESUME), ("read", READ)] {
        let f = c.function(key)?;
        c.set_property(r, name, f)?;
    }
    let getter = c.function(BODY_STATE)?;
    let setter = c.function(SOCKET)?;
    c.define_accessor(r, "_bodyInputState", getter, setter)?;
    let connection = c.object()?;
    let destroy = c.function(SOCKET)?;
    c.set_property(connection, "destroy", destroy)?;
    c.set_property(r, "connection", connection)?;
    c.set_property(r, "socket", connection)?;
    let hook = c.function(REJECT)?;
    c.set_private(hook, RESPONSE_HANDLE, response)?;
    c.set_private(r, HOOK, hook)?;
    c.set_private(response, HOOK, hook)?;
    Ok(())
}
pub fn emit(
    c: &mut dyn ModuleContext,
    r: ValueHandle,
    name: &str,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    let hook = c.get_private(r, HOOK)?;
    listeners::emit_capturing(c, r, name, args, Some(hook))
}
pub fn server_emit(
    c: &mut dyn ModuleContext,
    s: ValueHandle,
    r: ValueHandle,
    response: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let hook = c.get_private(r, HOOK)?;
    listeners::emit_capturing(c, s, "request", &[r, response], Some(hook))
}
pub fn close(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<ModuleCallResult, ModuleError> {
    if !enabled(c, r)? {
        return Ok(ModuleCallResult::Return(r));
    }
    let aborted = !flag(c, r, EOF)? && !flag(c, r, TERMINAL)?;
    set_flag(c, r, TERMINAL, true)?;
    let empty = c.undefined();
    c.set_private(r, QUEUE, empty)?;
    if aborted {
        emit(c, r, "aborted", &[])
    } else {
        Ok(ModuleCallResult::Return(r))
    }
}
fn finish(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<ModuleCallResult, ModuleError> {
    let q = c.get_private(r, QUEUE)?;
    if flag(c, r, EOF)? && !flag(c, r, ENDED)? && c.value_kind(q)? == Kind::Undefined {
        set_flag(c, r, ENDED, true)?;
        let pending = c.get_private(r, PENDING)?;
        let mut pending = buffer::encode(&c.as_string(pending)?, "hex")?;
        if !pending.is_empty() {
            let text = buffer::decode_utf8(&mut pending, &[], true)?;
            let v = c.string(&text)?;
            let out = emit(c, r, "data", &[v])?;
            if !matches!(out, ModuleCallResult::Return(_)) {
                return Ok(out);
            }
        }
        emit(c, r, "end", &[])
    } else {
        Ok(ModuleCallResult::Return(r))
    }
}
fn take(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<Option<ValueHandle>, ModuleError> {
    let q = c.get_private(r, QUEUE)?;
    if c.value_kind(q)? == Kind::Undefined {
        return Ok(None);
    }
    let empty = c.undefined();
    c.set_private(r, QUEUE, empty)?;
    set_flag(c, r, CONSUMED, true)?;
    let enc = c.get_private(r, TEXT_ENCODING)?;
    if c.value_kind(enc)? == Kind::Undefined {
        return Ok(Some(q));
    }
    let bytes = c.read_bytes(q)?;
    let enc = c.as_string(enc)?;
    let text = if enc == "utf8" {
        let p = c.get_private(r, PENDING)?;
        let mut p = buffer::encode(&c.as_string(p)?, "hex")?;
        let text = buffer::decode_utf8(&mut p, &bytes, false)?;
        let p = c.string(&buffer::decode(&p, "hex")?)?;
        c.set_private(r, PENDING, p)?;
        text
    } else {
        buffer::decode(&bytes, &enc)?
    };
    Ok(Some(c.string(&text)?))
}
pub fn drain(c: &mut dyn ModuleContext, r: ValueHandle) -> Result<ModuleCallResult, ModuleError> {
    if !flag(c, r, PAUSED)? && flag(c, r, FLOWING)? {
        if let Some(chunk) = take(c, r)? {
            let out = emit(c, r, "data", &[chunk])?;
            if !matches!(out, ModuleCallResult::Return(_)) {
                return Ok(out);
            }
        }
    }
    finish(c, r)
}
pub fn listener_added(
    c: &mut dyn ModuleContext,
    r: ValueHandle,
    key: ModuleFunctionKey,
    args: &[ValueHandle],
) -> Result<(), ModuleError> {
    if matches!(key, listeners::ON | listeners::ONCE)
        && args.len() == 2
        && enabled(c, r)?
        && c.as_string(args[0])? == "data"
    {
        set_flag(c, r, FLOWING, true)?;
    }
    Ok(())
}
pub fn call(
    c: &mut dyn ModuleContext,
    key: ModuleFunctionKey,
    callee: ValueHandle,
    r: ValueHandle,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    if key == BODY_STATE {
        let enc = c.get_private(r, TEXT_ENCODING)?;
        let state = if flag(c, r, TERMINAL)? {
            "terminal"
        } else if flag(c, r, ENDED)? {
            "ended"
        } else if flag(c, r, CONSUMED)? {
            "consumed"
        } else if c.value_kind(enc)? != Kind::Undefined {
            "decoded"
        } else {
            "available"
        };
        return Ok(ModuleCallResult::Return(c.string(state)?));
    }
    if key == SOCKET {
        return Ok(thrown("node_http_socket_destroy_unsupported"));
    }
    if key == REJECT {
        let response = c.get_private(callee, RESPONSE_HANDLE)?;
        let reason = if let Some(v) = args.first() {
            c.to_string(*v)?
        } else {
            "undefined".into()
        };
        let reason = c.string(&reason)?;
        c.set_private(response, FAILED, reason)?;
        return Ok(ModuleCallResult::Return(c.undefined()));
    }
    if !enabled(c, r)? {
        return Ok(thrown("node_http_readable_controls_require_streamed_input"));
    }
    if !args.is_empty() {
        return Ok(thrown("node_http_readable_control_arity_unsupported"));
    }
    if flag(c, r, TERMINAL)? || (flag(c, r, ENDED)? && key != READ) {
        return Ok(thrown("node_http_input_terminal"));
    }
    match key {
        PAUSE => {
            set_flag(c, r, PAUSED, true)?;
            Ok(ModuleCallResult::Return(r))
        }
        RESUME => {
            set_flag(c, r, PAUSED, false)?;
            set_flag(c, r, FLOWING, true)?;
            drain(c, r)
        }
        READ => {
            let v = take(c, r)?;
            let out = finish(c, r)?;
            if !matches!(out, ModuleCallResult::Return(_)) {
                return Ok(out);
            }
            let null = if let Some(v) = v {
                v
            } else {
                {
                    let text = c.string("null")?;
                    c.json_parse(text)?
                }
            };
            Ok(ModuleCallResult::Return(null))
        }
        _ => Err(invalid("node_http_input_function_unknown")),
    }
}
pub fn event(
    c: &mut dyn ModuleContext,
    response: ValueHandle,
    event: u32,
    payload: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let r = c.get_private(response, REQUEST_HANDLE)?;
    if !enabled(c, r)? {
        return Err(invalid("node_http_input_events_require_streamed_input"));
    }
    if event == STATE {
        let failed = c.get_private(response, FAILED)?;
        if c.value_kind(failed)? != Kind::Undefined {
            return Err(invalid(&format!(
                "node_http_async_rejection: {}",
                c.as_string(failed)?
            )));
        }
        let q = c.get_private(r, QUEUE)?;
        let credit = if flag(c, r, PAUSED)?
            || flag(c, r, EOF)?
            || flag(c, r, TERMINAL)?
            || c.value_kind(q)? != Kind::Undefined
        {
            0
        } else {
            MAX_CHUNK
        };
        let next = c.get_private(r, NEXT)?;
        let next = c.as_number(next)? as u64;
        let encoded =
            serde_json::json!({"credit":credit,"inputEnded":flag(c,r,ENDED)?,"next":next});
        return Ok(ModuleCallResult::Return(c.string(&encoded.to_string())?));
    }
    if flag(c, r, EOF)? || flag(c, r, TERMINAL)? {
        return Err(invalid("node_http_input_event_after_terminal"));
    }
    for name in ["connectionId", "requestId"] {
        let actual = c.get_property(payload, name)?;
        let expected = c.get_private(
            response,
            if name == "connectionId" {
                CONNECTION_ID
            } else {
                REQUEST_ID
            },
        )?;
        if c.as_string(actual)? != c.as_string(expected)? {
            return Err(invalid("node_http_input_identity_mismatch"));
        }
    }
    let seq = c.get_property(payload, "sequence")?;
    let seq = c.as_number(seq)?;
    let next = c.get_private(r, NEXT)?;
    if seq != c.as_number(next)? {
        return Err(invalid("node_http_input_sequence_mismatch"));
    }
    let version = c.get_property(payload, "version")?;
    if c.as_number(version)? != 1.0 {
        return Err(invalid("node_http_input_version_unsupported"));
    }
    match event {
        DATA => {
            let q = c.get_private(r, QUEUE)?;
            if flag(c, r, PAUSED)? || c.value_kind(q)? != Kind::Undefined {
                return Err(invalid("node_http_input_credit_exceeded"));
            }
            let bytes = c.get_property(payload, "bytes")?;
            let bytes = buffer::encode(&c.as_string(bytes)?, "base64")?;
            if bytes.is_empty() || bytes.len() > MAX_CHUNK {
                return Err(invalid("node_http_input_chunk_size_invalid"));
            }
            let b = buffer::from_bytes(c, &bytes)?;
            c.set_private(r, QUEUE, b)?;
            let started = c.bool(true)?;
            c.set_private(r, DATA_STARTED, started)?;
        }
        END => {
            let q = c.get_private(r, QUEUE)?;
            if c.value_kind(q)? != Kind::Undefined {
                return Err(invalid("node_http_end_requires_consumed_input"));
            }
            set_flag(c, r, EOF, true)?;
        }
        ABORT | ERROR => {
            set_flag(c, r, TERMINAL, true)?;
            let empty = c.undefined();
            c.set_private(r, QUEUE, empty)?;
            let next = c.number(seq + 1.0)?;
            c.set_private(r, NEXT, next)?;
            return if event == ABORT {
                emit(c, r, "aborted", &[])
            } else {
                let reason = c.get_property(payload, "reason")?;
                emit(c, r, "error", &[reason])
            };
        }
        _ => return Err(invalid("node_http_input_event_unknown")),
    }
    let next = c.number(seq + 1.0)?;
    c.set_private(r, NEXT, next)?;
    if event == DATA && !flag(c, r, FLOWING)? {
        let out = emit(c, r, "readable", &[])?;
        if !matches!(out, ModuleCallResult::Return(_)) {
            return Ok(out);
        }
    }
    drain(c, r)
}
