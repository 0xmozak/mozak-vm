mod message_header;
mod v0;

pub enum VersionedMessage {
    V0(v0::Message),
}
