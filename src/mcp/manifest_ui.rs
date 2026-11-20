/// 这个模块由AI负责

use super::{MyServer, get_info};
use rmcp::model::{ExtensionCapabilities, ServerCapabilities, ServerConfig};
use rmcp::model::{ListResourcesResult, ListToolsResult, MetaObject, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, Tool};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, tool_handler};

#[allow(dead_code)]
const MANIFEST_RESOURCE_URI: &str = "ui://disk-cleaner/manifest";
pub const MANIFEST_MIME_TYPE: &str = "text/html;profile=mcp-app";

#[allow(dead_code)]
fn manifest_tool(mut tool: Tool) -> Tool {
    let mut meta = tool.meta.unwrap_or_default();
    meta.0.insert(
        "ui".to_string(),
        serde_json::json!({ "resourceUri": MANIFEST_RESOURCE_URI }),
    );
    tool.meta = Some(meta);
    tool
}

fn manifest_resource_meta() -> MetaObject {
    MetaObject(serde_json::Map::from_iter([(
        "ui".to_string(),
        serde_json::json!({ "prefersBorder": true }),
    )]))
}

pub fn manifest_tool_meta() -> MetaObject {
    MetaObject(serde_json::Map::from_iter([(
        "ui".to_string(),
        serde_json::json!({
            "resourceUri": MANIFEST_RESOURCE_URI,
            "visibility": ["model", "app"]
        }),
    )]))
}

#[allow(dead_code)]
fn manifest_resource() -> Resource {
    Resource::new(MANIFEST_RESOURCE_URI, "cleanup_manifest")
        .with_title("Cleanup manifest")
        .with_description("Review and decide what to clean")
        .with_mime_type(MANIFEST_MIME_TYPE)
        .with_meta(manifest_resource_meta())
}

#[tool_handler]
impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerConfig {
        let mut ui_extension = serde_json::Map::new();
        ui_extension.insert(
            "mimeTypes".to_string(),
            serde_json::json!([MANIFEST_MIME_TYPE]),
        );
        let extensions = ExtensionCapabilities::from([(
            "io.modelcontextprotocol/ui".to_string(),
            ui_extension,
        )]);

        let cfg = ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_extensions_with(extensions)
                .build(),
        );
        get_info(cfg)
    }
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| {
                if tool.name == "show_manifest" {
                    manifest_tool(tool)
                } else {
                    tool
                }
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned().map(|tool| {
            if name == "show_manifest" {
                manifest_tool(tool)
            } else {
                tool
            }
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            manifest_resource(),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != MANIFEST_RESOURCE_URI {
            return Err(ErrorData::resource_not_found(request.uri, None));
        }

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(include_str!("manifest.html"), MANIFEST_RESOURCE_URI)
                .with_mime_type(MANIFEST_MIME_TYPE)
                .with_meta(manifest_resource_meta()),
        ]).into())
    }
}
