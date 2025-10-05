use crate::config::SiteConfig;
use crate::error::{Result, GuLoaderError};
use reqwest::{Client, Response};
use std::time::Duration;

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("GuLoader/1.0 (Professional Manga Monitoring System)")
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client }
    }
    
    pub async fn get(&self, url: &str, config: &SiteConfig) -> Result<String> {
        let mut request = self.client.get(url);
        
        // Add custom user agent if specified
        if let Some(user_agent) = &config.user_agent {
            request = request.header("User-Agent", user_agent);
        }
        
        // Add custom headers if specified
        if let Some(headers) = &config.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            return Err(GuLoaderError::Http(
                response.error_for_status().unwrap_err()
            ));
        }
        
        let text = response.text().await?;
        Ok(text)
    }
    
    pub async fn get_raw(&self, url: &str) -> Result<Response> {
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(GuLoaderError::Http(
                response.error_for_status().unwrap_err()
            ));
        }
        
        Ok(response)
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}
