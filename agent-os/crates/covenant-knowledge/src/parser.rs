use crate::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ParsedFile {
    pub objects: Vec<Object>,
    pub relationships: Vec<Relationship>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ParsedFile {
    pub(crate) fn valid_for(&self, file: &SourceFile) -> bool {
        let digest = sha256(file.content.as_bytes());
        let ids: BTreeSet<_> = self.objects.iter().map(|o| o.id.as_str()).collect();
        !self.objects.is_empty()
            && ids.len() == self.objects.len()
            && self.objects.iter().all(|o| {
                o.path == file.path
                    && o.source_sha256 == digest
                    && o.start_line > 0
                    && o.end_line >= o.start_line
                    && file.content.get(o.start_byte..o.end_byte).is_some()
            })
            && self.relationships.iter().all(|r| {
                r.path == file.path
                    && ids.contains(r.from.as_str())
                    && r.to.as_deref().is_none_or(|id| ids.contains(id))
            })
            && self.diagnostics.iter().all(|d| d.path == file.path)
    }
}

pub(crate) fn cache_key(file: &SourceFile) -> String {
    sha256(
        format!(
            "{}:{}:{}:{}",
            COMPILER_VERSION,
            file.path,
            sha256(file.content.as_bytes()),
            serde_json::to_string(&parser_versions()).unwrap()
        )
        .as_bytes(),
    )
}

pub(crate) fn language(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|p| p.to_str())
        .unwrap_or("")
    {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "md" | "markdown" | "mdx" => "markdown",
        _ => "text",
    }
}

fn object(
    file: &SourceFile,
    lang: &str,
    digest: &str,
    kind: &str,
    name: &str,
    start: usize,
    end: usize,
    start_line: usize,
    end_line: usize,
    exported: bool,
) -> Object {
    let id = sha256(format!("{}\0{}\0{}\0{}\0{}", file.path, kind, name, start, end).as_bytes());
    let signature = file
        .content
        .get(start..end)
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(240)
        .collect();
    Object {
        id,
        name: name.into(),
        kind: kind.into(),
        language: lang.into(),
        path: file.path.clone(),
        start_byte: start,
        end_byte: end,
        start_line,
        end_line,
        source_sha256: digest.into(),
        signature,
        exported,
    }
}

pub(crate) fn parse(file: &SourceFile) -> Result<ParsedFile> {
    let lang = language(&file.path);
    let digest = sha256(file.content.as_bytes());
    let file_obj = object(
        file,
        lang,
        &digest,
        "file",
        &file.path,
        0,
        file.content.len(),
        1,
        file.content.lines().count().max(1),
        false,
    );
    let mut parsed = ParsedFile {
        objects: vec![file_obj.clone()],
        relationships: vec![],
        diagnostics: vec![],
    };
    if lang == "text" {
        parsed.diagnostics.push(Diagnostic {
            path: file.path.clone(),
            code: "text_only".into(),
            message: "Indexed as text; structural extraction is unsupported for this format."
                .into(),
        });
        return Ok(parsed);
    }
    if lang == "markdown" {
        let mut headings = Vec::new();
        let mut offset = 0;
        let mut fence: Option<(u8, usize)> = None;
        for (i, line) in file.content.split_inclusive('\n').enumerate() {
            let trim = line.trim_start();
            let marker = trim.as_bytes().first().copied().unwrap_or(0);
            let length = trim.bytes().take_while(|c| *c == marker).count();
            if matches!(marker, b'`' | b'~') && length >= 3 {
                match fence {
                    None => fence = Some((marker, length)),
                    Some((open, n))
                        if marker == open && length >= n && trim[length..].trim().is_empty() =>
                    {
                        fence = None
                    }
                    _ => {}
                }
            } else if fence.is_none() {
                let level = trim.bytes().take_while(|c| *c == b'#').count();
                if (1..=6).contains(&level) && trim.as_bytes().get(level) == Some(&b' ') {
                    headings.push((
                        offset,
                        i + 1,
                        level,
                        trim[level..]
                            .trim()
                            .trim_end_matches('#')
                            .trim()
                            .to_string(),
                    ));
                }
            }
            offset += line.len();
        }
        for (index, (start, line, level, title)) in headings.iter().enumerate() {
            let end = headings[index + 1..]
                .iter()
                .find(|h| h.2 <= *level)
                .map(|h| h.0)
                .unwrap_or(file.content.len());
            let end_line = line + file.content[*start..end].lines().count().saturating_sub(1);
            let o = object(
                file, lang, &digest, "heading", title, *start, end, *line, end_line, false,
            );
            parsed.relationships.push(Relationship {
                from: file_obj.id.clone(),
                to: Some(o.id.clone()),
                kind: "contains".into(),
                target: title.clone(),
                resolution: "resolved".into(),
                path: file.path.clone(),
                line: *line,
            });
            parsed.objects.push(o);
        }
        return Ok(parsed);
    }
    let grammar = match lang {
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => unreachable!(),
    };
    let mut parser = Parser::new();
    parser.set_language(&grammar)?;
    let tree = parser
        .parse(file.content.as_bytes(), None)
        .context("parser returned no tree")?;
    if tree.root_node().has_error() {
        parsed.diagnostics.push(Diagnostic {
            path: file.path.clone(),
            code: "parse_error".into(),
            message: "Parser found malformed syntax; extracted objects are partial.".into(),
        });
    }
    walk(
        tree.root_node(),
        file,
        lang,
        &digest,
        &file_obj.id,
        false,
        0,
        &mut parsed,
    );
    Ok(parsed)
}

fn text<'a>(n: Node<'_>, file: &'a SourceFile) -> &'a str {
    file.content.get(n.byte_range()).unwrap_or("")
}

fn walk(
    n: Node<'_>,
    file: &SourceFile,
    lang: &str,
    digest: &str,
    owner: &str,
    exported: bool,
    depth: usize,
    out: &mut ParsedFile,
) {
    if depth > 256 || out.objects.len() >= 10_000 {
        if !out.diagnostics.iter().any(|d| d.code == "extraction_limit") {
            out.diagnostics.push(Diagnostic {
                path: file.path.clone(),
                code: "extraction_limit".into(),
                message: "Structural extraction limit reached; file coverage is partial.".into(),
            });
        }
        return;
    }
    // Error branches are not promoted to declarations.
    if n.is_error() || n.is_missing() {
        return;
    }
    let kind = match n.kind() {
        "function_item"
        | "function_definition"
        | "function_declaration"
        | "generator_function_declaration"
        | "method_definition"
        | "method_signature" => Some("function"),
        "struct_item" => Some("struct"),
        "enum_item" | "enum_declaration" => Some("enum"),
        "trait_item" => Some("trait"),
        "mod_item" => Some("module"),
        "class_definition" | "class_declaration" | "abstract_class_declaration" => Some("class"),
        "interface_declaration" => Some("interface"),
        "type_item" | "type_alias_declaration" => Some("type"),
        "const_item" | "static_item" => Some("constant"),
        "variable_declarator" => Some(
            if n.child_by_field_name("value").is_some_and(|v| {
                matches!(
                    v.kind(),
                    "arrow_function" | "function_expression" | "generator_function"
                )
            }) {
                "function"
            } else {
                "variable"
            },
        ),
        _ => None,
    };
    let mut current_owner = owner.to_string();
    let mut is_exported = exported || n.kind() == "export_statement";
    if lang == "rust" {
        let mut c = n.walk();
        is_exported |= n
            .named_children(&mut c)
            .any(|c| c.kind() == "visibility_modifier");
    }
    if let Some(kind) = kind {
        if let Some(name) = n.child_by_field_name("name") {
            let name = text(name, file);
            let o = object(
                file,
                lang,
                digest,
                kind,
                name,
                n.start_byte(),
                n.end_byte(),
                n.start_position().row + 1,
                n.end_position().row + 1,
                is_exported,
            );
            out.relationships.push(Relationship {
                from: owner.into(),
                to: Some(o.id.clone()),
                kind: "contains".into(),
                target: name.into(),
                resolution: "resolved".into(),
                path: file.path.clone(),
                line: n.start_position().row + 1,
            });
            current_owner = o.id.clone();
            out.objects.push(o);
        }
    }
    let import_target = match n.kind() {
        "import_statement" if lang != "python" => n
            .child_by_field_name("source")
            .map(|n| text(n, file).to_string()),
        "export_statement" => n
            .child_by_field_name("source")
            .map(|n| text(n, file).to_string()),
        "use_declaration" => n
            .child_by_field_name("argument")
            .map(|n| text(n, file).to_string()),
        "import_from_statement" => n
            .child_by_field_name("module_name")
            .map(|n| text(n, file).to_string()),
        "mod_item" if n.child_by_field_name("body").is_none() => n
            .child_by_field_name("name")
            .map(|n| format!("self::{}", text(n, file))),
        _ => None,
    };
    let mut import_targets: Vec<String> = import_target.into_iter().collect();
    if n.kind() == "import_statement" && lang == "python" {
        let mut cursor = n.walk();
        for module in n.named_children(&mut cursor) {
            let module = if module.kind() == "aliased_import" {
                module.child_by_field_name("name").unwrap_or(module)
            } else {
                module
            };
            import_targets.push(text(module, file).to_string());
        }
    }
    for target in import_targets {
        out.relationships.push(Relationship {
            from: owner.into(),
            to: None,
            kind: "imports".into(),
            target: target.trim_matches(['\'', '"']).into(),
            resolution: "unresolved".into(),
            path: file.path.clone(),
            line: n.start_position().row + 1,
        });
    }
    if matches!(n.kind(), "call_expression" | "call") {
        if let Some(function) = n.child_by_field_name("function") {
            let target = text(function, file);
            // A call-site object keeps exact bytes available independently of
            // its enclosing declaration.
            let call = object(
                file,
                lang,
                digest,
                "call",
                target,
                n.start_byte(),
                n.end_byte(),
                n.start_position().row + 1,
                n.end_position().row + 1,
                false,
            );
            out.relationships.push(Relationship {
                from: call.id.clone(),
                to: None,
                kind: "calls".into(),
                target: target.into(),
                resolution: "unresolved".into(),
                path: file.path.clone(),
                line: n.start_position().row + 1,
            });
            out.relationships.push(Relationship {
                from: current_owner.clone(),
                to: Some(call.id.clone()),
                kind: "contains".into(),
                target: target.into(),
                resolution: "resolved".into(),
                path: file.path.clone(),
                line: n.start_position().row + 1,
            });
            out.objects.push(call);
        }
    }
    // Conservative expression-position identifier sites. Declarations, member
    // names, type names and call targets have separate representations or are
    // excluded. Binding/type inference is intentionally not claimed.
    if n.kind() == "identifier"
        && n.parent().is_some_and(|p| match p.kind() {
            "binary_expression"
            | "binary_operator"
            | "return_statement"
            | "argument_list"
            | "arguments"
            | "expression_statement" => true,
            "variable_declarator" | "let_declaration" => p.child_by_field_name("value") == Some(n),
            "member_expression" | "attribute" => p.child_by_field_name("object") == Some(n),
            _ => false,
        })
    {
        let target = text(n, file);
        let reference = object(
            file,
            lang,
            digest,
            "reference",
            target,
            n.start_byte(),
            n.end_byte(),
            n.start_position().row + 1,
            n.end_position().row + 1,
            false,
        );
        out.relationships.push(Relationship {
            from: reference.id.clone(),
            to: None,
            kind: "references".into(),
            target: target.into(),
            resolution: "unresolved".into(),
            path: file.path.clone(),
            line: n.start_position().row + 1,
        });
        out.relationships.push(Relationship {
            from: current_owner.clone(),
            to: Some(reference.id.clone()),
            kind: "contains".into(),
            target: target.into(),
            resolution: "resolved".into(),
            path: file.path.clone(),
            line: n.start_position().row + 1,
        });
        out.objects.push(reference);
    }
    let mut cursor = n.walk();
    for child in n.named_children(&mut cursor) {
        walk(
            child,
            file,
            lang,
            digest,
            &current_owner,
            is_exported && kind.is_none(),
            depth + 1,
            out,
        );
    }
}

fn normalized_relative(base: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

pub(crate) fn resolve(objects: &[Object], relationships: &mut [Relationship]) {
    let files: BTreeMap<&str, &Object> = objects
        .iter()
        .filter(|o| o.kind == "file")
        .map(|o| (o.path.as_str(), o))
        .collect();
    // Resolve only module paths supported without executing package resolvers.
    for r in relationships.iter_mut().filter(|r| r.kind == "imports") {
        let parent = r.path.rsplit_once('/').map(|p| p.0).unwrap_or("");
        let lang = language(&r.path);
        let mut roots = Vec::new();
        if matches!(lang, "javascript" | "typescript" | "tsx") && r.target.starts_with('.') {
            if let Some(path) = normalized_relative(parent, &r.target) {
                roots.push(path);
            }
        } else if lang == "python" {
            if r.target.starts_with('.') {
                let count = r.target.bytes().take_while(|c| *c == b'.').count();
                let suffix = r.target[count..].replace('.', "/");
                if let Some(path) =
                    normalized_relative(parent, &format!("{}{}", "../".repeat(count - 1), suffix))
                {
                    roots.push(path);
                }
            } else if r
                .target
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
            {
                roots.push(r.target.replace('.', "/"));
            }
        } else if lang == "rust" && !r.target.contains(['{', '*', ' ']) {
            let target = r.target.replace("::", "/");
            if let Some(suffix) = target.strip_prefix("crate/") {
                let base = r
                    .path
                    .rsplit_once("/src/")
                    .map(|(p, _)| format!("{p}/src"))
                    .unwrap_or_else(|| {
                        if r.path.starts_with("src/") {
                            "src".into()
                        } else {
                            parent.into()
                        }
                    });
                roots.push(format!("{base}/{suffix}").trim_start_matches('/').into());
            } else if let Some(suffix) = target.strip_prefix("self/") {
                let base = if matches!(
                    Path::new(&r.path).file_name().and_then(|p| p.to_str()),
                    Some("mod.rs" | "lib.rs" | "main.rs")
                ) {
                    parent.to_string()
                } else {
                    r.path.trim_end_matches(".rs").to_string()
                };
                if let Some(p) = normalized_relative(&base, suffix) {
                    roots.push(p);
                }
            }
            // Do not silently drop the final path segment: a missing module or
            // imported symbol must not become a false edge to its parent.
        }
        let mut candidates = BTreeSet::new();
        let suffixes: &[&str] = match lang {
            "rust" => &["", ".rs", "/mod.rs"],
            "python" => &["", ".py", ".pyi", "/__init__.py"],
            _ => &[
                "",
                ".ts",
                ".tsx",
                ".js",
                ".jsx",
                ".mjs",
                "/index.ts",
                "/index.tsx",
                "/index.js",
            ],
        };
        for root in roots {
            for ext in suffixes {
                if let Some(o) = files.get(format!("{root}{ext}").as_str()) {
                    candidates.insert(o.id.clone());
                }
            }
        }
        if candidates.len() == 1 {
            r.to = candidates.into_iter().next();
            r.resolution = "resolved".into();
        } else if candidates.len() > 1 {
            r.resolution = "ambiguous".into();
        }
    }
    // Only unambiguous top-level direct calls in the same file are resolved.
    // Method dispatch, imported aliases, macros and shadowed locals stay visible
    // as unresolved instead of guessed edges.
    let file_ids: BTreeSet<_> = files.values().map(|o| o.id.as_str()).collect();
    let top_level: BTreeSet<_> = relationships
        .iter()
        .filter(|r| r.kind == "contains" && file_ids.contains(r.from.as_str()))
        .filter_map(|r| r.to.clone())
        .collect();
    let mut functions: BTreeMap<(&str, &str), Vec<&Object>> = BTreeMap::new();
    for o in objects
        .iter()
        .filter(|o| o.kind == "function" && top_level.contains(&o.id))
    {
        functions.entry((&o.path, &o.name)).or_default().push(o);
    }
    for r in relationships.iter_mut().filter(|r| r.kind == "calls") {
        if !r.target.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let candidates = functions
            .get(&(r.path.as_str(), r.target.as_str()))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if candidates.len() == 1 {
            // Syntax cannot rule out a parameter/local binding with the same
            // name, so this is a candidate, not a definite runtime call.
            r.to = Some(candidates[0].id.clone());
            r.resolution = "candidate".into();
        } else if candidates.len() > 1 {
            r.resolution = "ambiguous".into();
        }
    }
}
