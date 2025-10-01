use axum::response::Html;

pub async fn graphql_playground() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>GraphQL Playground</title>
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/static/css/index.css" />
  <link rel="shortcut icon" href="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/favicon.png" />
  <script src="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/static/js/middleware.js"></script>
</head>
<body>
  <div id="root"></div>
  <script>
    window.addEventListener('load', function (event) {
      GraphQLPlayground.init(document.getElementById('root'), {
        endpoint: '/graphql',
        settings: {
          'request.credentials': 'same-origin'
        }
      })
    })
  </script>
</body>
</html>
"#,
    )
}
