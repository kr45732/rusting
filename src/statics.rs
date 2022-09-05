use lazy_static::lazy_static;
use tokio::sync::Mutex;
use twilight_model::id::{marker::ApplicationMarker, Id};

lazy_static! {
    pub static ref SELF_USER_ID: Mutex<Option<Id<ApplicationMarker>>> = Mutex::new(None);
}
