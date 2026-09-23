use f8_core::compiler::compiled::{DependencyNode, DependencyTree};

/// Renders an upstream precedent calculation tree.
#[must_use]
pub fn render_precedents(tree: &DependencyTree, recursive: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("Coordinate:     {}\n", tree.root));

    if !recursive {
        if tree.direct_count == 0 {
            out.push_str("Direct Precedents: None (Input driver)\n");
        } else {
            out.push_str(&format!("Direct Precedents ({}):\n", tree.direct_count));
            render_node_list(&tree.nodes, "  ", &mut out);
        }
    } else if tree.transitive_count == 0 {
        out.push_str("Precedent Tree: None (Input driver)\n");
    } else {
        let calc_label = if tree.transitive_count == 1 {
            "1 calculation".to_string()
        } else {
            format!("{} calculations", tree.transitive_count)
        };
        out.push_str(&format!("Transitive Precedent Tree ({}):\n", calc_label));
        render_node_list(&tree.nodes, "  ", &mut out);
    }

    out
}

/// Renders a downstream blast-radius dependent tree.
#[must_use]
pub fn render_dependents(tree: &DependencyTree, recursive: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("Coordinate:     {}\n", tree.root));

    if !recursive {
        if tree.direct_count == 0 {
            out.push_str("Direct Dependents: None (Terminal output)\n");
        } else {
            out.push_str(&format!("Direct Dependents ({}):\n", tree.direct_count));
            render_node_list(&tree.nodes, "  ", &mut out);
        }
    } else if tree.transitive_count == 0 {
        out.push_str("Blast Radius: None (Terminal output)\n");
    } else {
        let calc_label = if tree.transitive_count == 1 {
            "1 downstream calculation affected".to_string()
        } else {
            format!("{} downstream calculations affected", tree.transitive_count)
        };
        out.push_str(&format!("Transitive Blast Radius ({}):\n", calc_label));
        render_node_list(&tree.nodes, "  ", &mut out);
    }

    out
}

fn render_node_list(nodes: &[DependencyNode], prefix: &str, out: &mut String) {
    let count = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        let branch = if is_last { "└── " } else { "├── " };
        let next_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };
        if node.is_pruned {
            out.push_str(&format!(
                "{prefix}{branch}{} (already visited)\n",
                node.cell_id
            ));
        } else {
            out.push_str(&format!("{prefix}{branch}{}\n", node.cell_id));
        }
        if !node.children.is_empty() {
            render_node_list(&node.children, &next_prefix, out);
        }
    }
}
