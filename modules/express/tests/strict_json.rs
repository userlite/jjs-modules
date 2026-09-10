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
                        id: "org.jjs.node-buffer".into(),
                        version: "0.1.0".into(),
                        implementation: "jjs-module-node-buffer-v1".into(),
                    },
                    imports: vec!["buffer".into()],
                },
                ModuleSelection {
                    identity: ModuleIdentity {
                        id: "org.jjs.node-http".into(),
                        version: "0.1.0".into(),
                        implementation: "jjs-module-node-http-v4".into(),
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
                contract_version: 2,
                completion: CompletionMode::Sync,
                schema: "jjs.http.stream.v2".into(),
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
  let result = false;
  middleware(request(body, contentType), {}, function (error) { result = error.name === expected && error.status === 400; });
  return result;
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
    let result = run(source);
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

fn run(source: &str) -> RunResult {
    let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(NodeHttpModule::default()))
        .unwrap();
    provider
        .add_implementation(Arc::new(ExpressModule::default()))
        .unwrap();
    provider
        .add_implementation(Arc::new(jjs_module_node_buffer::BufferModule::default()))
        .unwrap();
    let mut host = TestHost;
    let runtime = RuntimeBuilder::new(&provider.build(), &host)
        .build_font_empty()
        .unwrap();
    let result = runtime.run(&program, &mut host, &[]).unwrap();
    result
}

#[test]
fn urlencoded_flat_options_decoding_limits_and_parser_selection() {
    let result = run(r#"
const express = require('express');
const form = express.urlencoded({extended:false,limit:'1kb'});
let count=0;
function check(condition) { if (!condition) throw new Error('form assertion '+count); count++; }
function parse(raw, type) {
  const req={body:raw,headers:{'content-type':type}};
  let calls=0;
  form(req,{},function(err){if(err)throw new Error(err.message);calls++;});
  form(req,{},function(err){if(err)throw new Error(err.message);calls++;});
  check(calls===2);
  return req.body;
}
const body=parse('a=1&a=2&title=hello+world&u=%E2%82%AC&empty=&flag&nested[x]=3&__proto__=no&constructor=yes','application/x-www-form-urlencoded; charset=UTF-8');
check(body.a.length===2 && body.a[1]==='2');
check(body.title==='hello world' && body.u==='€');
check(body.empty==='' && body.flag==='');
check(body['nested[x]']==='3' && body.constructor==='yes');
check(Object.keys(body).indexOf('__proto__')===-1);
const bad=parse('a=%ZZ&b=%E0%A4&c=%FF','application/x-www-form-urlencoded');
check(bad.a==='%ZZ' && bad.b==='%E0%A4' && bad.c==='%FF');
check(Object.keys(parse('','application/x-www-form-urlencoded')).length===0);
check(parse('raw','application/octet-stream')==='raw');
for(const opts of [{extended:true},{extended:0},{type:'text/plain'},{inflate:true},{limit:0},{limit:'bad'},{parameterLimit:2}]) {
 let rejected=false;try{express.urlencoded(opts);}catch(e){rejected=e.name==='TypeError';}check(rejected);
}
for(const entry of [[express.urlencoded({limit:3}),'a=12',413], [form,'a=1',415]]) {
 let calls=0;
 const req={body:entry[1],headers:{'content-type': entry[2]===415?'application/x-www-form-urlencoded; charset=latin1':'application/x-www-form-urlencoded'}};
 entry[0](req,{},function(e){check(e.status===entry[2]);calls++;});check(calls===1);
}
const req={body:'a=1',headers:{'content-type':'application/x-www-form-urlencoded'}};
express.json()(req,{},function(e){check(!e);});
form(req,{},function(e){check(!e && req.body.a==='1');});
true;
"#);
    assert!(
        matches!(
            result,
            RunResult::Halt {
                output: Value::Bool(true),
                ..
            }
        ),
        "{result:?}"
    );
}
