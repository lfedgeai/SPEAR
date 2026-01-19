#![deny(unsafe_op_in_unsafe_fn)]

use boa_engine::builtins::promise::PromiseState;
use boa_engine::js_string;
use boa_engine::module::ModuleLoader;
use boa_engine::object::builtins::JsPromise;
use boa_engine::Source;
use std::rc::Rc;

const DEFAULT_ENTRY: &str = r#"
import { Spear } from "spear";

export default async function main() {
  const tools = [
    Spear.tool({
      name: "sum",
      description: "Add two integers",
      parameters: {
        type: "object",
        properties: {
          a: { type: "integer" },
          b: { type: "integer" },
        },
        required: ["a", "b"],
      },
      handler: ({ a, b }) => {
        console.log("sum invoked:", "a=", a, "b=", b);
        return { sum: (a ?? 0) + (b ?? 0) };
      },
    }),
  ];

  const resp = await Spear.chat.completions.create({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Please call sum(a,b) for a=7 and b=35." }],
    tools,
    maxTotalToolCalls: 4,
    maxIterations: 4,
  });

  return resp.text();
}
"#;

fn main() {
    let loader = Rc::new(spear_boa::BuiltinModuleRegistry::new());
    let mut context = spear_boa::build_context(loader.clone()).expect("build context");
    spear_boa::init_tool_runtime(&mut context, spear_boa::DEFAULT_TOOL_SLOTS);
    spear_boa::install_native_bindings(&mut context);

    let src = std::env::var("SPEAR_JS_ENTRY").ok();
    let entry_code = match src {
        Some(path) => std::fs::read_to_string(path).unwrap_or_else(|_| DEFAULT_ENTRY.to_string()),
        None => DEFAULT_ENTRY.to_string(),
    };

    let specifier = "app";
    let entry_source = Source::from_bytes(entry_code.as_bytes());
    let module =
        boa_engine::module::Module::parse(entry_source, None, &mut context).expect("parse entry");
    loader.register_module(js_string!(specifier), module.clone());

    let _promise = module.load_link_evaluate(&mut context);
    context.run_jobs();

    let ns = module.namespace(&mut context);
    let entry = ns
        .get(js_string!("default"), &mut context)
        .or_else(|_| ns.get(js_string!("main"), &mut context))
        .expect("get entry export");
    let Some(func) = entry.as_callable().cloned() else {
        eprintln!("entry export is not callable (expected default export or named export main)");
        std::process::exit(2);
    };
    let res = func
        .call(&boa_engine::JsValue::Undefined, &[], &mut context)
        .expect("call entry export");
    context.run_jobs();

    let settled = if res.is_promise() {
        let p = JsPromise::from_object(res.as_object().cloned().unwrap()).expect("promise wrapper");
        for _ in 0..32 {
            if matches!(p.state(), PromiseState::Pending) {
                context.run_jobs();
            } else {
                break;
            }
        }
        match p.state() {
            PromiseState::Fulfilled(v) => v,
            PromiseState::Rejected(v) => {
                if let Ok(s) = v.to_string(&mut context) {
                    eprintln!("rejected: {}", s.to_std_string_escaped());
                } else {
                    eprintln!("rejected");
                }
                std::process::exit(1);
            }
            PromiseState::Pending => {
                eprintln!("promise still pending after draining job queue");
                std::process::exit(3);
            }
        }
    } else {
        res
    };

    if let Ok(s) = settled.to_string(&mut context) {
        println!("{}", s.to_std_string_escaped());
    }
}
