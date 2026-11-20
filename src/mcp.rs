mod manifest_ui;

use std::path::Path;
use std::sync::LazyLock;
use rmcp::{
    ErrorData,
    handler::server::{tool::ToolRouter},
    model::*,
    tool, tool_router,
};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use jsonschema::{validator_for, Validator};
use crate::mcp::manifest_ui::manifest_tool_meta;
use super::scan::*;



pub struct MyServer {
    #[allow(dead_code)]
    pub(crate) tool_router: ToolRouter<Self>
}

pub(self) fn get_info(cfg: ServerConfig) -> ServerConfig {
    cfg.with_server_info(Implementation::new(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    )).with_instructions("提供了帮用户清理磁盘时需要的工具，\
        你应该使用本MCP服务器提供的工具帮用户分析和清理磁盘垃圾，\
        而不是通过调用终端，本MCP服务器提供的工具由rust编写，\
        性能比PowerShell和python高10倍，你用终端用户会骂你为什么让他等这么久"
    )
}

impl MyServer {
    pub fn new() -> Self {
        Self {tool_router: Self::tool_router()}
    }
}

type ToolResult = Result<CallToolResult, ErrorData>;

#[derive(Deserialize, JsonSchema, Serialize)]
struct CheckDirParam {
    #[schemars(description = "目录路径")]
    dir: String
}
static CHECK_DIR: LazyLock<Validator> = LazyLock::new(
    || validator_for(schema_for!(CheckDirParam).as_value()).unwrap()
);
#[derive(Deserialize, JsonSchema, Serialize)]
struct ShowManifestParam {
    #[schemars(description = "建议清理项清单")]
    to_cleans: Vec<ToClean>
}
#[derive(Deserialize, JsonSchema, Serialize)]
struct ToClean {
    #[schemars(description = "目标文件或文件夹的路径")]
    path: String,
    #[schemars(
        description = "清理此项的安全程度打分，从0到100",
        range(min = 0, max = 100)
    )]
    safety: u8,
    #[schemars(description = "对此项的备注、描述，以及对安全性的解释等等")]
    desc: String,
}
static SHOW_MANIFEST: LazyLock<Validator> = LazyLock::new(
    || validator_for(schema_for!(ShowManifestParam).as_value()).unwrap()
);
#[derive(Deserialize, JsonSchema, Serialize)]
struct OpenPathParam {
    #[schemars(description = "要在系统文件管理器中浏览的文件或文件夹路径")]
    path: String,
}
static OPEN_PATH: LazyLock<Validator> = LazyLock::new(
    || validator_for(schema_for!(OpenPathParam).as_value()).unwrap()
);
macro_rules! validate_param {
    ($input:expr, $validator:expr, $ident:ident{$($($i:ident)*),* $(,)?}) => {
        let input_value = Value::Object($input);
        match $validator.validate(&input_value) {
            Err(e) => return Err(ErrorData::invalid_params(
                format!("参数不符合schema: {}", e),
                None
            )),
            _ => ()
        }
        let $ident{$($($i)*),*} = serde_json::from_value(input_value).unwrap();
    };
}
#[tool_router]
impl MyServer {
    #[tool(
        description = "查看一个目录或磁盘内的文件层级结构及文件和文件夹的大小。输出结果会自动通过折叠文件夹保证足够短，且在保证足够短的同时尽量少折叠文件夹。",
        input_schema = schema_for!(CheckDirParam).as_value().as_object().unwrap().clone()
    )]
    async fn check_dir(&self, input: JsonObject) -> ToolResult {
        validate_param!(input, CHECK_DIR, CheckDirParam{dir});
        if !Path::new(&dir).is_dir() {
            return Err(ErrorData::invalid_params(
                format!("目录不存在或目标不是目录: {dir}"),
                None,
            ))
        }
        let result = match ScanResult::new(dir) {
            Ok(result) => result,
            Err(_) => return Err(ErrorData::invalid_params(
                "访问目录失败，可能没有访问权限".to_string(), None
            ))
        };
        Ok(CallToolResult::success(vec![
            ContentBlock::Text(TextContent::new(result.optimised_folding(10000)))
        ]))
    }
    #[tool(
        description = "通过展示界面向用户列出建议清理的目标，用户能在界面中一一做出决定",
        input_schema = schema_for!(ShowManifestParam).as_value().as_object().unwrap().clone(),
        meta = manifest_tool_meta()
    )]
    async fn show_manifest(&self, input: JsonObject) -> ToolResult {
        validate_param!(input, SHOW_MANIFEST, ShowManifestParam{mut to_cleans});
        to_cleans.sort_by(|a, b| b.safety.cmp(&a.safety));
        let mut to_cleans_json = json!(to_cleans);
        let Value::Array(inners) = &mut to_cleans_json else {unreachable!()};
        fn get_path_size(path_str: &str) -> Result<f64, ErrorData> {
            let path = Path::new(path_str);
            if path.is_dir() {
                match ScanResult::new(path_str) {
                    Ok(result) => Ok(result.get_size()),
                    Err(_) => Err(
                        ErrorData::invalid_params(
                            format!("访问目录失败，可能没有权限: {}", path_str),
                            None
                        )
                    )
                }
            } else if path.is_file() {
                let raw_size = path.metadata().map_err(|_| {
                    ErrorData::invalid_params(
                        format!("访问文件失败，可能没有权限: {}", path_str),
                        None
                    )
                })?.len();
                Ok(convert_size(raw_size))
            } else {
                Err(ErrorData::invalid_params(
                    format!("目标不存在: {}", path_str),
                    None
                ))
            }
        }
        for i in inners {
            let Value::Object(obj) = i else {unreachable!()};
            let path = obj["path"].as_str().unwrap_or_else(|| unreachable!());
            obj.insert("size_mb".to_string(), get_path_size(path)?.into());
        }
        Ok(CallToolResult::structured(json!({
            "to_cleans": to_cleans_json,
        })))
    }
    #[tool(
        description = "在系统文件管理器中打开指定文件或文件夹所在的目录，供用户浏览",
        input_schema = schema_for!(OpenPathParam).as_value().as_object().unwrap().clone()
    )]
    /// 这是本模块唯一由AI负责的部分
    async fn open_path(&self, input: JsonObject) -> ToolResult {
        validate_param!(input, OPEN_PATH, OpenPathParam{path});
        let target = Path::new(&path);
        if !target.exists() {
            return Err(ErrorData::invalid_params(
                format!("目标不存在: {path}"),
                None,
            ))
        }
        fn open_in_file_manager(path: &Path) -> std::io::Result<()> {
            let is_file = path.is_file();
            #[cfg(target_os = "windows")]
            {
                let mut command = std::process::Command::new("explorer.exe");
                if is_file {
                    command.arg(format!("/select,{}", path.display()));
                } else {
                    command.arg(path);
                }
                command.spawn()?;
            }
            #[cfg(target_os = "macos")]
            {
                let mut command = std::process::Command::new("open");
                if is_file {
                    command.arg("-R").arg(path);
                } else {
                    command.arg(path);
                }
                command.spawn()?;
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let dir = if is_file {
                    path.parent().unwrap_or(path)
                } else {
                    path
                };
                std::process::Command::new("xdg-open").arg(dir).spawn()?;
            }
            Ok(())
        }
        open_in_file_manager(target).map_err(|error| {
            ErrorData::internal_error(format!("无法打开文件管理器: {error}"), None)
        })?;
        Ok(CallToolResult::success(vec![
            ContentBlock::Text(TextContent::new(format!("已在文件管理器中打开: {path}")))
        ]))
    }
}
