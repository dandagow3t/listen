use anyhow::Result;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info};
use url::Url;

// Global SOL price cache
pub static SOL_PRICE_CACHE: Lazy<SolPriceCache> = Lazy::new(SolPriceCache::new);

#[derive(Debug, Deserialize)]
struct TradeData {
    p: String,
}

#[derive(Debug, Deserialize)]
struct BinancePrice {
    price: String,
}

#[derive(Debug, Clone)]
pub struct SolPriceCache {
    price: Arc<RwLock<f64>>,
}

impl Default for SolPriceCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SolPriceCache {
    pub fn new() -> Self {
        Self {
            price: Arc::new(RwLock::new(0.0)),
        }
    }

    pub async fn set_price(&self, price: f64) {
        *self.price.write().await = price;
    }

    pub async fn get_price(&self) -> f64 {
        let current_price = *self.price.read().await;
        if current_price == 0.0 {
            match self.fetch_rest_price().await {
                Ok(rest_price) => {
                    *self.price.write().await = rest_price;
                    rest_price
                }
                Err(e) => {
                    error!("Failed to fetch REST price: {}", e);
                    current_price
                }
            }
        } else {
            current_price
        }
    }

    async fn fetch_rest_price(&self) -> Result<f64> {
        let rest_url =
            "https://api.binance.com/api/v3/ticker/price?symbol=SOLUSDT";
        let response = reqwest::get(rest_url).await?;
        let price_data: BinancePrice = response.json().await?;
        price_data.price.parse::<f64>().map_err(Into::into)
    }

    pub async fn start_price_stream(&self) -> Result<()> {
        let url = Url::parse("wss://stream.binance.com:9443/ws/solusdt@trade")?;
        let (ws_stream, _) = connect_async(url).await?;
        let price = self.price.clone();
        info!("WebSocket connected to Binance SOL/USDT stream");

        let (_, mut read) = ws_stream.split();

        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<TradeData>(&text) {
                        Ok(trade) => {
                            if let Ok(new_price) = trade.p.parse::<f64>() {
                                *price.write().await = new_price;
                            }
                        }
                        Err(e) => error!("Error parsing JSON: {}", e),
                    }
                }
                Ok(Message::Ping(_)) => {}
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_sol_price_cache() {
        let price_cache = SolPriceCache::new();
        let price_cache_clone = price_cache.clone();

        // Spawn the price stream in a separate task
        tokio::spawn(async move {
            if let Err(e) = price_cache.start_price_stream().await {
                error!("Error in price stream: {}", e);
            }
        });

        // Wait a bit for the first price update
        sleep(Duration::from_secs(2)).await;

        let price = price_cache_clone.get_price().await;
        info!("Current SOL price: ${:.3}", price);
        assert!(price > 0.0, "Price should be greater than 0");
    }

    #[tokio::test]
    async fn test_rest_fallback() {
        let price_cache = SolPriceCache::new();

        // Test initial state (should trigger REST fallback)
        let price = price_cache.get_price().await;
        info!("Initial SOL price from REST: ${:.3}", price);
        assert!(price > 0.0, "REST fallback price should be greater than 0");

        // Test that the price was cached
        let cached_price = *price_cache.price.read().await;
        assert_eq!(
            price, cached_price,
            "Price should be cached after REST call"
        );
    }
}
