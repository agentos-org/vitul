use opentool_rust::{CliArguments, OpenToolServer, Tool};
use tools::GreetTool;

mod tools;

pub type VitulOpenToolServer = OpenToolServer<GreetTool>;

/// Creates an initialized OpenTool server that can be hosted over HTTP or
/// dispatched in-process through another transport such as native IPC.
pub async fn create_server(
  arguments: impl IntoIterator<Item = String>,
  host: String,
  port: u16,
) -> Result<VitulOpenToolServer, Box<dyn std::error::Error + Send + Sync>> {
  let tool = GreetTool;
  let cli_arguments = CliArguments::new(arguments);
  let tool_args = tool.init_args(cli_arguments.parse()?)?;
  opentool_rust::schema::load_json(tools::SPEC)?;
  tool.init(&tool_args).await?;
  let max_request_body_bytes = if std::env::var_os("AGENTOS_APP_BUNDLE_ID").is_some() {
    1024 * 1024
  } else {
    opentool_rust::DEFAULT_MAX_REQUEST_BODY_BYTES
  };

  Ok(
    OpenToolServer::builder(tool)
      .host(host)
      .port(port)
      .version(env!("CARGO_PKG_VERSION"))
      .spec_json(tools::SPEC)
      .cli_arguments(CliArguments::new(std::iter::empty::<String>()))
      .max_request_body_bytes(max_request_body_bytes)
      .build(),
  )
}
