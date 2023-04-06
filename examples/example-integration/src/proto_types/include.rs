pub mod othersub;
pub mod sub;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IncludeMessage {
    #[prost(int32, tag = "1")]
    pub field_one: i32,
    #[prost(string, tag = "2")]
    pub field_two: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub include_sub: ::core::option::Option<sub::IncludeSubMessage>,
}
