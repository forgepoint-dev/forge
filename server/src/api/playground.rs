use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use axum::response::Html;

pub async fn graphql_playground() -> Html<String> {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/graphql"),
    ))
}
