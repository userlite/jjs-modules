//! Unsigned cookie parsing for Express applications.
use jjs_module_api::*;
use std::collections::BTreeMap;

pub fn parse_cookies(header: &str) -> BTreeMap<String, String> {
    let mut cookies = BTreeMap::new();
    for part in header.split(';') {
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || cookies.contains_key(name) {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(value);
        cookies.insert(name.into(), decode(value).unwrap_or_else(|| value.into()));
    }
    cookies
}
fn decode(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(b) = bytes.next() {
        out.push(if b == b'%' {
            let a = (bytes.next()? as char).to_digit(16)?;
            let b = (bytes.next()? as char).to_digit(16)?;
            (a * 16 + b) as u8
        } else {
            b
        });
    }
    String::from_utf8(out).ok()
}
pub struct CookieParserModule {
    manifest: ModuleManifest,
}
impl Default for CookieParserModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.cookie-parser".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-cookie-parser-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["cookie-parser".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: vec![1, 2],
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
impl NativeModule for CookieParserModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn ModuleContext) -> Result<ModuleCallResult, ModuleError> {
        let f = c.function(ModuleFunctionKey(1))?;
        c.set_property(f, "default", f)?;
        Ok(ModuleCallResult::Return(f))
    }
    fn call(
        &self,
        key: ModuleFunctionKey,
        _: ValueHandle,
        _: ValueHandle,
        args: &[ValueHandle],
        c: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if key.0 == 1 {
            if !args.is_empty() {
                return Ok(ModuleCallResult::Throw {
                    name: "TypeError".into(),
                    message:
                        "cookie-parser baseline accepts no options; signed cookies are unsupported"
                            .into(),
                });
            }
            return Ok(ModuleCallResult::Return(c.function(ModuleFunctionKey(2))?));
        }
        if key.0 != 2 || args.len() != 3 {
            return Err(ModuleError::ContractViolation(
                "cookie-parser requires req, res, next".into(),
            ));
        }
        let headers = c.get_property(args[0], "headers")?;
        let header = c.get_property(headers, "cookie")?;
        let header = if c.value_kind(header)? == ModuleValueKind::Undefined {
            String::new()
        } else {
            c.as_string(header)?
        };
        let cookies = c.object()?;
        for (name, value) in parse_cookies(&header) {
            // These names cannot be installed safely on a plain guest object.
            if matches!(name.as_str(), "__proto__" | "constructor" | "prototype") {
                continue;
            }
            let mut v = c.string(&value)?;
            if let Some(json) = value.strip_prefix("j:") {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json) {
                    let json = c.string(&parsed.to_string())?;
                    v = c.json_parse(json)?;
                }
            }
            c.set_property(cookies, &name, v)?;
        }
        c.set_property(args[0], "cookies", cookies)?;
        let signed = c.object()?;
        c.set_property(args[0], "signedCookies", signed)?;
        let undefined = c.undefined();
        c.call(args[2], undefined, &[])?;
        Ok(ModuleCallResult::Return(undefined))
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[ValueHandle],
        _: Result<ValueHandle, String>,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "cookie-parser has no continuation".into(),
        ))
    }
    fn event(
        &self,
        _: u32,
        _: ValueHandle,
        _: ValueHandle,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "cookie-parser has no events".into(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoding_duplicates_and_malformed_values() {
        let c =
            parse_cookies("a=hello%20world; a=second; plus=a+b; bad=%E0; quote=\"yes\"; ignored");
        assert_eq!(c["a"], "hello world");
        assert_eq!(c["plus"], "a+b");
        assert_eq!(c["bad"], "%E0");
        assert_eq!(c["quote"], "yes");
    }
}
