use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, NullHost, RunResult, RuntimeBuilder,
    Value,
};
use jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_node_url::UrlModule;
use std::sync::Arc;
struct ParserHost;
impl Host for ParserHost {
    fn call(&mut self, n: &str, a: &[Value]) -> jjs::Result<HostResult> {
        NullHost.call(n, a)
    }
}
impl ModuleHost for ParserHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        let m = UrlModule::default();
        HostModuleCatalog {
            selections: vec![ModuleSelection {
                identity: m.manifest().identity.clone(),
                imports: m.manifest().imports.clone(),
            }],
        }
    }
    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![]
    }
}
fn runtime(host: &ParserHost) -> jjs::Runtime {
    let mut p = ModuleProviderBuilder::new();
    p.add_implementation(Arc::new(UrlModule::default()))
        .unwrap();
    RuntimeBuilder::new(&p.build(), host)
        .build_font_empty()
        .unwrap()
}
fn execute(source: &str, jit: bool) -> jjs::Result<RunResult> {
    jjs::discard_jit_cache();
    jjs::set_enabled(jit);
    let mut host = ParserHost;
    let rt = runtime(&host);
    let p = compile(&parse(&tokenize(source)?)?)?;
    let probe = p
        .functions
        .iter()
        .position(|f| f.name.as_deref() == Some("probe"));
    if jit {
        if let Some(i) = probe {
            jjs::force_compile(&p, i as u32).expect("compile parser probe");
        }
    }
    let result = if jit {
        rt.run(&p, &mut host, &[])
    } else {
        rt.run_interpreted(&p, &mut host, &[])
    };
    if jit && result.is_ok() {
        if let Some(i) = probe {
            assert!(
                jjs::native_completion_count(&p, i as u32) > 0,
                "native parser caller must complete"
            );
        }
    }
    result
}
#[test]
fn differential_node_24_interpreter_and_native() {
    for jit in [false, true] {
        let result = execute(include_str!("fixtures/parsing.js"), jit)
            .unwrap_or_else(|e| panic!("jit={jit}: {e}"));
        let RunResult::Halt { output, .. } = result else {
            panic!("unexpected yield")
        };
        assert_eq!(
            output.as_str().unwrap(),
            include_str!("fixtures/parsing.expected.json").trim(),
            "jit={jit}"
        );
    }
}
#[test]
fn unknown_import_is_rejected() {
    for jit in [false, true] {
        assert!(execute("return require('node:parser-does-not-exist');", jit).is_err());
    }
}
#[test]
fn url_and_live_search_params_survive_freeze() {
    for jit in [false, true] {
        jjs::discard_jit_cache();
        jjs::set_enabled(jit);
        let mut host = ParserHost;
        let rt = runtime(&host);
        let source="const U=require('url').URL;var u=new U('/a?x=1','https://example.com');var p=u.searchParams;http_get('pause');p.append('x','2');u.pathname='/b';return u.href + '|' + p.getAll('x').join(',');";
        let p = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
        let r = if jit {
            rt.run(&p, &mut host, &[])
        } else {
            rt.run_interpreted(&p, &mut host, &[])
        }
        .unwrap();
        let RunResult::Yield { state, .. } = r else {
            panic!("yield")
        };
        let frozen = jjs::freeze(&p, &state).unwrap();
        let mut state = rt.thaw(&p, &frozen).unwrap();
        let RunResult::Halt { output, .. } = rt
            .resume_host(
                &p,
                &mut state,
                &mut host,
                jjs::HostCompletion::Success(Value::Undefined),
            )
            .unwrap()
        else {
            panic!("halt")
        };
        assert_eq!(
            output.as_str().unwrap(),
            "https://example.com/b?x=1&x=2|1,2"
        );
    }
}
#[test]
fn unsupported_url_options_and_limits_fail_explicitly() {
    for jit in [false, true] {
        for source in [
            "const P=require('url').URLSearchParams;new P({a:1});",
            "require('url').parse('//host/a',false,true);",
            "require('url').parse('mailto:a@b');",
            "const U=require('url').URL;var u=new U('https://example.com');u.protocol='http:';",
            "const P=require('url').URLSearchParams;new P('a=1').has('a','1');",
            "const P=require('url').URLSearchParams;new P().entries();",
        ] {
            assert!(execute(source, jit).is_err(), "{source}");
        }
        let source = format!(
            "const P=require('url').URLSearchParams;return new P('{}');",
            "x".repeat(65537)
        );
        let error = execute(&source, jit).unwrap_err();
        assert!(
            error.to_string().contains("parser_input_limit"),
            "jit={jit}: {error:?}"
        );
        let source = format!(
            "const P=require('url').URLSearchParams;return new P('{}');",
            vec!["x=1"; 4097].join("&")
        );
        assert!(execute(&source, jit)
            .unwrap_err()
            .to_string()
            .contains("parser_pair_limit"));
    }
}
