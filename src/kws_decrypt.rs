use base64::{decode_config, STANDARD};
use openssl::symm::{decrypt, Cipher};
use rusoto_core::Region;
use rusoto_kms::{DecryptRequest, Kms, KmsClient};
use std::{env, str::FromStr};

pub async fn decrypt_data_with_kms() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let region_name = env::var("AWS_REGION").unwrap_or_else(|_| String::from("eu-central-1"));
    let region = Region::from_str(&region_name)?;

    let client = KmsClient::new(region);

    // the encrypted data key (received from AWS KMS) passed via environment variable
    let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")
        .expect("Specify your encrypted data key")
        .replace(" ", ""); // Remove whitespace characters
    let encrypted_data_key = decode_config(&encrypted_data_key, STANDARD)
        .map_err(|err| format!("Failed to decode encrypted data key: {}", err))?;

    // the actual data (e.g. private key) to be decrypted, passed via environment variable
    let encrypted_data = env::var("ENCRYPTED_DATA")
        .expect("Specify your encrypted data")
        .replace(" ", ""); // Remove whitespace characters
    let encrypted_data = decode_config(&encrypted_data, STANDARD)
        .map_err(|err| format!("Failed to decode encrypted data: {}", err))?;

    // creating a request to decrypt the data key
    let decrypt_request = DecryptRequest {
        ciphertext_blob: encrypted_data_key.into(),
        ..Default::default()
    };

    let decrypt_response = client.decrypt(decrypt_request).await?;
    let decrypted_data_key = decrypt_response
        .plaintext
        .ok_or("Failed to decrypt the data key")?;

    // decrypt the actual data
    let decrypted_data = decrypt(
        Cipher::aes_256_cbc(),
        &decrypted_data_key,
        Some(&encrypted_data[..16]), // the first 16 bytes is the IV
        &encrypted_data[16..],       // the rest is the actual encrypted data
    )?;

    Ok(decrypted_data)
}
