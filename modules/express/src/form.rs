//! Flat URL-encoded middleware; shares the bounded byte collector with JSON.
use super::*;

pub(super) fn create(
    c: &mut dyn ModuleContext,
    args: &[ValueHandle],
) -> Result<ModuleCallResult, ModuleError> {
    if args.len() > 1 {
        return Ok(type_throw(
            "express.urlencoded accepts at most one options object",
        ));
    }
    let mut limit = DEFAULT_JSON_LIMIT;
    if let Some(options) = args.first().copied() {
        if c.value_kind(options)? != ModuleValueKind::Object {
            return Ok(type_throw("express.urlencoded options must be an object"));
        }
        for name in c.own_property_names(options)? {
            let value = c.get_property(options, &name)?;
            match name.as_str() {
                "extended" => {
                    if c.value_kind(value)? != ModuleValueKind::Bool || c.as_bool(value)? {
                        return Ok(type_throw(
                            "express.urlencoded supports only extended: false",
                        ));
                    }
                }
                "limit" => {
                    limit = match c.value_kind(value)? {
                        ModuleValueKind::Number => {
                            let n = c.as_number(value)?;
                            if !n.is_finite() || n.fract() != 0.0 || !(1.0..=usize::MAX as f64).contains(&n) { return Ok(type_throw("express.urlencoded limit must be a positive integer or byte-size string")); }
                            n as usize
                        }
                        ModuleValueKind::String => {
                            let Some(n) = parse_json_limit(&c.as_string(value)?) else { return Ok(type_throw("express.urlencoded limit must be a positive integer or byte-size string")); };
                            n
                        }
                        _ => return Ok(type_throw("express.urlencoded limit must be a positive integer or byte-size string")),
                    };
                }
                _ => {
                    return Ok(type_throw(format!(
                        "express.urlencoded option {name:?} is not supported"
                    )))
                }
            }
        }
    }
    let middleware = c.function(FORM_MIDDLEWARE)?;
    let limit = c.number(limit as f64)?;
    let strict = c.bool(false)?;
    c.set_private(middleware, JSON_LIMIT, limit)?;
    c.set_private(middleware, JSON_STRICT, strict)?;
    Ok(ModuleCallResult::Return(middleware))
}

pub(super) fn parse_body(
    c: &mut dyn ModuleContext,
    request: ValueHandle,
    next: ValueHandle,
    bytes: &[u8],
) -> Result<ModuleCallResult, ModuleError> {
    // Express 5 flat mode preserves bracket keys, combines duplicates, ignores
    // __proto__, and retains an encoded component if percent decoding is invalid.
    let raw = String::from_utf8_lossy(bytes);
    if !raw.is_empty() && raw.split('&').count() > 1000 {
        return json_fail(
            c,
            next,
            "ExpressFormParameterLimitError",
            "express.urlencoded exceeds 1000 parameters",
            413,
        );
    }
    c.charge_fuel(bytes.len() as u64 + 1)?;
    let parsed = jjs_module_node_querystring::null_object(c)?;
    for part in raw.split('&').filter(|part| !part.is_empty()) {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let key = decode(key);
        if key == "__proto__" {
            continue;
        }
        let value = c.string(&decode(value))?;
        let previous = c.get_property(parsed, &key)?;
        match c.value_kind(previous)? {
            ModuleValueKind::Undefined => c.set_property(parsed, &key, value)?,
            ModuleValueKind::String => {
                let values = c.array()?;
                c.array_push(values, previous)?;
                c.array_push(values, value)?;
                c.set_property(parsed, &key, values)?;
            }
            ModuleValueKind::Array => c.array_push(previous, value)?,
            _ => {
                return Err(ModuleError::ContractViolation(
                    "express.urlencoded invalid field state".into(),
                ))
            }
        }
    }
    parsed_body(c, request, next, parsed)
}

fn decode(input: &str) -> String {
    let text = input.replace('+', " ");
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return text;
        }
    }
    match percent_encoding::percent_decode_str(&text).decode_utf8() {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => text, // Express/qs retains the entire malformed component.
    }
}
