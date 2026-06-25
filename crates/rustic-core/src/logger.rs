use anyhow::Result;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_gcloud_trace::GcpCloudTraceExporterBuilder;
use opentelemetry_otlp::WithExportConfig;

use tracing::Level;
use tracing_subscriber::{
    EnvFilter, FmtSubscriber, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

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

pub async fn set_logger_with_telemetry(
    filter: String,
    service_name: &str,
    project_id: &str,
    endpoint: &str,
) -> Result<()> {
    let is_cloud = std::env::var("LOG_FORMAT").is_ok();
    println!("{}", filter);
    println!("Initializing telemetry for service: {}", service_name);

    let env_filter = EnvFilter::new(&filter);

    if is_cloud {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_target(true)
            .with_line_number(true)
            .flatten_event(true);

        let tracer = init_cloud_tracer(project_id).await?;
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(otel_layer)
            .with(fmt_layer)
            .init();
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::NONE) // ← no span open/close events
            .compact();

        let tracer = init_tracer(service_name, endpoint);
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(otel_layer)
            .with(fmt_layer)
            .init();
    }

    Ok(())
}

pub fn init_tracer(service_name: &str, endpoint: &str) -> opentelemetry_sdk::trace::Tracer {
    /*
        docker stop jaeger
    docker rm jaeger
    docker run -d --name jaeger \
      -p 16686:16686 \
      -p 4317:4317 \
      -p 4318:4318 \
      jaegertracing/all-in-one:latest
         */
    // let endpoint =  "https://cloudtrace.googleapis.com";

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http() // ← http instead of tonic
                .with_endpoint(endpoint),
        )
        .with_trace_config(opentelemetry_sdk::trace::Config::default().with_resource(
            opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                service_name.to_string(),
            )]),
        ))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .expect("Failed to initialize tracer provider");

    opentelemetry::global::set_tracer_provider(provider.clone());
    provider.tracer(service_name.to_string())
}

pub async fn init_cloud_tracer(project_id: &str) -> Result<opentelemetry_sdk::trace::Tracer> {
    let tracer = GcpCloudTraceExporterBuilder::new(project_id.to_string())
        .install()
        .await
        .expect("Failed to create GCP exporter");

    Ok(tracer)
}
