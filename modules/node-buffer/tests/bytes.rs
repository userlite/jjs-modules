use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, NullHost, RunResult, RuntimeBuilder,
    Value,
};
use jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_node_buffer::BufferModule;
use std::sync::Arc;
struct BufferHost;
impl Host for BufferHost {
    fn call(&mut self, n: &str, a: &[Value]) -> jjs::Result<HostResult> {
        NullHost.call(n, a)
    }
}
impl ModuleHost for BufferHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        let m = BufferModule::default();
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
fn runtime(host: &BufferHost) -> jjs::Runtime {
    let mut p = ModuleProviderBuilder::new();
    p.add_implementation(Arc::new(BufferModule::default()))
        .unwrap();
    RuntimeBuilder::new(&p.build(), host)
        .build_font_empty()
        .unwrap()
}
fn execute(source: &str, jit: bool) -> jjs::Result<RunResult> {
    jjs::discard_jit_cache();
    jjs::set_enabled(jit);
    let mut host = BufferHost;
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
        let result =
            execute(include_str!("bytes.js"), jit).unwrap_or_else(|e| panic!("jit={jit}: {e}"));
        let RunResult::Halt { output, .. } = result else {
            panic!("unexpected yield")
        };
        assert_eq!(
            output.as_str().unwrap(),
            include_str!("bytes.expected.json").trim(),
            "jit={jit}"
        );
    }
}

#[test]
fn buffers_survive_freeze_and_reject_unsupported_inputs() {
    for jit in [false, true] {
        let mut host = BufferHost;
        let rt = runtime(&host);
        let p=compile(&parse(&tokenize("const B=require('buffer').Buffer;var b=B.from([0,255,128]);http_get('pause');b[1]=129;var again=B.from([1]);if(again[0]!==1)throw new Error('new Buffer after thaw');return b.toString('hex') + ':' + b.length;").unwrap()).unwrap()).unwrap();
        jjs::set_enabled(jit);
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
        assert_eq!(output.as_str().unwrap(), "008180:3");
        for source in [
            "require('buffer').Buffer.alloc(-1)",
            "require('buffer').Buffer.alloc(16777217)",
            "require('buffer').Buffer.from('a','hex')",
            "require('buffer').Buffer.from('!','base64')",
            "require('buffer').Buffer.from('x','unknown')",
            "require('buffer').Buffer.concat([[1]])",
            "require('buffer').Buffer.from({})",
        ] {
            assert!(execute(source, jit).is_err(), "{source}");
        }
    }
}
#[test]
fn explicit_utf8_decoder_retains_split_scalars() {
    use jjs_module_node_buffer::decode_utf8;
    let mut pending = Vec::new();
    assert_eq!(decode_utf8(&mut pending, &[0xf0, 0x9f], false).unwrap(), "");
    assert_eq!(pending, vec![0xf0, 0x9f]);
    assert_eq!(
        decode_utf8(&mut pending, &[0x98, 0x80, 0xe2], false).unwrap(),
        "😀"
    );
    assert_eq!(decode_utf8(&mut pending, &[0x82], true).unwrap(), "�");
    assert!(pending.is_empty());
}
