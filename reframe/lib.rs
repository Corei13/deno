use std::borrow::Cow;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_error::JsErrorBox;

fn normalize_path(path: &str) -> Cow<'_, str> {
  if path.contains("://") {
    Cow::Borrowed(path)
  } else if path.starts_with('/') {
    Cow::Owned(format!("file://{path}"))
  } else {
    Cow::Owned(format!("file:///{path}"))
  }
}

struct ReframeNS;

impl GarbageCollected for ReframeNS {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ReframeNS"
  }
}

#[op2]
impl ReframeNS {
  #[static_method]
  #[string]
  fn analyze(
    #[string] path: &str,
    #[string] content: &str,
    #[string] env: &str,
  ) -> Result<String, JsErrorBox> {
    let normalized_path = normalize_path(path);
    let result = analyze::analyze(normalized_path.as_ref(), content, env)
      .map_err(JsErrorBox::generic)?;
    serde_json::to_string(&result)
      .map_err(JsErrorBox::from_err)
  }
}

deno_core::extension!(
  deno_reframe,
  objects = [ReframeNS],
  esm = ["01_reframe.js"]
);
