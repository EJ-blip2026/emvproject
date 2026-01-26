// Cloud storage import module for Google Drive, OneDrive, iCloud, and S3
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub mime_type: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub provider: String, // "google_drive", "onedrive", "icloud", "s3"
    pub file_ids: Vec<String>,
    pub vault_id: String,
}

// Google Drive OAuth and file listing
pub mod google_drive {
    use super::CloudFile;
    use reqwest::Client;
    use serde_json::Value;

    pub async fn list_files(access_token: &str) -> Result<Vec<CloudFile>, String> {
        let client = Client::new();
        let url = "https://www.googleapis.com/drive/v3/files?pageSize=100&fields=files(id,name,size,mimeType)";
        
        let response = client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to list files: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Drive API error: {}", response.status()));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let files = data["files"]
            .as_array()
            .ok_or("Invalid response format")?
            .iter()
            .filter_map(|f| {
                Some(CloudFile {
                    id: f["id"].as_str()?.to_string(),
                    name: f["name"].as_str()?.to_string(),
                    size: f["size"].as_str()?.parse().ok()?,
                    mime_type: f["mimeType"].as_str().map(|s| s.to_string()),
                    download_url: Some(format!(
                        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
                        f["id"].as_str()?
                    )),
                })
            })
            .collect();

        Ok(files)
    }

    pub async fn download_file(
        access_token: &str,
        file_id: &str,
    ) -> Result<Vec<u8>, String> {
        let client = Client::new();
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            file_id
        );

        let response = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Download error: {}", response.status()));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed to read bytes: {}", e))
    }
}

// OneDrive support (Microsoft Graph API)
pub mod onedrive {
    use super::CloudFile;
    use reqwest::Client;
    use serde_json::Value;

    pub async fn list_files(access_token: &str) -> Result<Vec<CloudFile>, String> {
        let client = Client::new();
        let url = "https://graph.microsoft.com/v1.0/me/drive/root/children";

        let response = client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to list files: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("OneDrive API error: {}", response.status()));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let files = data["value"]
            .as_array()
            .ok_or("Invalid response format")?
            .iter()
            .filter_map(|f| {
                Some(CloudFile {
                    id: f["id"].as_str()?.to_string(),
                    name: f["name"].as_str()?.to_string(),
                    size: f["size"].as_i64()?,
                    mime_type: f["file"]["mimeType"].as_str().map(|s| s.to_string()),
                    download_url: f["@microsoft.graph.downloadUrl"]
                        .as_str()
                        .map(|s| s.to_string()),
                })
            })
            .collect();

        Ok(files)
    }

    pub async fn download_file(download_url: &str) -> Result<Vec<u8>, String> {
        let client = Client::new();
        let response = client
            .get(download_url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Download error: {}", response.status()));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed to read bytes: {}", e))
    }
}

// AWS S3 support (using pre-signed URLs or access keys)
pub mod s3 {
    use super::CloudFile;
    use reqwest::Client;

    pub async fn download_file(
        bucket: &str,
        key: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Result<Vec<u8>, String> {
        // For MVP, use pre-signed URLs or public buckets
        // Production should use AWS SDK with proper signing
        let client = Client::new();
        let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket, region, key);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("S3 download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("S3 error: {}", response.status()));
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed to read bytes: {}", e))
    }
}
