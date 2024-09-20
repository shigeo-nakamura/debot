use lettre::message::{Message, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
use std::env;

pub struct EmailClient {
    mailer: Option<SmtpTransport>,
    from_address: Option<String>,
    to_address: Option<String>,
}

impl EmailClient {
    pub fn new() -> Self {
        let from_address = env::var("GMAIL_USER").ok();
        let to_address = env::var("TO_ADDRESS").ok();
        let app_password = env::var("GMAIL_APP_PASSWORD").ok();

        if let (Some(from_address), Some(to_address), Some(app_password)) =
            (from_address, to_address, app_password)
        {
            let creds = Credentials::new(from_address.clone(), app_password);
            let mailer = SmtpTransport::starttls_relay("smtp.gmail.com")
                .unwrap()
                .credentials(creds)
                .build();

            EmailClient {
                mailer: Some(mailer),
                from_address: Some(from_address),
                to_address: Some(to_address),
            }
        } else {
            log::warn!("Failed to create EmailClient: missing credentials");
            EmailClient {
                mailer: None,
                from_address: None,
                to_address: None,
            }
        }
    }

    pub fn send(&self, subject: &str, body: &str) {
        if let Some(mailer) = &self.mailer {
            let from_address = self.from_address.as_ref().expect("from_address is missing");
            let to_address = self.to_address.as_ref().expect("to_address is missing");
            let email = Message::builder()
                .from(from_address.parse().unwrap())
                .to(to_address.parse().unwrap())
                .subject(subject)
                .singlepart(SinglePart::plain(body.to_string()))
                .unwrap();

            if let Err(e) = mailer.send(&email) {
                log::warn!("Failed to send an e-mail: {:?}", e);
            }
        } else {
            log::warn!("No mailer available to send the email");
        }
    }
}
