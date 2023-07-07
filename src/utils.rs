use chrono::{DateTime, NaiveDateTime, Utc};
use std::time::SystemTime;

pub struct DateTimeUtils {}

pub trait ToDateTimeString {
    fn to_datetime_string(self) -> String;
}

impl ToDateTimeString for i64 {
    fn to_datetime_string(self) -> String {
        let naive_datetime = NaiveDateTime::from_timestamp_opt(self, 0).expect("Invalid timestamp");
        naive_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

impl ToDateTimeString for SystemTime {
    fn to_datetime_string(self) -> String {
        let datetime: DateTime<Utc> = self.into();
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

impl DateTimeUtils {
    pub fn get_current_datetime_string() -> String {
        let current_time = Utc::now().timestamp();
        let naive_datetime =
            NaiveDateTime::from_timestamp_opt(current_time, 0).expect("Invalid timestamp");
        naive_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn get_current_date_string() -> String {
        let current_time = Utc::now().timestamp();
        let naive_datetime =
            NaiveDateTime::from_timestamp_opt(current_time, 0).expect("Invalid timestamp");
        naive_datetime.format("%Y-%m-%d").to_string()
    }
}
