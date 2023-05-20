use actix_files::Files;
use actix_web::web::Json;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;
use std::{env, sync::Arc, sync::RwLock};

#[derive(Debug, Serialize, Clone)]
pub struct TransactionResult {
    pub timestamp: u64,
    pub dex_names: Vec<String>,
    pub token_symbols: Vec<String>,
    pub amounts: Vec<f64>,
    pub profit: f64,
}

pub async fn start_server(
    transaction_results: Arc<RwLock<Vec<TransactionResult>>>,
) -> std::io::Result<()> {
    let port = env::var("PORT").unwrap_or("5000".to_string());

    HttpServer::new(move || {
        App::new()
            .app_data(transaction_results.clone())
            .route(
                "/transaction_results",
                web::get().to(transaction_results_handler),
            )
            .service(Files::new("/dashboard", "./static"))
    })
    .bind("0.0.0.0:".to_owned() + &port)?
    .run()
    .await
}

async fn transaction_results_handler(
    transaction_results: web::Data<Arc<RwLock<Vec<TransactionResult>>>>,
) -> impl Responder {
    let transaction_results_arc = transaction_results.into_inner();
    let transaction_results_guard = transaction_results_arc.read().unwrap();
    HttpResponse::Ok().json(Json(&*transaction_results_guard))
}
