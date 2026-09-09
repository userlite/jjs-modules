//! Bounded Node querystring codec; malformed decoding follows Node's replacement rule.
use jjs_module_api::{
    ModuleCallResult as Out, ModuleContext as Ctx, ModuleContinuation, ModuleError as Err,
    ModuleFunctionKey as Key, ModuleIdentity, ModuleManifest, ModuleValueKind as Kind,
    NativeModule, ValueHandle as V, MODULE_API_VERSION,
};
pub const MAX_BYTES: usize = 65536;
pub const MAX_PAIRS: usize = 4096;
pub fn fault(message: &str) -> Err {
    Err::ContractViolation(message.into())
}
pub fn thrown(name: &str, message: &str) -> Out {
    Out::Throw {
        name: name.into(),
        message: message.into(),
    }
}
pub fn null(c: &mut dyn Ctx) -> Result<V, Err> {
    let s = c.string("null")?;
    c.json_parse(s)
}
pub fn null_object(c: &mut dyn Ctx) -> Result<V, Err> {
    let object = c.global("Object")?;
    let create = c.get_property(object, "create")?;
    let n = null(c)?;
    c.call(create, object, &[n])
}
pub fn bounded(c: &mut dyn Ctx, s: &str) -> Result<(), Err> {
    if s.len() > MAX_BYTES {
        return Err(fault("parser_input_limit: 65536 UTF-8 bytes"));
    }
    c.charge_fuel(s.len() as u64 + 1)
}
pub fn decode(s: &str) -> String {
    percent_encoding::percent_decode_str(&s.replace('+', " "))
        .decode_utf8_lossy()
        .into_owned()
}
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
pub fn parse(c: &mut dyn Ctx, input: &str, sep: &str, eq: &str, max: usize) -> Result<V, Err> {
    bounded(c, input)?;
    let result = null_object(c)?;
    for (i, part) in input.split(sep).enumerate() {
        if max > 0 && i >= max {
            break;
        }
        if i >= MAX_PAIRS {
            return Err(fault("parser_pair_limit: 4096 fields"));
        }
        c.charge_fuel(1)?;
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once(eq).unwrap_or((part, ""));
        let key = decode(key);
        let value = c.string(&decode(value))?;
        let previous = c.get_property(result, &key)?;
        match c.value_kind(previous)? {
            Kind::Undefined => c.set_property(result, &key, value)?,
            Kind::String => {
                let a = c.array()?;
                c.array_push(a, previous)?;
                c.array_push(a, value)?;
                c.set_property(result, &key, a)?;
            }
            Kind::Array => c.array_push(previous, value)?,
            _ => return Err(fault("querystring_invalid_result_state")),
        }
    }
    Ok(result)
}
fn primitive(c: &dyn Ctx, v: V) -> Result<String, Err> {
    Ok(match c.value_kind(v)? {
        Kind::String | Kind::Bool => c.to_string(v)?,
        Kind::Number if c.as_number(v)?.is_finite() => c.to_string(v)?,
        _ => String::new(), // Node stringify explicitly maps non-primitives and non-finite numbers to empty text.
    })
}
fn stringify(c: &mut dyn Ctx, object: V, sep: &str, eq: &str) -> Result<V, Err> {
    if c.value_kind(object)? != Kind::Object && c.value_kind(object)? != Kind::Array {
        return c.string("");
    }
    let mut fields = Vec::new();
    let mut bytes = 0;
    let builtin = c.global("Object")?;
    let keys_fn = c.get_property(builtin, "keys")?;
    let is_array = c.value_kind(object)? == Kind::Array;
    let keys = if is_array {
        let len = c.array_len(object)?;
        if len > MAX_PAIRS {
            return Err(fault("parser_pair_limit"));
        }
        let keys = c.array()?;
        let has_own = c.get_property(builtin, "hasOwn")?;
        for i in 0..len {
            c.charge_fuel(1)?;
            let key = c.string(&i.to_string())?;
            let present = c.call(has_own, builtin, &[object, key])?;
            if c.as_bool(present)? {
                c.array_push(keys, key)?;
            }
        }
        keys
    } else {
        c.call(keys_fn, builtin, &[object])?
    };
    let count = c.array_len(keys)?;
    if count > MAX_PAIRS {
        return Err(fault("parser_pair_limit"));
    }
    for index in 0..count {
        let key = c.array_get(keys, index)?;
        let key = c.as_string(key)?;
        c.charge_fuel(key.len() as u64 + 1)?;
        let value = if is_array {
            c.array_get(
                object,
                key.parse()
                    .map_err(|_| fault("querystring_invalid_array_key"))?,
            )?
        } else {
            c.get_property(object, &key)?
        };
        let values = if c.value_kind(value)? == Kind::Array {
            let len = c.array_len(value)?;
            if len > MAX_PAIRS {
                return Err(fault("parser_pair_limit"));
            }
            let mut a = Vec::new();
            for i in 0..len {
                a.push(c.array_get(value, i)?);
            }
            a
        } else {
            vec![value]
        };
        for v in values {
            let v = primitive(c, v)?;
            bounded(c, &v)?;
            bounded(c, &key)?;
            let field = format!("{}{}{}", encode(&key), eq, encode(&v));
            bytes += field.len() + if fields.is_empty() { 0 } else { sep.len() };
            if bytes > MAX_BYTES || fields.len() >= MAX_PAIRS {
                return Err(fault("parser_output_limit"));
            }
            fields.push(field);
        }
    }
    c.string(&fields.join(sep))
}
pub struct QuerystringModule {
    manifest: ModuleManifest,
}
impl Default for QuerystringModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-querystring".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-querystring-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["querystring".into(), "node:querystring".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: vec![1, 2],
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
impl NativeModule for QuerystringModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn Ctx) -> Result<Out, Err> {
        let o = c.object()?;
        for (key, names) in [(1, ["parse", "decode"]), (2, ["stringify", "encode"])] {
            let f = c.function(Key(key))?;
            for name in names {
                c.set_property(o, name, f)?;
            }
        }
        Ok(Out::Return(o))
    }
    fn call(&self, key: Key, _: V, _: V, args: &[V], c: &mut dyn Ctx) -> Result<Out, Err> {
        if ![1, 2].contains(&key.0) {
            return Err(fault("querystring_unknown_function"));
        }
        if args.len() > 4 {
            return Ok(thrown("TypeError", "querystring_arity"));
        }
        let mut delimiters = Vec::new();
        for (i, default) in [(1, "&"), (2, "=")] {
            let text = match args.get(i) {
                None => default.into(),
                Some(v) if c.value_kind(*v)? == Kind::Undefined => default.into(),
                Some(v) if c.value_kind(*v)? == Kind::String => c.as_string(*v)?,
                _ => return Ok(thrown("TypeError", "querystring_separator_must_be_string")),
            };
            if text.is_empty() || text.len() > 16 {
                return Ok(thrown(
                    "TypeError",
                    "querystring_separator_unsupported: use 1..16 UTF-8 bytes",
                ));
            }
            delimiters.push(text);
        }
        let mut max = 1000;
        if let Some(options) = args.get(3) {
            if c.value_kind(*options)? != Kind::Undefined {
                if c.value_kind(*options)? != Kind::Object {
                    return Ok(thrown("TypeError", "querystring_options_must_be_object"));
                }
                for name in c.own_property_names(*options)? {
                    if key.0 != 1 || name != "maxKeys" {
                        return Ok(thrown("TypeError", "querystring_option_unsupported"));
                    }
                }
                if key.0 == 1 {
                    let v = c.get_property(*options, "maxKeys")?;
                    if c.value_kind(v)? != Kind::Undefined {
                        if c.value_kind(v)? != Kind::Number {
                            return Ok(thrown("TypeError", "querystring_maxKeys_must_be_integer"));
                        }
                        let n = c.as_number(v)?;
                        if !n.is_finite() || n.fract() != 0.0 || n < 0.0 || n > MAX_PAIRS as f64 {
                            return Ok(thrown("RangeError", "querystring_maxKeys_unsupported"));
                        }
                        max = n as usize;
                    }
                }
            }
        }
        let result = if key.0 == 1 {
            let s = match args.first() {
                Some(v) if c.value_kind(*v)? == Kind::String => c.as_string(*v)?,
                _ => String::new(),
            }; // Node parse(non-string) is an empty result.
            parse(c, &s, &delimiters[0], &delimiters[1], max)?
        } else {
            let object = args.first().copied().unwrap_or_else(|| c.undefined());
            stringify(c, object, &delimiters[0], &delimiters[1])?
        };
        Ok(Out::Return(result))
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[V],
        _: Result<V, String>,
        _: &mut dyn Ctx,
    ) -> Result<Out, Err> {
        Err(fault("querystring_never_yields"))
    }
    fn event(&self, _: u32, _: V, _: V, _: &mut dyn Ctx) -> Result<Out, Err> {
        Err(fault("querystring_has_no_events"))
    }
}
