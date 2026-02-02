use crate::MiniTypescriptFrontend;
use nyar_vm::bytecode::format::NyarcModule;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct PackageJson {
    main: Option<String>,
}

pub struct ProjectLoader {
    frontend: MiniTypescriptFrontend,
}

impl ProjectLoader {
    pub fn new(_base_dir: impl Into<PathBuf>) -> Self {
        Self {
            frontend: MiniTypescriptFrontend::new(),
        }
    }

    /// Loads and compiles a project starting from a directory or a file.
    pub fn load_project(&mut self, start_path: &Path) -> Result<Vec<NyarcModule>, String> {
        let mut modules = Vec::new();
        let mut loaded_files = HashMap::new();

        let entry_point = if start_path.is_dir() {
            let pkg_json_path = start_path.join("package.json");
            if pkg_json_path.exists() {
                let content = fs::read_to_string(&pkg_json_path).map_err(|e| e.to_string())?;
                let pkg: PackageJson = serde_json::from_str(&content).map_err(|e| e.to_string())?;
                let main_file = pkg.main.unwrap_or_else(|| "index.ts".to_string());
                start_path.join(main_file)
            } else {
                start_path.join("index.ts")
            }
        } else {
            start_path.to_path_buf()
        };

        self.load_file_recursive(&entry_point, &mut modules, &mut loaded_files)?;

        Ok(modules)
    }

    fn load_file_recursive(
        &mut self,
        file_path: &Path,
        modules: &mut Vec<NyarcModule>,
        loaded_files: &mut HashMap<PathBuf, usize>,
    ) -> Result<usize, String> {
        let canonical_path = fs::canonicalize(file_path)
            .map_err(|e| format!("Failed to canonicalize {:?}: {}", file_path, e))?;

        if let Some(&idx) = loaded_files.get(&canonical_path) {
            return Ok(idx);
        }

        let source = fs::read_to_string(&canonical_path)
            .map_err(|e| format!("Failed to read {:?}: {}", canonical_path, e))?;

        // Compile current file
        let module = self
            .frontend
            .compile_to_nyar(&source)
            .map_err(|e| format!("Compile error in {:?}: {:?}", file_path, e))?;

        let module_idx = modules.len();
        modules.push(module);
        loaded_files.insert(canonical_path.clone(), module_idx);

        // Process imports to load dependencies
        // For now, we need a way to extract imports from the module or the source.
        // Since we already have the compiled module, we can look at its imports_info.

        // But wait, NyarModule's imports only have provider strings.
        // We need to resolve these relative to the current file.
        let parent_dir = canonical_path.parent().unwrap_or(Path::new("."));

        let imports = modules[module_idx].imports.clone();
        println!(
            "Loaded file {:?}, found {} imports",
            canonical_path,
            imports.len()
        );
        for import in imports {
            println!("  Import provider: {}", import.provider);
            if import.provider.starts_with(".") {
                // Relative import
                let mut dep_path = parent_dir.join(&import.provider);
                if !dep_path.exists() && dep_path.with_extension("ts").exists() {
                    dep_path = dep_path.with_extension("ts");
                }

                self.load_file_recursive(&dep_path, modules, loaded_files)?;
            }
            // For now, ignore non-relative imports (assume they are built-ins or already loaded)
        }

        Ok(module_idx)
    }
}
