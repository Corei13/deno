use std::borrow::Cow;
use std::sync::OnceLock;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_error::JsErrorBox;
use tokio::sync::oneshot;

enum Strategy {
  Spawn,
  Rayon,
}

struct ThreadConfig {
  strategy: Strategy,
  max_threads: usize,
}

static CONFIG: OnceLock<ThreadConfig> = OnceLock::new();
static RAYON_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn normalize_path(path: &str) -> Cow<'_, str> {
  if path.contains("://") {
    Cow::Borrowed(path)
  } else if path.starts_with('/') {
    Cow::Owned(format!("file://{path}"))
  } else {
    Cow::Owned(format!("file:///{path}"))
  }
}

fn do_analyze(
  path: &str,
  content: &str,
  env: &str,
) -> Result<String, JsErrorBox> {
  let normalized_path = normalize_path(path);
  let result = analyze::analyze(normalized_path.as_ref(), content, env)
    .map_err(JsErrorBox::generic)?;
  serde_json::to_string(&result).map_err(JsErrorBox::from_err)
}

fn default_threads() -> usize {
  std::thread::available_parallelism()
    .map(|value| value.get())
    .unwrap_or(1)
}

fn config() -> &'static ThreadConfig {
  CONFIG.get_or_init(|| {
    let strategy = match std::env::var("REFRAME_THREAD_STRATEGY")
      .ok()
      .map(|value| value.trim().to_ascii_lowercase())
      .as_deref()
    {
      Some("rayon") => Strategy::Rayon,
      Some("spawn") | None => Strategy::Spawn,
      Some(_) => Strategy::Spawn,
    };

    let max_threads = std::env::var("REFRAME_MAX_THREADS")
      .ok()
      .and_then(|value| value.trim().parse::<usize>().ok())
      .unwrap_or_else(default_threads);

    ThreadConfig {
      strategy,
      max_threads,
    }
  })
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
    do_analyze(path, content, env)
  }
}

#[op2(async)]
#[string]
async fn op_reframe_analyze(
  #[string] path: String,
  #[string] content: String,
  #[string] env: String,
) -> Result<String, JsErrorBox> {
  if config().max_threads == 0 {
    return do_analyze(&path, &content, &env);
  }

  match config().strategy {
    Strategy::Spawn => tokio::task::spawn_blocking(move || {
      do_analyze(&path, &content, &env)
    })
    .await
    .map_err(|error| {
      JsErrorBox::generic(format!("reframe analyze task failed: {error}"))
    })?,
    Strategy::Rayon => {
      let (tx, rx) = oneshot::channel();
      let pool = RAYON_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
          .num_threads(config().max_threads.max(1))
          .build()
          .expect("failed to create reframe rayon thread pool")
      });

      pool.spawn(move || {
        let result = do_analyze(&path, &content, &env);
        let _ = tx.send(result);
      });
      rx.await.map_err(|error| {
        JsErrorBox::generic(format!("reframe analyze worker failed: {error}"))
      })?
    },
  }
}

deno_core::extension!(
  deno_reframe,
  ops = [op_reframe_analyze],
  objects = [ReframeNS],
  esm = ["01_reframe.js"]
);
