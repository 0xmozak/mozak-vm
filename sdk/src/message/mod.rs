mod message_header;
mod v0;

#[allow(clippy::module_name_repetitions)]
pub enum VersionedMessage {
    V0(v0::Message),
}
