pub mod sublevel;
pub mod topsub;

/// Heres is a comment!
///```ignore
///     Here is a doc comment that should get wrapped in ignore
///```
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TestMessage {
    #[prost(int32, tag = "1")]
    pub field_one: i32,
    #[prost(string, tag = "2")]
    pub field_two: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub cross_package_include: ::core::option::Option<super::include::IncludeMessage>,
    #[prost(message, optional, tag = "4")]
    pub direct_subdependency_import: ::core::option::Option<
        super::include::othersub::NestIncludeMessage,
    >,
}
