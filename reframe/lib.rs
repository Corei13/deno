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
static SPAWN_SEMAPHORE: OnceLock<tokio::sync::Semaphore> =
  OnceLock::new();

const DEFAULT_RUST_MIN_STACK: usize = 8 * 1024 * 1024;

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
  minify: bool,
) -> Result<String, JsErrorBox> {
  let normalized_path = normalize_path(path);
  let result = analyze::analyze(normalized_path.as_ref(), content, env, minify)
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
    if std::env::var_os("RUST_MIN_STACK").is_none() {
      // SAFETY: set once during lazy config initialization before analyze worker pools are created.
      unsafe { std::env::set_var("RUST_MIN_STACK", DEFAULT_RUST_MIN_STACK.to_string()) };
    }

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
    minify: bool,
  ) -> Result<String, JsErrorBox> {
    do_analyze(path, content, env, minify)
  }
}

#[op2(async)]
#[string]
async fn op_reframe_analyze(
  #[string] path: String,
  #[string] content: String,
  #[string] env: String,
  minify: bool,
) -> Result<String, JsErrorBox> {
  if config().max_threads == 0 {
    return do_analyze(&path, &content, &env, minify);
  }

  match config().strategy {
    Strategy::Spawn => {
      let semaphore = SPAWN_SEMAPHORE.get_or_init(|| {
        tokio::sync::Semaphore::new(config().max_threads.max(1))
      });
      let _permit = semaphore.acquire().await.map_err(|error| {
        JsErrorBox::generic(format!("reframe semaphore closed: {error}"))
      })?;
      tokio::task::spawn_blocking(move || {
        do_analyze(&path, &content, &env, minify)
      })
      .await
      .map_err(|error| {
        JsErrorBox::generic(format!("reframe analyze task failed: {error}"))
      })?
    },
    Strategy::Rayon => {
      let (tx, rx) = oneshot::channel();
      let pool = RAYON_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
          .num_threads(config().max_threads.max(1))
          .build()
          .expect("failed to create reframe rayon thread pool")
      });

      pool.spawn(move || {
        let result = do_analyze(&path, &content, &env, minify);
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
