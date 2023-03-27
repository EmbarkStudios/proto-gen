#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MyImportantMessage {
    /// Always true
    #[prost(bool, tag = "1")]
    pub is_important: bool,
}
