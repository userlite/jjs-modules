//! Explicit Buffer subset backed by JJS compact, budgeted byte storage.
use base64::Engine as _;
use jjs_module_api::{
    ModuleCallResult as Out, ModuleContext as Ctx, ModuleContinuation, ModuleError as Err,
    ModuleFunctionKey as Key, ModuleIdentity, ModuleManifest, ModuleObjectKind,
    ModuleValueKind as Kind, NativeModule, ValueHandle as V, MODULE_API_VERSION,
};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub fn identity() -> ModuleIdentity {
    ModuleIdentity {
        id: "org.jjs.node-buffer".into(),
        version: "0.1.0".into(),
        implementation: "jjs-module-node-buffer-v1".into(),
    }
}
fn fail(s: &str) -> Err {
    Err::ContractViolation(s.into())
}
fn thrown(s: &str) -> Out {
    Out::Throw {
        name: "TypeError".into(),
        message: s.into(),
    }
}
pub fn encoding(s: &str) -> Result<&'static str, Err> {
    match s.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok("utf8"),
        "latin1" | "binary" => Ok("latin1"),
        "ascii" => Ok("ascii"),
        "hex" => Ok("hex"),
        "base64" => Ok("base64"),
        _ => Err(fail("buffer_encoding_unsupported")),
    }
}
pub fn encode(s: &str, enc: &str) -> Result<Vec<u8>, Err> {
    if s.len() > MAX_BYTES * 2 {
        return Err(fail("buffer_length_limit"));
    }
    let result = match encoding(enc)? {
        "utf8" => s.as_bytes().to_vec(),
        "latin1" | "ascii" => s.encode_utf16().map(|n| n as u8).collect(),
        "hex" => {
            if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(fail("buffer_hex_requires_complete_valid_pairs"));
            }
            s.as_bytes()
                .chunks_exact(2)
                .map(|p| {
                    let a = (p[0] as char).to_digit(16).unwrap();
                    let b = (p[1] as char).to_digit(16).unwrap();
                    ((a << 4) | b) as u8
                })
                .collect()
        }
        "base64" => base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|_| fail("buffer_base64_requires_valid_standard_encoding"))?,
        _ => unreachable!(),
    };
    if result.len() > MAX_BYTES {
        return Err(fail("buffer_length_limit"));
    }
    Ok(result)
}
/// Node-compatible explicit UTF-8 decoding, with pending incomplete scalars.
pub fn decode_utf8(pending: &mut Vec<u8>, chunk: &[u8], last: bool) -> Result<String, Err> {
    if pending.len() + chunk.len() > MAX_BYTES {
        return Err(fail("buffer_length_limit"));
    }
    pending.extend_from_slice(chunk);
    let mut out = String::new();
    let mut pos = 0;
    while pos < pending.len() {
        match std::str::from_utf8(&pending[pos..]) {
            Ok(s) => {
                out.push_str(s);
                pos = pending.len();
            }
            Err(e) => {
                let valid = e.valid_up_to();
                out.push_str(
                    std::str::from_utf8(&pending[pos..pos + valid])
                        .expect("validated UTF-8 prefix"),
                );
                pos += valid;
                match e.error_len() {
                    Some(n) => {
                        out.push('\u{FFFD}');
                        pos += n;
                    }
                    None if last => {
                        out.push('\u{FFFD}');
                        pos = pending.len();
                    }
                    None => break,
                }
            }
        }
    }
    pending.drain(..pos);
    Ok(out)
}
pub fn decode(bytes: &[u8], enc: &str) -> Result<String, Err> {
    if bytes.len() > MAX_BYTES {
        return Err(fail("buffer_length_limit"));
    }
    Ok(match encoding(enc)? {
        "utf8" => decode_utf8(&mut Vec::new(), bytes, true)?,
        "latin1" => bytes.iter().map(|b| char::from(*b)).collect(),
        "ascii" => bytes.iter().map(|b| char::from(*b & 127)).collect(),
        "hex" => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        "base64" => base64::engine::general_purpose::STANDARD.encode(bytes),
        _ => unreachable!(),
    })
}
fn size(c: &dyn Ctx, v: V) -> Result<usize, Err> {
    let n = c.as_number(v)?;
    if !n.is_finite() || n < 0.0 || n.fract() != 0.0 || n > MAX_BYTES as f64 {
        return Err(fail("buffer_size_invalid"));
    }
    Ok(n as usize)
}
fn enc_arg(c: &dyn Ctx, args: &[V], i: usize) -> Result<String, Err> {
    match args.get(i) {
        None => Ok("utf8".into()),
        Some(v) if c.value_kind(*v)? == Kind::Undefined => Ok("utf8".into()),
        Some(v) => Ok(encoding(&c.as_string(*v)?)?.into()),
    }
}
fn create(c: &mut dyn Ctx, bytes: &[u8]) -> Result<V, Err> {
    let o = c.module_object(ModuleObjectKind(1))?;
    c.init_bytes(o, bytes)?;
    let f = c.function(Key(7))?;
    c.set_property(o, "toString", f)?;
    let f = c.function(Key(8))?;
    c.set_property(o, "toJSON", f)?;
    Ok(o)
}
pub struct BufferModule {
    manifest: ModuleManifest,
}
impl Default for BufferModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: identity(),
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["buffer".into(), "node:buffer".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: (1..=8).collect(),
                object_kind_keys: vec![1],
                deterministic_resources: vec![],
            },
        }
    }
}
impl NativeModule for BufferModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn Ctx) -> Result<Out, Err> {
        let o = c.object()?;
        let b = c.function(Key(1))?;
        for (i, name) in ["from", "alloc", "concat", "isBuffer", "byteLength"]
            .iter()
            .enumerate()
        {
            let f = c.function(Key(i as u32 + 2))?;
            c.set_property(b, name, f)?;
        }
        c.set_property(o, "Buffer", b)?;
        Ok(Out::Return(o))
    }
    fn call(&self, k: Key, _: V, o: V, args: &[V], c: &mut dyn Ctx) -> Result<Out, Err> {
        let result = (|| {
            if k.0 == 1 {
                return Ok(thrown("Use Buffer.from or Buffer.alloc"));
            }
            if k.0 == 5 {
                if args.len() != 1 {
                    return Ok(thrown("Buffer.isBuffer arity"));
                }
                let yes = c.is_bytes(args[0]);
                return Ok(Out::Return(c.bool(yes)?));
            }
            if k.0 == 7 {
                if args.len() > 3 {
                    return Ok(thrown("Buffer.toString arity"));
                }
                let bytes = c.read_bytes(o)?;
                c.charge_fuel(bytes.len() as u64 + 1)?;
                let enc = enc_arg(c, args, 0)?;
                let start = if let Some(v) = args.get(1) {
                    size(c, *v)?.min(bytes.len())
                } else {
                    0
                };
                let end = if let Some(v) = args.get(2) {
                    size(c, *v)?.min(bytes.len())
                } else {
                    bytes.len()
                };
                let text = decode(&bytes[start..end.max(start)], &enc)?;
                return Ok(Out::Return(c.string(&text)?));
            }
            if k.0 == 8 {
                if args.len() > 1 {
                    return Ok(thrown("Buffer.toJSON arity"));
                }
                let bytes = c.read_bytes(o)?;
                let data = c.array()?;
                for b in bytes {
                    c.charge_fuel(1)?;
                    let v = c.number(b as f64)?;
                    c.array_push(data, v)?;
                }
                let result = c.object()?;
                let ty = c.string("Buffer")?;
                c.set_property(result, "type", ty)?;
                c.set_property(result, "data", data)?;
                return Ok(Out::Return(result));
            }
            if args.is_empty() {
                return Ok(thrown("Buffer argument required"));
            }
            let bytes = match k.0 {
                2 => {
                    if args.len() > 2 {
                        return Ok(thrown("Buffer.from arity"));
                    }
                    if c.is_bytes(args[0]) {
                        if args.len() != 1 {
                            return Ok(thrown("Buffer.from encoding only applies to strings"));
                        }
                        c.read_bytes(args[0])?
                    } else {
                        match c.value_kind(args[0])? {
                            Kind::String => {
                                let s = c.as_string(args[0])?;
                                c.charge_fuel(s.len() as u64 + 1)?;
                                encode(&s, &enc_arg(c, args, 1)?)?
                            }
                            Kind::Array => {
                                if args.len() != 1 {
                                    return Ok(thrown("Buffer.from array encoding unsupported"));
                                }
                                let len = c.array_len(args[0])?;
                                if len > MAX_BYTES {
                                    return Err(fail("buffer_length_limit"));
                                }
                                let mut data = Vec::with_capacity(len);
                                for i in 0..len {
                                    c.charge_fuel(1)?;
                                    let v = c.array_get(args[0], i)?;
                                    let n = c.as_number(v)?;
                                    data.push(if n.is_finite() {
                                        n.trunc().rem_euclid(256.0) as u8
                                    } else {
                                        0
                                    });
                                }
                                data
                            }
                            _ => {
                                return Ok(thrown(
                                    "Buffer.from requires string, numeric array, or Buffer",
                                ))
                            }
                        }
                    }
                }
                3 => {
                    if args.len() > 3 {
                        return Ok(thrown("Buffer.alloc arity"));
                    }
                    let len = size(c, args[0])?;
                    c.charge_fuel(len as u64 + 1)?;
                    let mut data = vec![0; len];
                    if let Some(fill) = args.get(1) {
                        let fill = if c.is_bytes(*fill) {
                            c.read_bytes(*fill)?
                        } else {
                            match c.value_kind(*fill)? {
                                Kind::Undefined => vec![0],
                                Kind::Number => {
                                    let n = c.as_number(*fill)?;
                                    vec![if n.is_finite() {
                                        n.trunc().rem_euclid(256.0) as u8
                                    } else {
                                        0
                                    }]
                                }
                                Kind::String => {
                                    encode(&c.as_string(*fill)?, &enc_arg(c, args, 2)?)?
                                }
                                _ => return Ok(thrown("Buffer.alloc fill unsupported")),
                            }
                        };
                        if !fill.is_empty() {
                            for (i, b) in data.iter_mut().enumerate() {
                                *b = fill[i % fill.len()];
                            }
                        }
                    }
                    data
                }
                4 => {
                    if args.len() > 2 || c.value_kind(args[0])? != Kind::Array {
                        return Ok(thrown("Buffer.concat requires Buffer array"));
                    }
                    let mut data = Vec::new();
                    let count = c.array_len(args[0])?;
                    for i in 0..count {
                        c.charge_fuel(1)?;
                        let v = c.array_get(args[0], i)?;
                        let bytes = c.read_bytes(v)?;
                        c.charge_fuel(bytes.len() as u64 + 1)?;
                        if data.len() + bytes.len() > MAX_BYTES {
                            return Err(fail("buffer_length_limit"));
                        }
                        data.extend(bytes);
                    }
                    if let Some(v) = args.get(1) {
                        data.resize(size(c, *v)?, 0);
                    }
                    data
                }
                6 => {
                    if args.len() > 2 {
                        return Ok(thrown("Buffer.byteLength arity"));
                    }
                    let len = if c.is_bytes(args[0]) {
                        c.read_bytes(args[0])?.len()
                    } else {
                        let s = c.as_string(args[0])?;
                        c.charge_fuel(s.len() as u64 + 1)?;
                        encode(&s, &enc_arg(c, args, 1)?)?.len()
                    };
                    return Ok(Out::Return(c.number(len as f64)?));
                }
                _ => return Err(fail("buffer_unknown_function")),
            };
            Ok(Out::Return(create(c, &bytes)?))
        })();
        match result {
            Err(Err::ContractViolation(message))
                if message.starts_with("buffer_")
                    || matches!(
                        message.as_str(),
                        "expected byte buffer"
                            | "expected string"
                            | "expected number"
                            | "expected array"
                    ) =>
            {
                Ok(Out::Throw {
                    name: if matches!(
                        message.as_str(),
                        "buffer_length_limit" | "buffer_size_invalid"
                    ) {
                        "RangeError"
                    } else {
                        "TypeError"
                    }
                    .into(),
                    message,
                })
            }
            other => other,
        }
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[V],
        _: Result<V, String>,
        _: &mut dyn Ctx,
    ) -> Result<Out, Err> {
        Err(fail("buffer_never_yields"))
    }
    fn event(&self, _: u32, _: V, _: V, _: &mut dyn Ctx) -> Result<Out, Err> {
        Err(fail("buffer_has_no_events"))
    }
}

/// Create through the selected Buffer module, retaining its ownership and methods.
pub fn from_bytes(c: &mut dyn Ctx, bytes: &[u8]) -> Result<V, Err> {
    let exports = c.import("node:buffer")?;
    let constructor = c.get_property(exports, "Buffer")?;
    let from = c.get_property(constructor, "from")?;
    if !c.is_callable(from) {
        return Err(fail("buffer_provider_from_not_callable"));
    }
    let data = c.string(&decode(bytes, "base64")?)?;
    let encoding = c.string("base64")?;
    c.call(from, constructor, &[data, encoding])
}
