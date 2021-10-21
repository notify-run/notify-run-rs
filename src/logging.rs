use tracing_stackdriver::Stackdriver;
use tracing_subscriber::{EnvFilter, Registry};
use tracing_subscriber::layer::SubscriberExt;

const LOG_MODULES: &[&str] = &[
    "notify_run",
];

pub fn init_logging() {
    let mut env_filter = EnvFilter::default();
    for module in LOG_MODULES {
        env_filter = env_filter.add_directive(
            format!("{}=info", module)
                .parse()
                .expect("Could not parse logging directive"),
        );
    }

    if std::env::var("LOG_JSON").is_ok() {
        let stackdriver = Stackdriver::default();
        let subscriber = Registry::default().with(stackdriver).with(env_filter);

        tracing::subscriber::set_global_default(subscriber)
            .expect("Could not set up global logger");
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
}
