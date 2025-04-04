use std::collections::HashMap;

use ecow::EcoString;
use lsp_types::{RenameParams, TextEdit, Url, WorkspaceEdit};

use crate::{
    analyse::name,
    ast::{self, SrcSpan, TypedModule, visit::Visit},
    build::Module,
    line_numbers::LineNumbers,
    type_::{ValueConstructor, ValueConstructorVariant, error::Named},
};

use super::TextEdits;

fn workspace_edit(uri: Url, edits: Vec<TextEdit>) -> WorkspaceEdit {
    let mut changes = HashMap::new();
    let _ = changes.insert(uri, edits);

    WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }
}

pub enum VariableRenameKind {
    Variable,
    LabelShorthand,
}

pub fn rename_local_variable(
    module: &Module,
    line_numbers: &LineNumbers,
    params: &RenameParams,
    definition_location: SrcSpan,
    kind: VariableRenameKind,
) -> Option<WorkspaceEdit> {
    if name::check_name_case(
        Default::default(),
        &params.new_name.as_str().into(),
        Named::Variable,
    )
    .is_err()
    {
        return None;
    }

    let uri = params.text_document_position.text_document.uri.clone();
    let mut edits = TextEdits::new(line_numbers);

    let references = find_variable_references(&module.ast, definition_location);

    match kind {
        VariableRenameKind::Variable => edits.replace(definition_location, params.new_name.clone()),
        VariableRenameKind::LabelShorthand => {
            edits.insert(definition_location.end, format!(" {}", params.new_name))
        }
    }

    references
        .into_iter()
        .for_each(|location| edits.replace(location, params.new_name.clone()));

    Some(workspace_edit(uri, edits.edits))
}

pub fn find_variable_references(
    module: &TypedModule,
    definition_location: SrcSpan,
) -> Vec<SrcSpan> {
    let mut finder = FindVariableReferences {
        references: Vec::new(),
        definition_location,
    };
    finder.visit_typed_module(module);
    finder.references
}

struct FindVariableReferences {
    references: Vec<SrcSpan>,
    definition_location: SrcSpan,
}

impl<'ast> Visit<'ast> for FindVariableReferences {
    fn visit_typed_function(&mut self, fun: &'ast ast::TypedFunction) {
        if fun.full_location().contains(self.definition_location.start) {
            ast::visit::visit_typed_function(self, fun);
        }
    }

    fn visit_typed_expr_var(
        &mut self,
        location: &'ast SrcSpan,
        constructor: &'ast ValueConstructor,
        _name: &'ast EcoString,
    ) {
        match constructor.variant {
            ValueConstructorVariant::LocalVariable {
                location: definition_location,
                ..
            } if definition_location == self.definition_location => self.references.push(*location),
            _ => {}
        }
    }

    fn visit_typed_clause_guard_var(
        &mut self,
        location: &'ast SrcSpan,
        _name: &'ast EcoString,
        _type_: &'ast std::sync::Arc<crate::type_::Type>,
        definition_location: &'ast SrcSpan,
    ) {
        if *definition_location == self.definition_location {
            self.references.push(*location)
        }
    }

    fn visit_typed_pattern_var_usage(
        &mut self,
        location: &'ast SrcSpan,
        _name: &'ast EcoString,
        constructor: &'ast Option<ValueConstructor>,
        _type_: &'ast std::sync::Arc<crate::type_::Type>,
    ) {
        let variant = match constructor {
            Some(constructor) => &constructor.variant,
            None => return,
        };
        match variant {
            ValueConstructorVariant::LocalVariable {
                location: definition_location,
                ..
            } if *definition_location == self.definition_location => {
                self.references.push(*location)
            }
            _ => {}
        }
    }
}
