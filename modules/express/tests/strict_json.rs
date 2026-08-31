use jjs::jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostModuleCatalog, ModuleIdentity,
    ModuleProviderBuilder, ModuleSelection,
};
use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, RunResult, RuntimeBuilder, Value,
};
use jjs_module_express::ExpressModule;
use jjs_module_node_http::NodeHttpModule;
use std::sync::Arc;

struct TestHost;

impl Host for TestHost {
    fn call(&mut self, name: &str, _args: &[Value]) -> jjs::Result<HostResult> {
        Err(jjs::Error::Host(format!("unexpected host call {name}")))
    }
}

impl ModuleHost for TestHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        HostModuleCatalog {
            selections: vec![
                ModuleSelection {
                    identity: ModuleIdentity {
                        id: "org.jjs.node-http".into(),
                        version: "0.1.0".into(),
                        implementation: "jjs-module-node-http-v1".into(),
                    },
                    imports: vec!["node:http".into()],
                },
                ModuleSelection {
                    identity: ModuleIdentity {
                        id: "org.jjs.express".into(),
                        version: "0.1.0".into(),
                        implementation: "jjs-module-express-v1".into(),
                    },
                    imports: vec!["express".into()],
                },
            ],
        }
    }

    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![
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
        ]
    }
}

#[test]
fn json_matches_standard_defaults_options_and_content_types() {
    let source = r#"
const express = require('express');
const middleware = express.json();
function request(body, contentType) {
  return { body: body, headers: { 'content-type': contentType } };
}
let nextCalls = 0;
let score = 0;
if (typeof middleware === 'function') score++;
let object = request('{"a":1}', 'application/json; charset=utf-8');
middleware(object, {}, function () { nextCalls++; });
if (object.body.a === 1) score++;

function failure(body, contentType, expected) {
  try { middleware(request(body, contentType), {}, function () { nextCalls++; }); }
  catch (error) { return error.name === expected; }
  return false;
}
let empty = request('', 'application/json');
middleware(empty, {}, function () { nextCalls++; });
if (typeof empty.body === 'object') score++;
if (failure('{', 'application/json', 'ExpressJsonSyntaxError')) score++;
if (failure('1', 'application/json', 'ExpressJsonStrictError')) score++;

const loose = express.json({ limit: '1kb', strict: false });
if (typeof loose === 'function') score++;
let primitive = request('1', 'application/json');
loose(primitive, {}, function () { nextCalls++; });
if (primitive.body === 1) score++;

let plain = request('{}', 'text/plain');
middleware(plain, {}, function () { nextCalls++; });
if (plain.body === '{}') score++;
let missing = { body: '', headers: {} };
middleware(missing, {}, function () { nextCalls++; });
if (missing.body === '') score++;
if (typeof express.json({}) === 'function') score++;
let optionsRejected = false;
try { express.json({ limit: 8, strict: true, extra: true }); }
catch (error) { optionsRejected = error.name === 'TypeError'; }
if (nextCalls === 5) score++;
if (optionsRejected) score++;
score;
"#;
    let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(NodeHttpModule::default()))
        .unwrap();
    provider
        .add_implementation(Arc::new(ExpressModule::default()))
        .unwrap();
    let mut host = TestHost;
    let runtime = RuntimeBuilder::new(&provider.build(), &host)
        .build_font_empty()
        .unwrap();
    let result = runtime.run(&program, &mut host, &[]).unwrap();
    assert!(
        matches!(
            result,
            RunResult::Halt {
                output: Value::Number(12.0),
                ..
            }
        ),
        "{result:?}"
    );
}
