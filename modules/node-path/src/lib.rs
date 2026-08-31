//! Node `path` compatibility for the POSIX session filesystem.

use jjs_module_api::{
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError, ModuleFunctionKey,
    ModuleIdentity, ModuleManifest, ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const RESOLVE: ModuleFunctionKey = ModuleFunctionKey(1);
const NORMALIZE: ModuleFunctionKey = ModuleFunctionKey(2);
const IS_ABSOLUTE: ModuleFunctionKey = ModuleFunctionKey(3);
const JOIN: ModuleFunctionKey = ModuleFunctionKey(4);
const RELATIVE: ModuleFunctionKey = ModuleFunctionKey(5);
const TO_NAMESPACED_PATH: ModuleFunctionKey = ModuleFunctionKey(6);
const DIRNAME: ModuleFunctionKey = ModuleFunctionKey(7);
const BASENAME: ModuleFunctionKey = ModuleFunctionKey(8);
const EXTNAME: ModuleFunctionKey = ModuleFunctionKey(9);
const FORMAT: ModuleFunctionKey = ModuleFunctionKey(10);
const PARSE: ModuleFunctionKey = ModuleFunctionKey(11);

pub struct NodePathModule {
    manifest: ModuleManifest,
}

impl Default for NodePathModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-path".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-node-path-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["path".into(), "node:path".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: (1..=11).collect(),
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}

fn type_error(message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "TypeError".into(),
        message: message.into(),
    }
}

fn normalize_posix(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|part| *part != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            part => parts.push(part),
        }
    }
    let mut result = if absolute {
        "/".to_owned()
    } else {
        String::new()
    };
    result.push_str(&parts.join("/"));
    if result.is_empty() {
        result.push('.');
    }
    if trailing && result != "/" && result != "." {
        result.push('/');
    }
    result
}

fn resolve_posix(paths: &[String]) -> String {
    let mut combined = String::new();
    for path in paths {
        if path.is_empty() {
            continue;
        }
        if path.starts_with('/') {
            combined.clear();
            combined.push_str(path);
        } else {
            if !combined.ends_with('/') && !combined.is_empty() {
                combined.push('/');
            }
            combined.push_str(path);
        }
    }
    if !combined.starts_with('/') {
        combined.insert(0, '/');
    }
    normalize_posix(&combined)
}

fn join_posix(paths: &[String]) -> String {
    let joined = paths
        .iter()
        .filter(|path| !path.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("/");
    normalize_posix(&joined)
}

fn trimmed_path(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() && path.starts_with('/') {
        "/"
    } else {
        trimmed
    }
}

fn basename_posix(path: &str) -> String {
    let path = trimmed_path(path);
    path.rsplit('/').next().unwrap_or_default().to_owned()
}

fn dirname_posix(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let path = trimmed_path(path);
    let Some(index) = path.rfind('/') else {
        return ".".to_owned();
    };
    let directory = path[..index].trim_end_matches('/');
    if directory.is_empty() {
        "/".to_owned()
    } else {
        directory.to_owned()
    }
}

fn extname_posix(path: &str) -> String {
    let base = basename_posix(path);
    if base == "." || base == ".." {
        return String::new();
    }
    match base.rfind('.') {
        Some(0) | None => String::new(),
        Some(index) => base[index..].to_owned(),
    }
}

fn string_args(
    context: &dyn ModuleContext,
    args: &[ValueHandle],
    operation: &str,
) -> Result<Vec<String>, ModuleCallResult> {
    args.iter()
        .map(|value| {
            context
                .as_string(*value)
                .map_err(|_| type_error(format!("path.{operation} arguments must be strings")))
        })
        .collect()
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

fn optional_string_property(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
) -> Result<String, ModuleCallResult> {
    let value = context
        .get_property(object, name)
        .map_err(|error| type_error(error.to_string()))?;
    match context
        .value_kind(value)
        .map_err(|error| type_error(error.to_string()))?
    {
        ModuleValueKind::Undefined => Ok(String::new()),
        ModuleValueKind::String => context
            .as_string(value)
            .map_err(|_| type_error(format!("path.format {name} must be a string"))),
        _ => Err(type_error(format!("path.format {name} must be a string"))),
    }
}

impl NativeModule for NodePathModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [
            ("resolve", RESOLVE),
            ("normalize", NORMALIZE),
            ("isAbsolute", IS_ABSOLUTE),
            ("join", JOIN),
            ("relative", RELATIVE),
            ("toNamespacedPath", TO_NAMESPACED_PATH),
            ("dirname", DIRNAME),
            ("basename", BASENAME),
            ("extname", EXTNAME),
            ("format", FORMAT),
            ("parse", PARSE),
        ] {
            let function = context.function(key)?;
            context.set_property(exports, name, function)?;
        }
        set_string(context, exports, "sep", "/")?;
        set_string(context, exports, "delimiter", ":")?;
        context.set_property(exports, "posix", exports)?;
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        _callee: ValueHandle,
        _receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let result = match key {
            RESOLVE => {
                let paths = match string_args(context, args, "resolve") {
                    Ok(paths) => paths,
                    Err(error) => return Ok(error),
                };
                context.string(&resolve_posix(&paths))?
            }
            NORMALIZE | IS_ABSOLUTE | TO_NAMESPACED_PATH | DIRNAME | EXTNAME => {
                if args.len() != 1 {
                    return Ok(type_error(
                        "path operation requires exactly one path string",
                    ));
                }
                let path = match context.as_string(args[0]) {
                    Ok(path) => path,
                    Err(_) => return Ok(type_error("path must be a string")),
                };
                if key == IS_ABSOLUTE {
                    return Ok(ModuleCallResult::Return(
                        context.bool(path.starts_with('/'))?,
                    ));
                }
                let value = match key {
                    NORMALIZE => normalize_posix(&path),
                    TO_NAMESPACED_PATH => path,
                    DIRNAME => dirname_posix(&path),
                    EXTNAME => extname_posix(&path),
                    _ => unreachable!(),
                };
                context.string(&value)?
            }
            JOIN => {
                let paths = match string_args(context, args, "join") {
                    Ok(paths) => paths,
                    Err(error) => return Ok(error),
                };
                context.string(&join_posix(&paths))?
            }
            RELATIVE => {
                if args.len() != 2 {
                    return Ok(type_error("path.relative requires two path strings"));
                }
                let paths = match string_args(context, args, "relative") {
                    Ok(paths) => paths,
                    Err(error) => return Ok(error),
                };
                let from = resolve_posix(&[paths[0].clone()]);
                let to = resolve_posix(&[paths[1].clone()]);
                let from = from
                    .split('/')
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>();
                let to = to
                    .split('/')
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>();
                let common = from
                    .iter()
                    .zip(&to)
                    .take_while(|(left, right)| left == right)
                    .count();
                let mut parts = vec![".."; from.len() - common];
                parts.extend_from_slice(&to[common..]);
                context.string(&parts.join("/"))?
            }
            BASENAME => {
                if !(1..=2).contains(&args.len()) {
                    return Ok(type_error(
                        "path.basename requires a path and optional suffix",
                    ));
                }
                let path = match context.as_string(args[0]) {
                    Ok(path) => path,
                    Err(_) => return Ok(type_error("path must be a string")),
                };
                let mut base = basename_posix(&path);
                if let Some(suffix) = args.get(1) {
                    let suffix = match context.as_string(*suffix) {
                        Ok(suffix) => suffix,
                        Err(_) => return Ok(type_error("path.basename suffix must be a string")),
                    };
                    if base.ends_with(&suffix) {
                        base.truncate(base.len() - suffix.len());
                    }
                }
                context.string(&base)?
            }
            FORMAT => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
                    return Ok(type_error("path.format requires one path object"));
                }
                let object = args[0];
                let mut optional = |name| match optional_string_property(context, object, name) {
                    Ok(value) => Ok(value),
                    Err(error) => Err(error),
                };
                let dir = match optional("dir") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let root = match optional("root") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let base = match optional("base") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let name = match optional("name") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                let mut ext = match optional("ext") {
                    Ok(value) => value,
                    Err(error) => return Ok(error),
                };
                if !ext.is_empty() && !ext.starts_with('.') {
                    ext.insert(0, '.');
                }
                let file = if base.is_empty() {
                    format!("{name}{ext}")
                } else {
                    base
                };
                let directory = if dir.is_empty() { root } else { dir };
                let value = if directory.is_empty() {
                    file
                } else if directory == "/" {
                    format!("/{file}")
                } else {
                    format!("{directory}/{file}")
                };
                context.string(&value)?
            }
            PARSE => {
                if args.len() != 1 {
                    return Ok(type_error("path.parse requires exactly one path string"));
                }
                let path = match context.as_string(args[0]) {
                    Ok(path) => path,
                    Err(_) => return Ok(type_error("path must be a string")),
                };
                let root = if path.starts_with('/') { "/" } else { "" };
                let dir = dirname_posix(&path);
                let dir = if dir == "." { "".to_owned() } else { dir };
                let base = basename_posix(&path);
                let ext = extname_posix(&path);
                let name = base.strip_suffix(&ext).unwrap_or(&base).to_owned();
                let object = context.object()?;
                set_string(context, object, "root", root)?;
                set_string(context, object, "dir", &dir)?;
                set_string(context, object, "base", &base)?;
                set_string(context, object, "ext", &ext)?;
                set_string(context, object, "name", &name)?;
                return Ok(ModuleCallResult::Return(object));
            }
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown node:path function key".into(),
                ));
            }
        };
        Ok(ModuleCallResult::Return(result))
    }

    fn resume(
        &self,
        _continuation: ModuleContinuation,
        _state: &[ValueHandle],
        _completion: Result<ValueHandle, String>,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "node:path never yields".into(),
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
            "node:path has no events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_path_operations_match_node_shapes() {
        assert_eq!(normalize_posix("/a//b/../c/"), "/a/c/");
        assert_eq!(normalize_posix("../../a"), "../../a");
        assert_eq!(
            join_posix(&["/a".into(), "b".into(), "..".into(), "c".into()]),
            "/a/c"
        );
        assert_eq!(
            resolve_posix(&["a".into(), "/b".into(), "c".into()]),
            "/b/c"
        );
        assert_eq!(dirname_posix("/a/b.txt"), "/a");
        assert_eq!(basename_posix("/a/b.txt"), "b.txt");
        assert_eq!(extname_posix("/a/b.txt"), ".txt");
        assert_eq!(extname_posix("/.env"), "");
    }
}
