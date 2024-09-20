use lettre::message::{Message, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
use std::env;

struct EmailClient {
    mailer: Option<SmtpTransport>,
    from_address: Option<String>,
}

impl EmailClient {
    fn new() -> Self {
        let from_address = env::var("GMAIL_USER").ok();
        let app_password = env::var("GMAIL_APP_PASSWORD").ok();

        if let (Some(from_address), Some(app_password)) = (from_address, app_password) {
            let creds = Credentials::new(from_address.clone(), app_password);
            let mailer = SmtpTransport::starttls_relay("smtp.gmail.com")
                .unwrap()
                .credentials(creds)
                .build();

            EmailClient {
                mailer: Some(mailer),
                from_address: Some(from_address),
            }
        } else {
            log::warn!("Failed to create EmailClient: missing credentials");
            EmailClient {
                mailer: None,
                from_address: None,
            }
        }
    }

    fn send(&self, to: &str, subject: &str, body: &str) {
        if let Some(mailer) = &self.mailer {
            let from_address = self.from_address.as_ref().expect("from_address is missing");
            let email = Message::builder()
                .from(from_address.parse().unwrap())
                .to(to.parse().unwrap())
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
