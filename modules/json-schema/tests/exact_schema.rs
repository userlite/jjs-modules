use jjs::jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleIdentity, ModuleProviderBuilder,
    ModuleSelection,
};
use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, RunResult, RuntimeBuilder, Value,
};
use jjs_module_json_schema::JsonSchemaModule;
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
            selections: vec![ModuleSelection {
                identity: ModuleIdentity {
                    id: "org.jjs.json-schema".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-json-schema-v1".into(),
                },
                imports: vec!["jjs-schema".into()],
            }],
        }
    }

    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![]
    }
}

#[test]
fn exact_objects_reject_missing_unknown_and_wrong_types_without_coercion() {
    let source = r#"
const schema = require('jjs-schema');
const question = {
  text: schema.string({ minLength: 1, maxLength: 160 }),
  votes: schema.safeInteger({ minimum: 0, maximum: 1000 }),
  active: schema.boolean(),
  tags: schema.array(schema.string({ minLength: 1, maxLength: 10 }), { minLength: 1, maxLength: 3 }),
  author: schema.object({ name: schema.string({ minLength: 1, maxLength: 30 }) })
};
let initialError = '';
try {
  schema.exactObject({ text: 'Useful?', votes: 0, active: true, tags: ['jjs'], author: { name: 'Ada' } }, question);
} catch (error) { initialError = error.name + ':' + error.message; }

let rejected = 0;
const invalid = [
  { text: 'missing fields' },
  { text: 'extra', votes: 0, active: true, tags: ['jjs'], author: { name: 'Ada' }, extra: true },
  { text: 'coercion', votes: '0', active: true, tags: ['jjs'], author: { name: 'Ada' } },
  { text: '', votes: 0, active: true, tags: ['jjs'], author: { name: 'Ada' } },
  { text: 'nested extra', votes: 0, active: true, tags: ['jjs'], author: { name: 'Ada', extra: true } }
];
for (let index = 0; index < invalid.length; index++) {
  try { schema.exactObject(invalid[index], question); }
  catch (error) { if (error.name === 'SchemaValidationError') rejected++; }
}
let unknownKeyword = false;
try { schema.string({ maxLength: 10, coerce: true }); }
catch (error) { unknownKeyword = error.name === 'TypeError'; }
initialError === '' ? rejected * 10 + (unknownKeyword ? 1 : 0) : -1;
"#;
    let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(JsonSchemaModule::default()))
        .unwrap();
    let mut host = TestHost;
    let runtime = RuntimeBuilder::new(&provider.build(), &host)
        .build_font_empty()
        .unwrap();
    match runtime.run(&program, &mut host, &[]).unwrap() {
        RunResult::Halt { output, .. } => assert_eq!(output, Value::Number(51.0), "{output:?}"),
        other => panic!("unexpected result {other:?}"),
    }
}
