//! Separate bounded WHATWG URL and legacy request-target parsers.
use jjs_module_api::{
    ModuleCallResult as Out, ModuleContext as Ctx, ModuleContinuation, ModuleError as Err,
    ModuleFunctionKey as Key, ModuleIdentity, ModuleManifest, ModuleObjectKind,
    ModuleValueKind as Kind, NativeModule, ValueHandle as V, MODULE_API_VERSION,
};
use jjs_module_node_querystring::{bounded, fault, null, thrown, MAX_BYTES, MAX_PAIRS};
use url::Url;
const BRAND: u32 = 4;
const DATA: u32 = 1;
const OWNER: u32 = 2;
const PARAMS: u32 = 3;
const FIELDS: [&str; 12] = [
    "href",
    "origin",
    "protocol",
    "username",
    "password",
    "host",
    "hostname",
    "port",
    "pathname",
    "search",
    "hash",
    "searchParams",
];
fn text(c: &mut dyn Ctx, v: V) -> Result<String, Err> {
    let s = c.as_string(v)?;
    bounded(c, &s)?;
    Ok(s)
}
fn save(c: &mut dyn Ctx, o: V, s: &str) -> Result<(), Err> {
    bounded(c, s)?;
    let v = c.string(s)?;
    c.set_private(o, DATA, v)
}
fn read_url(c: &mut dyn Ctx, o: V) -> Result<Url, Err> {
    let brand = c.get_private(o, BRAND)?;
    if c.as_number(brand)? != 1.0 {
        return Err(fault("url_invalid_receiver"));
    }
    let v = c.get_private(o, DATA)?;
    let s = text(c, v)?;
    Url::parse(&s).map_err(|_| fault("url_invalid_saved_state"))
}
fn method(c: &mut dyn Ctx, o: V, name: &str, key: u32) -> Result<(), Err> {
    let f = c.function(Key(key))?;
    c.set_property(o, name, f)
}
fn accessor(c: &mut dyn Ctx, o: V, name: &str, get: u32, set: u32) -> Result<(), Err> {
    let getter = c.function(Key(get))?;
    let setter = c.function(Key(set))?;
    c.define_accessor(o, name, getter, setter)
}
fn new_params(c: &mut dyn Ctx, query: &str, owner: Option<V>) -> Result<V, Err> {
    let o = c.module_object(ModuleObjectKind(2))?;
    let brand = c.number(2.0)?;
    c.set_private(o, BRAND, brand)?;
    save(c, o, query)?;
    let owner = owner.unwrap_or_else(|| c.undefined());
    c.set_private(o, OWNER, owner)?;
    for (i, name) in [
        "toString", "get", "getAll", "has", "append", "set", "delete", "sort",
    ]
    .iter()
    .enumerate()
    {
        method(c, o, name, 40 + i as u32)?;
    }
    accessor(c, o, "size", 48, 99)?;
    Ok(o)
}
fn pairs(c: &mut dyn Ctx, o: V) -> Result<Vec<(String, String)>, Err> {
    let brand = c.get_private(o, BRAND)?;
    if c.as_number(brand)? != 2.0 {
        return Err(fault("url_params_invalid_receiver"));
    }
    let owner = c.get_private(o, OWNER)?;
    let query = if c.value_kind(owner)? == Kind::Undefined {
        let v = c.get_private(o, DATA)?;
        text(c, v)?
    } else {
        read_url(c, owner)?.query().unwrap_or("").to_owned()
    };
    let mut out = Vec::new();
    for (k, v) in form_urlencoded::parse(query.as_bytes()) {
        c.charge_fuel(1)?;
        if out.len() >= MAX_PAIRS {
            return Err(fault("parser_pair_limit"));
        }
        out.push((k.into_owned(), v.into_owned()));
    }
    Ok(out)
}
fn serialize(pairs: &[(String, String)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k, v)))
        .finish()
}
fn save_pairs(c: &mut dyn Ctx, o: V, pairs: &[(String, String)]) -> Result<(), Err> {
    if pairs.len() > MAX_PAIRS {
        return Err(fault("parser_pair_limit"));
    }
    let query = serialize(pairs);
    bounded(c, &query)?;
    let owner = c.get_private(o, OWNER)?;
    if c.value_kind(owner)? == Kind::Undefined {
        save(c, o, &query)
    } else {
        let mut url = read_url(c, owner)?;
        url.set_query(if query.is_empty() { None } else { Some(&query) });
        save(c, owner, url.as_str())
    }
}
fn new_url(c: &mut dyn Ctx, url: Url) -> Result<V, Err> {
    let o = c.module_object(ModuleObjectKind(1))?;
    let brand = c.number(1.0)?;
    c.set_private(o, BRAND, brand)?;
    save(c, o, url.as_str())?;
    let p = new_params(c, url.query().unwrap_or(""), Some(o))?;
    let _ = pairs(c, p)?;
    c.set_private(o, PARAMS, p)?;
    for (i, name) in FIELDS.iter().enumerate() {
        accessor(c, o, name, 10 + i as u32, 70 + i as u32)?;
    }
    method(c, o, "toString", 30)?;
    method(c, o, "toJSON", 30)?;
    Ok(o)
}
fn params_call(c: &mut dyn Ctx, key: u32, o: V, args: &[V]) -> Result<Out, Err> {
    let expected = match key {
        40 | 47 | 48 => 0,
        44 | 45 => 2,
        _ => 1,
    };
    if args.len() != expected {
        return Ok(thrown(
            "TypeError",
            "URLSearchParams_arity_or_option_unsupported",
        ));
    }
    let mut p = pairs(c, o)?;
    let k = if expected > 0 {
        Some(text(c, args[0])?)
    } else {
        None
    };
    let value = match key {
        40 => {
            let query = serialize(&p);
            bounded(c, &query)?;
            c.string(&query)?
        }
        48 => c.number(p.len() as f64)?,
        41 => match p.iter().find(|(key, _)| Some(key) == k.as_ref()) {
            Some((_, v)) => c.string(v)?,
            None => null(c)?,
        },
        42 => {
            let a = c.array()?;
            for (key, v) in &p {
                if Some(key) == k.as_ref() {
                    let v = c.string(v)?;
                    c.array_push(a, v)?;
                }
            }
            a
        }
        43 => c.bool(p.iter().any(|(key, _)| Some(key) == k.as_ref()))?,
        44 | 45 | 46 | 47 => {
            match key {
                44 => {
                    let v = text(c, args[1])?;
                    p.push((k.unwrap(), v));
                }
                45 => {
                    let v = text(c, args[1])?;
                    let k = k.unwrap();
                    let first = p.iter().position(|(key, _)| key == &k);
                    p.retain(|(key, _)| key != &k);
                    let pos = first.unwrap_or(p.len());
                    p.insert(pos, (k, v));
                }
                46 => p.retain(|(key, _)| Some(key) != k.as_ref()),
                47 => p.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16())),
                _ => unreachable!(),
            }
            save_pairs(c, o, &p)?;
            c.undefined()
        }
        _ => return Err(fault("url_unknown_params_function")),
    };
    Ok(Out::Return(value))
}
fn legacy_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if " <>\"'`{}|^".contains(ch) {
            out.push_str(&format!("%{:02X}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}
fn legacy(c: &mut dyn Ctx, args: &[V]) -> Result<Out, Err> {
    if args.is_empty() || args.len() > 3 {
        return Ok(thrown("TypeError", "url_parse_arity"));
    }
    let input = text(c, args[0])?;
    let parse_query = match args.get(1) {
        None => false,
        Some(v) if c.value_kind(*v)? == Kind::Undefined => false,
        Some(v) if c.value_kind(*v)? == Kind::Bool => c.as_bool(*v)?,
        _ => return Ok(thrown("TypeError", "url_parse_query_flag_must_be_boolean")),
    };
    if let Some(v) = args.get(2) {
        if c.value_kind(*v)? != Kind::Undefined
            && (c.value_kind(*v)? != Kind::Bool || c.as_bool(*v)?)
        {
            return Ok(thrown(
                "TypeError",
                "url_parse_slashesDenoteHost_unsupported",
            ));
        }
    }
    let input = input.trim();
    if input.chars().any(|c| c.is_control()) {
        return Ok(thrown(
            "TypeError",
            "url_parse_control_characters_unsupported",
        ));
    }
    let (pre_hash, hash) = match input.split_once('#') {
        Some((a, b)) => (a, Some(format!("#{}", legacy_escape(b)))),
        None => (input, None),
    };
    let (path, search) = match pre_hash.split_once('?') {
        Some((a, b)) => (a, Some(format!("?{}", legacy_escape(b)))),
        None => (pre_hash, None),
    };
    let path = path.replace('\\', "/");
    let mut path = path.as_str();
    let mut protocol = None;
    let mut host = None;
    let mut hostname = None;
    let mut port = None;
    if let Some((scheme, rest)) = path.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Ok(thrown("TypeError", "url_parse_protocol_unsupported"));
        }
        protocol = Some(format!("{scheme}:"));
        let end = rest.find('/').unwrap_or(rest.len());
        let authority = &rest[..end];
        path = &rest[end..];
        let (name, p) = match authority.split_once(':') {
            Some((name, p)) => (name, Some(p)),
            None => (authority, None),
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
            || p.is_some_and(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Ok(thrown(
                "TypeError",
                "url_parse_authority_unsupported: ASCII host and numeric port only",
            ));
        }
        hostname = Some(name.to_ascii_lowercase());
        port = p.map(str::to_owned);
        host = Some(authority.to_ascii_lowercase());
    } else if path.split('/').next().unwrap_or("").contains(':') {
        return Ok(thrown("TypeError", "url_parse_protocol_unsupported"));
    } else if path.starts_with("//") && path.contains('@') {
        return Ok(thrown("TypeError", "url_parse_authority_unsupported"));
    }
    let pathname = if path.is_empty() {
        if protocol.is_some() {
            Some("/".to_owned())
        } else {
            None
        }
    } else {
        Some(legacy_escape(path))
    };
    let request_path = if pathname.is_some() || search.is_some() {
        Some(format!(
            "{}{}",
            pathname.as_deref().unwrap_or(""),
            search.as_deref().unwrap_or("")
        ))
    } else {
        None
    };
    let href = format!(
        "{}{}{}{}",
        protocol
            .as_ref()
            .map(|p| format!("{p}//"))
            .unwrap_or_default(),
        host.as_deref().unwrap_or(""),
        request_path.as_deref().unwrap_or(""),
        hash.as_deref().unwrap_or("")
    );
    bounded(c, &href)?;
    let o = c.object()?;
    for (name, value) in [
        ("protocol", protocol.clone()),
        ("slashes", None),
        ("auth", None),
        ("host", host),
        ("port", port),
        ("hostname", hostname),
        ("hash", hash),
        ("search", search.clone()),
        ("query", None),
        ("pathname", pathname),
        ("path", request_path),
        ("href", Some(href)),
    ] {
        let value = if name == "slashes" && protocol.is_some() {
            c.bool(true)?
        } else if name == "query" && parse_query {
            jjs_module_node_querystring::parse(
                c,
                search.as_deref().map(|s| &s[1..]).unwrap_or(""),
                "&",
                "=",
                1000,
            )?
        } else if name == "query" {
            match &search {
                Some(s) => c.string(&s[1..])?,
                None => null(c)?,
            }
        } else {
            match value {
                Some(s) => c.string(&s)?,
                None => null(c)?,
            }
        };
        c.set_property(o, name, value)?;
    }
    Ok(Out::Return(o))
}
pub struct UrlModule {
    manifest: ModuleManifest,
}
impl Default for UrlModule {
    fn default() -> Self {
        let mut keys = vec![1, 2, 3, 30, 99];
        keys.extend(10..22);
        keys.extend(40..49);
        keys.extend(70..82);
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-url".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-url-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["url".into(), "node:url".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: keys,
                object_kind_keys: vec![1, 2],
                deterministic_resources: vec![],
            },
        }
    }
}
impl NativeModule for UrlModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn Ctx) -> Result<Out, Err> {
        let o = c.object()?;
        for (name, key) in [("URL", 1), ("URLSearchParams", 2), ("parse", 3)] {
            method(c, o, name, key)?;
        }
        Ok(Out::Return(o))
    }
    fn call(&self, key: Key, _: V, o: V, args: &[V], c: &mut dyn Ctx) -> Result<Out, Err> {
        let key = key.0;
        if key == 99 {
            return Ok(thrown("TypeError", "url_property_is_read_only"));
        }
        if key == 3 {
            return legacy(c, args);
        }
        if (40..49).contains(&key) {
            return params_call(c, key, o, args);
        }
        if key == 1 {
            if !(1..=2).contains(&args.len()) {
                return Ok(thrown("TypeError", "URL_requires_input_and_optional_base"));
            }
            let input = text(c, args[0])?;
            let base = if args.len() == 2 && c.value_kind(args[1])? != Kind::Undefined {
                let s = text(c, args[1])?;
                match Url::parse(&s) {
                    Ok(u) => Some(u),
                    Err(_) => return Ok(thrown("TypeError", "Invalid URL base")),
                }
            } else {
                None
            };
            let url = match Url::options().base_url(base.as_ref()).parse(&input) {
                Ok(u) => u,
                Err(_) => return Ok(thrown("TypeError", "Invalid URL")),
            };
            return Ok(Out::Return(new_url(c, url)?));
        }
        if key == 2 {
            if args.len() > 1 {
                return Ok(thrown("TypeError", "URLSearchParams_constructor_arity"));
            }
            let query = match args.first() {
                None => String::new(),
                Some(v) if c.value_kind(*v)? == Kind::Undefined => String::new(),
                Some(v) if c.value_kind(*v)? == Kind::String => {
                    let s = text(c, *v)?;
                    s.strip_prefix('?').unwrap_or(&s).to_owned()
                }
                _ => {
                    return Ok(thrown(
                        "TypeError",
                        "URLSearchParams_only_string_constructor_supported",
                    ))
                }
            };
            let p = new_params(c, &query, None)?;
            let _ = pairs(c, p)?;
            return Ok(Out::Return(p));
        }
        if key == 21 {
            if !args.is_empty() {
                return Err(fault("url_getter_arity"));
            }
            return Ok(Out::Return(c.get_private(o, PARAMS)?));
        }
        if (70..82).contains(&key) {
            if ![70, 78, 79, 80].contains(&key) {
                return Ok(thrown("TypeError", "url_property_setter_unsupported"));
            }
            if args.len() != 1 {
                return Ok(thrown("TypeError", "url_setter_arity"));
            }
            let s = text(c, args[0])?;
            let mut u = read_url(c, o)?;
            match key {
                70 => {
                    u = match Url::parse(&s) {
                        Ok(u) => u,
                        Err(_) => return Ok(thrown("TypeError", "Invalid URL")),
                    };
                }
                78 => u.set_path(&s),
                79 => u.set_query(if s.is_empty() {
                    None
                } else {
                    Some(s.strip_prefix('?').unwrap_or(&s))
                }),
                80 => u.set_fragment(if s.is_empty() {
                    None
                } else {
                    Some(s.strip_prefix('#').unwrap_or(&s))
                }),
                _ => unreachable!(),
            }
            if u.as_str().len() > MAX_BYTES {
                return Err(fault("parser_output_limit"));
            }
            if u.query_pairs().count() > MAX_PAIRS {
                return Err(fault("parser_pair_limit"));
            }
            save(c, o, u.as_str())?;
            return Ok(Out::Return(c.undefined()));
        }
        if !args.is_empty() && !(key == 30 && args.len() == 1) {
            return Ok(thrown("TypeError", "url_method_arity"));
        }
        let u = read_url(c, o)?;
        let s = match key {
            10 | 30 => u.as_str().to_owned(),
            11 => u.origin().ascii_serialization(),
            12 => format!("{}:", u.scheme()),
            13 => u.username().into(),
            14 => u.password().unwrap_or("").into(),
            15 => match u.port() {
                Some(p) => format!("{}:{p}", u.host_str().unwrap_or("")),
                None => u.host_str().unwrap_or("").into(),
            },
            16 => u.host_str().unwrap_or("").into(),
            17 => u.port().map(|p| p.to_string()).unwrap_or_default(),
            18 => u.path().into(),
            19 => u
                .query()
                .filter(|s| !s.is_empty())
                .map(|s| format!("?{s}"))
                .unwrap_or_default(),
            20 => u
                .fragment()
                .filter(|s| !s.is_empty())
                .map(|s| format!("#{s}"))
                .unwrap_or_default(),
            _ => return Err(fault("url_unknown_function")),
        };
        Ok(Out::Return(c.string(&s)?))
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[V],
        _: Result<V, String>,
        _: &mut dyn Ctx,
    ) -> Result<Out, Err> {
        Err(fault("url_never_yields"))
    }
    fn event(&self, _: u32, _: V, _: V, _: &mut dyn Ctx) -> Result<Out, Err> {
        Err(fault("url_has_no_events"))
    }
}
