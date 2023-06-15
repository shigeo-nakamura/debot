use rusoto_core::{Region, RusotoError};
use rusoto_kms::{DecryptRequest, Kms, KmsClient};
use openssl::symm::{decrypt, Cipher};
use std::env;
use base64::decode;

pub fn decrypt_data_with_kms() -> Result<String, Box<dyn std::error::Error>> {
    // you can set up the region by the environment variable `AWS_REGION`
    let region_name = env::var("AWS_REGION").unwrap_or_else(|_| String::from("us-east-1"));
    let region = Region::from_str(&region_name)?;

    let client = KmsClient::new(region);

    // the encrypted data key (received from AWS KMS) passed via environment variable
    let encrypted_data_key = env::var("ENCRYPTED_DATA_KEY")?;
    let encrypted_data_key = decode(&encrypted_data_key)?;

    // the actual data (e.g. private key) to be decrypted, passed via environment variable
    let encrypted_data = env::var("ENCRYPTED_DATA")?;
    let encrypted_data = decode(&encrypted_data)?;

    // creating a request to decrypt the data key
    let decrypt_request = DecryptRequest {
        ciphertext_blob: encrypted_data_key.into(),
        ..Default::default()
    };

    match client.decrypt(decrypt_request).sync() {
        Ok(decrypt_response) => {
            // get the decrypted data key
            let decrypted_data_key = decrypt_response.plaintext.unwrap();

            // decrypt the actual data
            let decrypted_data = decrypt(
                Cipher::aes_256_cbc(),
                &decrypted_data_key,
                Some(&encrypted_data[..16]), // the first 16 bytes is the IV
                &encrypted_data[16..], // the rest is the actual encrypted data
            )?;

            Ok(String::from_utf8(decrypted_data)?)
        },
        Err(error) => {
            Err(format!("Failed to decrypt the data key: {:?}", error).into())
        }
    }
}

