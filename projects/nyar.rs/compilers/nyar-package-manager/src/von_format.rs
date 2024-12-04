use std::collections::BTreeMap;

use crate::manifest::{DependencySpec, PackageManifest, PublishConfig, PublishTarget};

fn escape_von_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_string_field(out: &mut String, indent: &str, name: &str, value: &str) {
    out.push_str(indent);
    out.push_str(name);
    out.push_str(": \"");
    out.push_str(&escape_von_string(value));
    out.push_str("\",\n");
}

fn write_optional_string_field(out: &mut String, indent: &str, name: &str, value: &Option<String>) {
    if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
        write_string_field(out, indent, name, value);
    }
}

fn render_dependency_value(spec: &DependencySpec) -> String {
    match spec {
        DependencySpec::Workspace => "workspace:*".to_string(),
        DependencySpec::Version(version) => format!("\"{}\"", escape_von_string(version)),
        DependencySpec::Detailed { version, path, registry } => {
            let mut inner = String::from("{\n");
            if let Some(version) = version {
                write_string_field(&mut inner, "        ", "version", version);
            }
            if let Some(path) = path {
                write_string_field(&mut inner, "        ", "path", path);
            }
            if let Some(registry) = registry {
                write_string_field(&mut inner, "        ", "registry", registry);
            }
            inner.push_str("    }");
            inner
        }
    }
}

fn render_dependency_map(out: &mut String, indent: &str, name: &str, map: &BTreeMap<String, DependencySpec>) {
    if map.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str(name);
    out.push_str(": {\n");
    for (dep_name, spec) in map {
        out.push_str(indent);
        out.push_str("    \"");
        out.push_str(&escape_von_string(dep_name));
        out.push_str("\": ");
        out.push_str(&render_dependency_value(spec));
        out.push_str(",\n");
    }
    out.push_str(indent);
    out.push_str("},\n");
}

fn render_string_map(out: &mut String, indent: &str, name: &str, map: &BTreeMap<String, String>) {
    if map.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str(name);
    out.push_str(": {\n");
    for (key, value) in map {
        out.push_str(indent);
        out.push_str("    \"");
        out.push_str(&escape_von_string(key));
        out.push_str("\": \"");
        out.push_str(&escape_von_string(value));
        out.push_str("\",\n");
    }
    out.push_str(indent);
    out.push_str("},\n");
}

fn render_publish_config(out: &mut String, indent: &str, config: &PublishConfig) {
    if config.registry.is_empty() && config.access == "public" && config.tag == "latest" {
        return;
    }
    out.push_str(indent);
    out.push_str("publishConfig: {\n");
    if !config.registry.is_empty() {
        write_string_field(out, &format!("{indent}    "), "registry", &config.registry);
    }
    write_string_field(out, &format!("{indent}    "), "access", &config.access);
    write_string_field(out, &format!("{indent}    "), "tag", &config.tag);
    out.push_str(indent);
    out.push_str("},\n");
}

fn render_publish_targets(out: &mut String, indent: &str, targets: &[PublishTarget]) {
    if targets.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str("publish: [\n");
    for target in targets {
        out.push_str(indent);
        out.push_str("    {\n");
        if !target.target.is_empty() {
            write_string_field(out, &format!("{indent}        "), "target", &target.target);
        }
        if !target.format_type.is_empty() {
            write_string_field(out, &format!("{indent}        "), "type", &target.format_type);
        }
        if !target.package_id.is_empty() {
            write_string_field(out, &format!("{indent}        "), "package_id", &target.package_id);
        }
        if !target.version.is_empty() {
            write_string_field(out, &format!("{indent}        "), "version", &target.version);
        }
        out.push_str(indent);
        out.push_str("    },\n");
    }
    out.push_str(indent);
    out.push_str("],\n");
}

fn render_string_list(out: &mut String, indent: &str, name: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str(name);
    out.push_str(": [\n");
    for value in values {
        out.push_str(indent);
        out.push_str("    \"");
        out.push_str(&escape_von_string(value));
        out.push_str("\",\n");
    }
    out.push_str(indent);
    out.push_str("],\n");
}

pub(crate) fn render_package_manifest(manifest: &PackageManifest) -> String {
    let mut out = String::from("{\n");
    write_string_field(&mut out, "    ", "name", &manifest.name);
    write_string_field(&mut out, "    ", "version", &manifest.version);
    write_optional_string_field(&mut out, "    ", "description", &manifest.description);
    write_optional_string_field(&mut out, "    ", "homepage", &manifest.homepage);
    write_optional_string_field(&mut out, "    ", "author", &manifest.author);
    write_optional_string_field(&mut out, "    ", "license", &manifest.license);
    render_dependency_map(&mut out, "    ", "dependencies", &manifest.dependencies);
    render_dependency_map(&mut out, "    ", "dev_dependencies", &manifest.dev_dependencies);
    render_dependency_map(&mut out, "    ", "peerDependencies", &manifest.peer_dependencies);
    render_string_map(&mut out, "    ", "scripts", &manifest.scripts);
    render_string_map(&mut out, "    ", "hooks", &manifest.hooks);
    render_publish_config(&mut out, "    ", &manifest.publish_config);
    render_publish_targets(&mut out, "    ", &manifest.publish);
    render_string_list(&mut out, "    ", "files", &manifest.files);
    out.push_str("}\n");
    out
}

pub(crate) fn render_lock_file(version: &str, packages: &BTreeMap<String, crate::lock::LockEntry>) -> String {
    let mut out = String::from("{\n");
    write_string_field(&mut out, "    ", "version", version);
    if !packages.is_empty() {
        out.push_str("    packages: {\n");
        for (key, entry) in packages {
            out.push_str("        \"");
            out.push_str(&escape_von_string(key));
            out.push_str("\": {\n");
            write_string_field(&mut out, "            ", "name", &entry.name);
            write_string_field(&mut out, "            ", "version", &entry.version);
            if !entry.registry.is_empty() {
                write_string_field(&mut out, "            ", "registry", &entry.registry);
            }
            if !entry.resolved.is_empty() {
                write_string_field(&mut out, "            ", "resolved", &entry.resolved);
            }
            if !entry.integrity.is_empty() {
                write_string_field(&mut out, "            ", "integrity", &entry.integrity);
            }
            if !entry.license.is_empty() {
                write_string_field(&mut out, "            ", "license", &entry.license);
            }
            if entry.is_dev {
                out.push_str("            is_dev: true,\n");
            }
            if entry.is_workspace {
                out.push_str("            is_workspace: true,\n");
            }
            if !entry.install_path.is_empty() {
                write_string_field(&mut out, "            ", "install_path", &entry.install_path);
            }
            if !entry.dependencies.is_empty() {
                out.push_str("            dependencies: [\n");
                for dep in &entry.dependencies {
                    out.push_str("                \"");
                    out.push_str(&escape_von_string(dep));
                    out.push_str("\",\n");
                }
                out.push_str("            ],\n");
            }
            out.push_str("        },\n");
        }
        out.push_str("    },\n");
    }
    out.push_str("}\n");
    out
}
