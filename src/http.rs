use actix_web::web::Json;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;
use std::{env, sync::Arc, sync::RwLock};

#[derive(Debug, Serialize)]
pub struct PriceData {
    pub timestamp: u64,
    pub token_pair: (String, String),
    pub dex_prices: Vec<(String, f64)>,
    pub profit: f64,
}

pub async fn start_server(price_history: Arc<RwLock<Vec<PriceData>>>) -> std::io::Result<()> {
    let port = env::var("PORT").unwrap_or("5000".to_string());

    HttpServer::new(move || {
        App::new()
            .app_data(price_history.clone())
            .route("/price_history", web::get().to(price_history_handler))
    })
    .bind("0.0.0.0:".to_owned() + &port)?
    .run()
    .await
}

async fn price_history_handler(
    price_history: web::Data<Arc<RwLock<Vec<PriceData>>>>,
) -> impl Responder {
    let price_history_arc = price_history.into_inner();
    let price_history_guard = price_history_arc.read().unwrap();
    HttpResponse::Ok().json(Json(&*price_history_guard))
}
