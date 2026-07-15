use super::*;

pub(super) async fn load(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<Vec<operator::ModuleRoot>>, String> {
    let owned_client = local_client(node, workspace)?;
    let Some(client) = node.or(owned_client.as_ref()) else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    Ok(Some(status.modules.iter().map(module_root).collect()))
}

pub(super) fn module_root(module: &ModuleStatus) -> operator::ModuleRoot {
    operator::ModuleRoot {
        id: module.id.clone(),
        root: module.root.clone(),
        category: match module.category.as_deref() {
            Some("workspace") => operator::ModuleCategory::Workspace,
            Some("developer") => operator::ModuleCategory::Developer,
            Some("automation") => operator::ModuleCategory::Automation,
            _ => operator::ModuleCategory::System,
        },
    }
}
