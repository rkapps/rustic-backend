use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub fn set_logger(filter: String) {
    // let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
    //     "bin_shared=info,fin_services=trace,fin_providers=trace,fin_core=debug,agentic_core::agent=debug,fin_tracker_pipeline=info,fin_tracker_admin=info,fin_tracker_api=info".to_string()
    // });

    let is_cloud = std::env::var("LOG_FORMAT").is_ok();

    println!("{}", filter);
    if is_cloud {
        let subscriber = FmtSubscriber::builder()
            .json()
            .with_env_filter(filter)
            .with_current_span(false)
            .with_span_list(false)
            .with_target(true)
            .with_line_number(true)
            .flatten_event(true) // ← this is the key
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    } else {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .with_target(true)
            .with_line_number(true)
            .with_env_filter(filter)
            .compact()
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    };
}
