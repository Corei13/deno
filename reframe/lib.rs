use std::sync::Arc;

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileOptions;
use deno_ast::TranspileResult;
use deno_ast::parse_module;
use deno_core::GarbageCollected;
use deno_core::op2;

fn transpile(path: &str, content: &str) -> TranspileResult {
  let parsed = parse_module(ParseParams {
    specifier: ModuleSpecifier::parse(path).unwrap(),
    media_type: MediaType::TypeScript,
    text: content.into(),
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })
  .expect("Failed to parse module");

  let transpile_options = TranspileOptions {
    ..Default::default()
  };
  let transpile_module_options = TranspileModuleOptions {
    ..Default::default()
  };
  let emit_options = EmitOptions {
    remove_comments: false,
    ..Default::default()
  };
  let result = parsed
    .transpile(&transpile_options, &transpile_module_options, &emit_options)
    .expect("Failed to transpile module");

  result
}

fn analyze(path: &str, content: &str) {
  let source = transpile(path, content).into_source();
  dbg!(source);
}

struct ReframeNS;

impl GarbageCollected for ReframeNS {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ReframeNS"
  }
}

#[op2]
impl ReframeNS {
  #[fast]
  #[static_method]
  fn analyze(#[string] path: &str, #[string] content: &str) {
    analyze(path, content);
  }
}

deno_core::extension!(
  deno_reframe,
  objects = [ReframeNS],
  esm = ["01_reframe.js"]
);
