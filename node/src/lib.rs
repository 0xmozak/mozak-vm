#![feature(assert_matches)]

pub use id::Id;
pub use user::message::Message;
pub use user::message_service::{DummyMessageService, MessageService};

mod user;

mod id;
